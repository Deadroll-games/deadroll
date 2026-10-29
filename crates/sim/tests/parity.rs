//! Parity: the simulator and the session program give the same results on the same dice.
//!
//! The program side is the program's own code (`deadroll_session::combat_input`, `rules`, the
//! `SyscallRoll` of the VRF callback) fed with its tables and the character program's records.
//! The simulator side starts from JSON presets (round-tripped, so edits to the preset folder do
//! not move this test) and the default `BalanceConfig` loaded from JSON, and rolls with
//! `VrfRoll` (`crates/combat`), the same stream as `SyscallRoll`.
//!
//! Registry data: the simulator reads `assets/registry/*.json` through `deadroll_rules::registry`;
//! the program decodes the table bodies it copies at `open_session`. Both come from the same
//! bytes (`registry_fixture` pins them against the seed script's encoder), and every resonance
//! pair of DAMAGE_TYPES_AND_MIXES 6.4 runs both ways below.
//!
//! Equipment: the program side wears what the character program's own gear code gives
//! (`deadroll_character::state::Gear`: the starter kit, then unequip), snapshotted the way
//! `set_ready` does (`Participation::hero`); the simulator side wears the hero preset's
//! `equipment` (left out = the starter kit). Every hero fights in each [`Gear`].
use deadroll_character::rules::CLASS_START_TIER;
use deadroll_character::state::Gear as CharacterGear;
use deadroll_character::state::{SkillExtra, SkillRecord};
use deadroll_combat::{
    resolve_hit, resolve_hit_into, AccuracyBuff, HitInput, HitResult, ModKind, OpKind,
    ResonanceTables, RollSource, TimedMod, TimedMods, VrfRoll,
};
use deadroll_rules::{
    input::{self, effect_action, EffectAction},
    items::Equipment,
    registry::Registry,
    resolve,
    spread::{spread, BurningCell, Field, BURNING_CELLS_MAX},
    BalanceConfig, HeroStats, MonsterDefense, MonsterProfile, MonsterTemplate,
};
use deadroll_session::{
    combat_input as program, rules as program_rules, state::participation::hero_of,
    vrf::SyscallRoll,
};
use deadroll_sim::{
    config_io,
    presets::{HeroPreset, Presets, SkillPreset},
    setup::{fighter, spawn, Fighter, Issues},
};
use deadroll_skillcheck::{derive, presets as skill_presets, Purpose};
use deadroll_types::{
    ActiveState, Aspect, Cell, CharacterData, Class, EquipSlot, ItemBase, Monster, Race, StateKind,
    STATES_MAX,
};

#[path = "../../combat/tests/golden.rs"]
#[allow(dead_code)]
mod golden;

const NONE: TimedMods = [TimedMod {
    kind: ModKind::Haste,
    aspect: Aspect::Fire,
    turns: 0,
    power_bp: 0,
    source: 0,
    weights_pct: [0; Aspect::COUNT],
}; deadroll_combat::MODS_MAX];

const SEEDS: u8 = 64;

/// The simulator's registry: `assets/registry` through its JSON loader.
fn sim_registry() -> Registry {
    Registry::shipped().unwrap()
}

/// The program's view of the same data: the table bodies decoded as `Monsters::tables_into`
/// does, and the defenses as `read_bestiary` reads them.
fn program_tables(r: &Registry) -> ResonanceTables {
    ResonanceTables::from_bytes(&r.aspects, &r.traits, &r.resonances, &r.triples).unwrap()
}

fn program_defense(r: &Registry, id: u8) -> MonsterDefense {
    let at = id as usize * deadroll_rules::monster::record::SIZE;
    MonsterDefense::from_record(&r.bestiary[at..at + deadroll_rules::monster::record::SIZE])
        .unwrap()
}

/// The default table, through JSON: what the UI starts from.
fn sim_config() -> BalanceConfig {
    config_io::from_json(&config_io::to_json(&BalanceConfig::DEFAULT)).unwrap()
}

fn seed(i: u8) -> [u8; 32] {
    let mut s = [i; 32];
    s[0] = 0xD1;
    s
}

fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned>(v: &T) -> T {
    serde_json::from_str(&serde_json::to_string(v).unwrap()).unwrap()
}

/// What a hero wears in a parity fight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gear {
    /// The class starter kit, as `create_character` gives it.
    Kit,
    /// The kit with the body armor taken off.
    WeaponOnly,
    /// Everything taken off: bare hands.
    Bare,
}

const GEARS: [Gear; 3] = [Gear::Kit, Gear::WeaponOnly, Gear::Bare];

/// Program side: the character program's gear code, then the session's snapshot.
fn program_hero(c: &CharacterData, gear: Gear) -> HeroStats {
    let mut l = CharacterGear::starter(c.class);
    if gear != Gear::Kit {
        l.unequip(EquipSlot::Body).unwrap();
    }
    if gear == Gear::Bare {
        l.unequip(EquipSlot::MainHand).unwrap();
    }
    hero_of(c, CLASS_START_TIER, &l.equipment())
}

/// Simulator side: the hero preset's `equipment` (written out, not taken from the rules).
fn preset_equipment(class: Class, gear: Gear) -> Option<Equipment> {
    let weapon = match class {
        Class::Warrior => ItemBase::Sword,
        Class::Mage => ItemBase::Staff,
        Class::Support => ItemBase::Wand,
    };
    let mut eq = Equipment::EMPTY;
    match gear {
        Gear::Kit => return None,
        Gear::WeaponOnly => eq.set(EquipSlot::MainHand, Some(weapon)),
        Gear::Bare => {}
    }
    Some(eq)
}

/// The two demo characters, as the character program stores them.
fn demo() -> [(CharacterData, &'static [skill_presets::Preset]); 3] {
    let character = |race, class, stats, predisposition| CharacterData {
        version: 1,
        owner: Default::default(),
        index: 0,
        nft: None,
        race,
        class,
        level: 1,
        xp: 0,
        stats,
        predisposition,
        predisposition_tier: 14,
        hp_max: deadroll_character::rules::hp_max(class, &stats, 1),
        alive: true,
        in_session: None,
        appearance: Default::default(),
        state_version: 0,
        bump: 255,
    };
    [
        (
            character(Race::Human, Class::Warrior, [10, 7, 5, 8], Aspect::Piercing),
            &skill_presets::WARRIOR,
        ),
        (
            character(Race::ForestElf, Class::Mage, [5, 7, 10, 8], Aspect::Cold),
            &skill_presets::MAGE,
        ),
        (
            character(Race::Human, Class::Support, [5, 7, 9, 9], Aspect::Blunt),
            &skill_presets::SUPPORT,
        ),
    ]
}

/// What `create_character` stores for a preset skill (its `SkillSet` record and its
/// `SkillExtras` part), as `set_ready` copies it onto the map.
fn record(p: &skill_presets::Preset) -> input::SkillStats {
    let d = derive(&p.blocks);
    let rec = SkillRecord {
        blocks: p.blocks,
        shares_bp: d.shares_bp,
        power_bp: d.power_bp,
        fusion: d.fusion,
        cooldown: d.cooldown,
        ..SkillRecord::default()
    };
    program::skill_stats(&rec, &SkillExtra::new(&p.ext, d.ap_cost)).unwrap()
}

/// The same character and skills as JSON presets, resolved by the simulator.
fn sim_fighter(
    c: &CharacterData,
    set: &[skill_presets::Preset],
    cfg: &BalanceConfig,
    gear: Gear,
) -> Fighter {
    let mut presets = Presets::default();
    let mut ids = Vec::new();
    for (i, p) in set.iter().enumerate() {
        let id = format!("s{i}");
        presets.skills.insert(
            id.clone(),
            round_trip(&SkillPreset::from_blocks(p.name, &p.blocks, &p.ext)),
        );
        ids.push(id);
    }
    let hero = round_trip(&HeroPreset {
        name: "demo".into(),
        race: c.race,
        class: c.class,
        level: c.level,
        stats: c.stats,
        predisposition: c.predisposition,
        tier: c.predisposition_tier,
        class_tier: CLASS_START_TIER,
        skills: ids,
        inline_skills: vec![],
        equipment: preset_equipment(c.class, gear),
    });
    let mut issues = Issues::default();
    let f = fighter(&presets, "demo", &hero, cfg, &mut issues);
    assert!(issues.errors.is_empty(), "{:?}", issues.errors);
    f
}

/// The shipped bestiary monsters as the simulator spawns them (`setup::spawn`), at level 1.
fn sim_monsters(cfg: &BalanceConfig) -> Vec<(u8, MonsterProfile, u32)> {
    let registry = sim_registry();
    (0..5u8)
        .map(|id| {
            let s = spawn(&registry, id, 1, false, None, cfg).unwrap();
            (
                id,
                s.template.profile(1, &cfg.monsters),
                s.template.hp_at(1, 0, &cfg.monsters),
            )
        })
        .collect()
}

/// The program's template of bestiary id `id`: the record bytes `open_session` copies into
/// `MonsterState.records`, decoded as `Monsters::template` does.
fn program_template(id: u8) -> MonsterTemplate {
    // The registry is read from disk once: the triple test asks thousands of times.
    static REGISTRY: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    let r = REGISTRY.get_or_init(sim_registry);
    let at = id as usize * deadroll_rules::monster::record::SIZE;
    MonsterTemplate::from_record(&r.bestiary[at..at + deadroll_rules::monster::record::SIZE])
        .unwrap()
}

/// The program's profile of monster `id` at level 1 (`Monsters::profile`).
fn program_profile(id: u8) -> MonsterProfile {
    program_template(id).profile(1, &program_rules::BALANCE.monsters)
}

fn monster(id: u8, hp: u32) -> Monster {
    Monster {
        bestiary_id: id,
        level: 1,
        hp,
        hp_max: hp,
        alive: true,
        ..Monster::default()
    }
}

/// A monster's natural attack on a fresh hero: the hit, HP and states after it, the death.
fn bite(
    cfg: &BalanceConfig,
    tables: &ResonanceTables,
    profile: &MonsterProfile,
    hero: &deadroll_rules::HeroStats,
    hp: u32,
    src: &mut impl RollSource,
) -> (HitResult, u32, [ActiveState; STATES_MAX], bool) {
    let (mut hp, mut states) = (hp, [ActiveState::default(); STATES_MAX]);
    let (mut input, mut result) = (HitInput::default(), HitResult::default());
    let mut own = NONE;
    let out = resolve::monster_attack(
        cfg,
        tables,
        deadroll_rules::MONSTER_UNIT_BASE,
        profile,
        None,
        &NONE,
        hero,
        &mut hp,
        &mut states,
        &mut own,
        &NONE,
        &mut input,
        &mut result,
        src,
    );
    (result, hp, states, out.died)
}

#[test]
fn golden_fixtures_give_the_same_damage() {
    let cfg = sim_config();
    let tables = golden::tables();
    for (input, s, damage) in [
        (golden::warrior_vs_wolf(), golden::SEED, 8),
        (golden::mage_vs_goblin(), golden::SEED, 13),
        (golden::mage_vs_goblin(), golden::RESONANCE_SEED, 11),
    ] {
        let mut on_chain = HitResult::default();
        resolve_hit_into(
            &input,
            &program_rules::BALANCE.combat,
            &tables,
            &mut SyscallRoll::new(s),
            &mut on_chain,
        );
        let sim = resolve_hit(&input, &cfg.combat, &tables, &mut VrfRoll::new(s));
        assert_eq!(sim, on_chain);
        assert_eq!(sim.damage, damage);
    }
}

#[test]
fn syscall_and_vrf_streams_match() {
    for i in 0..SEEDS {
        let mut a = SyscallRoll::new(seed(i));
        let mut b = VrfRoll::new(seed(i));
        for _ in 0..16 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        // The effect of a skill reads the same stream from the hit's `rolls_used`.
        assert_eq!(
            SyscallRoll::at(seed(i), 5).next_u64(),
            VrfRoll::roll_at(&seed(i), 5)
        );
    }
}

#[test]
fn table_numbers_match_the_program() {
    let cfg = sim_config();
    for (c, set) in demo() {
        for gear in GEARS {
            let f = sim_fighter(&c, set, &cfg, gear);
            let h = program_hero(&c, gear);
            assert_eq!(f.hero, h, "{gear:?}");
            assert_eq!(input::hero::ap(&cfg, &f.hero), program_rules::hero_ap(&h));
            assert_eq!(
                input::hero::initiative(&cfg, &f.hero),
                program_rules::hero_initiative(&h)
            );
            assert_eq!(f.hp_max, program_rules::party_hp_max(c.hp_max, &h));
        }
        let dex = c.stats[deadroll_types::Stat::Dexterity as usize];
        assert_eq!(
            deadroll_rules::player::player_ap(c.race, c.class, dex, &cfg.players),
            program_rules::player_ap(c.race, c.class, dex)
        );
        assert_eq!(
            deadroll_rules::player::player_initiative(&c.stats, &cfg.players),
            program_rules::player_initiative(&c.stats)
        );
        assert_eq!(
            deadroll_rules::player::player_accuracy(&c.stats, &cfg.players),
            program_rules::player_accuracy(&c.stats)
        );
        // Level-ups and spending points: the character program reads the same table.
        for xp in [0, 99, 100, 299, 300, 600, u32::MAX] {
            assert_eq!(
                deadroll_rules::player::level_for(xp, &cfg.players),
                deadroll_character::rules::level_for(xp)
            );
        }
        assert_eq!(
            deadroll_rules::player::level_up_points(1, 3, &cfg.players),
            deadroll_character::rules::level_up_points(1, 3)
        );
        for add in [[1, 0, 2, 1], [0; 4], [5, 0, 0, 0]] {
            assert_eq!(
                deadroll_rules::player::spend_points(&c.stats, 4, &add),
                deadroll_character::rules::spend_points(&c.stats, 4, &add)
            );
        }
    }
    assert_eq!(cfg.players.class_start_tier, CLASS_START_TIER);
    for (id, profile, hp) in sim_monsters(&cfg) {
        assert_eq!(profile, program_profile(id), "monster {id}");
        let t = program_template(id);
        assert_eq!(hp, t.hp_at(1, 0, &program_rules::BALANCE.monsters));
        assert_eq!(
            sim_registry().template(id).unwrap().hp_at(
                1,
                deadroll_types::SPAWN_ELITE,
                &cfg.monsters
            ),
            t.hp_at(
                1,
                deadroll_types::SPAWN_ELITE,
                &program_rules::BALANCE.monsters
            )
        );
        assert_eq!(
            deadroll_rules::monster::monster_ap(t.speed, &cfg.monsters),
            program_rules::monster_ap(t.speed)
        );
        assert_eq!(cfg.monsters.slots, program_rules::BALANCE.monsters.slots);
    }
}

#[test]
fn heroes_and_skills_resolve_like_the_character_program() {
    let cfg = sim_config();
    for (c, set) in demo() {
        let f = sim_fighter(&c, set, &cfg, Gear::Kit);
        // The starter kit adds no HP: the stored maximum is the party maximum.
        assert_eq!(f.hp_max, c.hp_max);
        assert_eq!(f.hero, program_hero(&c, Gear::Kit));
        assert_eq!(
            f.hero,
            HeroStats::new(&c, CLASS_START_TIER).with_equipment(f.hero.equipment)
        );
        let usable: Vec<input::SkillStats> = set
            .iter()
            .map(record)
            .filter(|r| program::skill_use(r) == program::SkillUse::Ok)
            .collect();
        assert_eq!(f.skills.len(), usable.len());
        for (sim, prog) in f.skills.iter().zip(&usable) {
            assert_eq!(sim.stats, *prog);
        }
    }
}

#[test]
fn every_demo_attack_deals_the_same_damage() {
    let cfg = sim_config();
    let monsters = sim_monsters(&cfg);
    let registry = sim_registry();
    let (st, pt) = (registry.tables, program_tables(&registry));
    let (warrior, _) = &demo()[0];
    for ((c, set), gear) in demo()
        .into_iter()
        .flat_map(|d| GEARS.map(|g| (d.clone(), g)))
    {
        let f = sim_fighter(&c, set, &cfg, gear);
        let ph = program_hero(&c, gear);
        let records: Vec<input::SkillStats> = set
            .iter()
            .map(record)
            .filter(|r| program::skill_use(r) == program::SkillUse::Ok)
            .collect();
        let mut choices: Vec<(Option<input::SkillStats>, Option<input::SkillStats>)> =
            vec![(None, None)];
        for (r, s) in records.iter().zip(&f.skills) {
            if r.blocks.purpose == Purpose::Damage {
                choices.push((Some(*r), Some(s.stats)));
            }
        }
        for (rec, stats) in &choices {
            for &(id, profile, hp) in &monsters {
                let no_buff = AccuracyBuff::default();
                let prog_input = HitInput {
                    attacker: program::player_attacker(0, &ph, rec.as_ref(), &NONE, &no_buff),
                    defender: program::monster_defender(
                        &monster(id, hp),
                        &program_profile(id),
                        &NONE,
                    ),
                };
                let sim_input = HitInput {
                    attacker: input::player_attacker(
                        &cfg,
                        0,
                        &f.hero,
                        stats.as_ref(),
                        &NONE,
                        &no_buff,
                    ),
                    defender: input::monster_defender(
                        &profile,
                        &[ActiveState::default(); STATES_MAX],
                        &NONE,
                    ),
                };
                assert_eq!(
                    sim_input,
                    prog_input,
                    "{gear:?} {:?} vs {id}",
                    rec.map(|r| r.blocks)
                );
                for i in 0..SEEDS {
                    // Program: what `consume_randomness` runs.
                    let (mut pm, mut pmods) = (monster(id, hp), NONE);
                    let (mut pin, mut pres) = (HitInput::default(), HitResult::default());
                    let mut p_acc = AccuracyBuff::default();
                    let p_out = resolve::attack_monster(
                        program_rules::BALANCE,
                        &pt,
                        0,
                        &ph,
                        rec.as_ref(),
                        &NONE,
                        &mut p_acc,
                        &program_profile(id),
                        &mut pm,
                        &mut pmods,
                        &NONE,
                        &mut pin,
                        &mut pres,
                        &mut SyscallRoll::new(seed(i)),
                    );
                    // Simulator: presets, the JSON table and `VrfRoll`.
                    let (mut sm, mut smods) = (monster(id, hp), NONE);
                    let (mut sin, mut sres) = (HitInput::default(), HitResult::default());
                    let mut s_acc = AccuracyBuff::default();
                    let s_out = resolve::attack_monster(
                        &cfg,
                        &st,
                        0,
                        &f.hero,
                        stats.as_ref(),
                        &NONE,
                        &mut s_acc,
                        &profile,
                        &mut sm,
                        &mut smods,
                        &NONE,
                        &mut sin,
                        &mut sres,
                        &mut VrfRoll::new(seed(i)),
                    );
                    assert_eq!(sres, pres);
                    assert_eq!(sres.damage, pres.damage);
                    assert_eq!(s_out, p_out);
                    assert_eq!((sm, smods, s_acc), (pm, pmods, p_acc));
                }
            }
        }
    }
    // Every monster's natural attack on the warrior, in each gear (armor cuts the bite).
    for (gear, &(id, profile, hp)) in GEARS
        .iter()
        .flat_map(|g| monsters.iter().map(move |m| (*g, m)))
    {
        let wf = sim_fighter(warrior, &skill_presets::WARRIOR, &cfg, gear);
        let unit = deadroll_rules::MONSTER_UNIT_BASE;
        let prog = program::monster_attacker(unit, &program_profile(id), None, &NONE);
        let _ = hp;
        assert_eq!(
            input::monster_attacker(&cfg, unit, &profile, None, &NONE),
            prog
        );
        let ph = program_hero(warrior, gear);
        let states = [ActiveState::default(); STATES_MAX];
        assert_eq!(
            input::player_defender(&cfg, &wf.hero, &states, &NONE),
            program::player_defender(&ph, &states, &NONE)
        );
        let program_profile = program_profile(id);
        for i in 0..SEEDS {
            let p = bite(
                program_rules::BALANCE,
                &pt,
                &program_profile,
                &ph,
                program_rules::party_hp_max(warrior.hp_max, &ph),
                &mut SyscallRoll::new(seed(i)),
            );
            let s = bite(
                &cfg,
                &st,
                &profile,
                &wf.hero,
                wf.hp_max,
                &mut VrfRoll::new(seed(i)),
            );
            assert_eq!(s, p, "{gear:?}, monster {id}, seed {i}");
        }
    }
}

#[test]
fn every_support_effect_lands_the_same() {
    let cfg = sim_config();
    let monsters = sim_monsters(&cfg);
    for ((c, set), gear) in demo()
        .into_iter()
        .flat_map(|d| GEARS.map(|g| (d.clone(), g)))
    {
        let f = sim_fighter(&c, set, &cfg, gear);
        let ph = program_hero(&c, gear);
        let records: Vec<input::SkillStats> = set
            .iter()
            .map(record)
            .filter(|r| program::skill_use(r) == program::SkillUse::Ok)
            .collect();
        for (rec, sim) in records.iter().zip(&f.skills) {
            let Some(effect) = effect_action(rec.blocks.effect) else {
                continue;
            };
            assert_eq!(
                input::effect_range(&cfg, &f.hero, sim.stats.power_bp, effect),
                program::effect_range(&ph, rec.power_bp, effect)
            );
            match rec.blocks.purpose {
                Purpose::Debuff => {
                    for &(id, profile, hp) in &monsters {
                        for i in 0..SEEDS {
                            let (mut pm, mut pmods) = (monster(id, hp), NONE);
                            let p = resolve::debuff_monster(
                                program_rules::BALANCE,
                                0,
                                &ph,
                                rec,
                                effect,
                                &program_profile(id),
                                &mut pm,
                                &mut pmods,
                                &mut SyscallRoll::new(seed(i)),
                            );
                            let (mut sm, mut smods) = (monster(id, hp), NONE);
                            let s = resolve::debuff_monster(
                                &cfg,
                                0,
                                &f.hero,
                                &sim.stats,
                                effect,
                                &profile,
                                &mut sm,
                                &mut smods,
                                &mut VrfRoll::new(seed(i)),
                            );
                            assert_eq!((s, sm, smods), (p, pm, pmods));
                        }
                    }
                }
                Purpose::Buff | Purpose::Damage => {
                    // On an ally, and on the caster itself (Endurance lengthens own buffs).
                    for (i, on_self) in (0..SEEDS).flat_map(|i| [(i, false), (i, true)]) {
                        let start = c.hp_max / 3;
                        let mut ally = (start, [ActiveState::default(); STATES_MAX], NONE);
                        let p = resolve::support_ally(
                            program_rules::BALANCE,
                            0,
                            &ph,
                            rec,
                            effect,
                            on_self,
                            &mut ally.0,
                            c.hp_max,
                            &mut ally.1,
                            &mut ally.2,
                            &mut SyscallRoll::new(seed(i)),
                        );
                        let mut sim_ally = (start, [ActiveState::default(); STATES_MAX], NONE);
                        let s = resolve::support_ally(
                            &cfg,
                            0,
                            &f.hero,
                            &sim.stats,
                            effect,
                            on_self,
                            &mut sim_ally.0,
                            f.hp_max,
                            &mut sim_ally.1,
                            &mut sim_ally.2,
                            &mut VrfRoll::new(seed(i)),
                        );
                        assert_eq!((s, sim_ally), (p, ally), "on self: {on_self}");
                    }
                }
            }
        }
    }
    // A heal (no starting preset has one): same strength range both ways, from the weapon.
    for ((c, set), gear) in demo()
        .into_iter()
        .flat_map(|d| GEARS.map(|g| (d.clone(), g)))
    {
        let f = sim_fighter(&c, set, &cfg, gear);
        assert_eq!(
            input::effect_range(&cfg, &f.hero, 10_000, EffectAction::Heal),
            program::effect_range(&program_hero(&c, gear), 10_000, EffectAction::Heal)
        );
    }
}

#[test]
fn gear_changes_the_fight() {
    // The parity above would also pass with equipment ignored on both sides; it is not.
    let (warrior, _) = &demo()[0];
    let kit = program_hero(warrior, Gear::Kit);
    let bare = program_hero(warrior, Gear::Bare);
    let states = [ActiveState::default(); STATES_MAX];
    let armor = |h: &HeroStats| program::player_defender(h, &states, &NONE).armor;
    assert_eq!(armor(&kit), 30);
    assert_eq!(armor(&program_hero(warrior, Gear::WeaponOnly)), 20);
    assert_eq!(armor(&bare), 20);
    let weapon = |h: &HeroStats| {
        program::player_attacker(0, h, None, &NONE, &AccuracyBuff::default()).weapon
    };
    assert_eq!((weapon(&kit).min, weapon(&kit).max), (4, 8));
    let unarmed = program_rules::BALANCE.items.unarmed;
    assert_eq!(
        (weapon(&bare).min, weapon(&bare).max),
        (unarmed[0], unarmed[1])
    );
}

#[test]
fn the_character_program_constants_match() {
    // `equip` / `unequip` accept session tokens issued for the session program.
    assert_eq!(
        deadroll_character::auth::SESSION_PROGRAM,
        deadroll_session::ID
    );
}

/// An open 12 x 12 floor with living units on `units`.
struct Open {
    units: Vec<Cell>,
}

impl Field for Open {
    fn passable(&self, c: Cell) -> bool {
        c.x < 12 && c.y < 12
    }

    fn occupied(&self, c: Cell) -> bool {
        self.units.contains(&c)
    }
}

/// The attacker stands here; the target is monster 0.
const ATTACKER: Cell = Cell { x: 3, y: 5 };

/// Goblins around the target at (5, 5): one right behind it (6, 5) and one beside it (5, 6), both
/// bleeding (Blood Arc), one diagonal (4, 4), one further on the line (7, 5).
fn cluster(target_states: [ActiveState; STATES_MAX]) -> Vec<Monster> {
    let hp = program_template(1).hp_at(1, 0, &program_rules::BALANCE.monsters);
    let bleed = ActiveState {
        kind: StateKind::Bleeding,
        turns: 3,
        power: 1,
        source: 9,
    };
    let at = |x, y, states| Monster {
        x,
        y,
        states,
        ..monster(1, hp)
    };
    let mut bleeding = [ActiveState::default(); STATES_MAX];
    bleeding[0] = bleed;
    vec![
        at(5, 5, target_states),
        at(6, 5, bleeding),
        at(5, 6, bleeding),
        at(4, 4, [ActiveState::default(); STATES_MAX]),
        at(7, 5, [ActiveState::default(); STATES_MAX]),
    ]
}

/// A damage skill of `base` by the demo mage, tinted by `tint` (race slot, tier 1) and
/// `second` (item slot, tier 1), with a state chance and a crit chance high enough to reach
/// every op over the seeds.
fn pair_input(cfg: &BalanceConfig, base: Aspect, tint: Aspect, second: Option<Aspect>) -> HitInput {
    use deadroll_skillcheck::{Category, Effect, SkillBlocks, Target, Trigger};
    let (c, _) = &demo()[1];
    let hero = HeroStats::new(c, CLASS_START_TIER);
    let blocks = SkillBlocks {
        category: Category::Active,
        purpose: Purpose::Damage,
        source: deadroll_types::Source::Magical,
        form: deadroll_types::Form::Projectile,
        target: Target::Single,
        aspects: [base, Aspect::default(), Aspect::default()],
        aspect_count: 1,
        effect: Effect::None,
        trigger: Trigger::None,
        aura_radius: 0,
    };
    let skill =
        input::SkillStats::derive(blocks, deadroll_skillcheck::SkillExt::MELEE, &cfg.skills);
    let mut a =
        input::player_attacker(cfg, 0, &hero, Some(&skill), &NONE, &AccuracyBuff::default());
    a.tints = Default::default();
    a.tints[0] = deadroll_combat::Tint::new(tint, 1, deadroll_combat::Carrier::Race);
    if let Some(s) = second {
        a.tints[1] = deadroll_combat::Tint::new(s, 1, deadroll_combat::Carrier::Item);
    }
    a.skill.state_chance_pct = 60;
    a.crit_chance_pct = 50;
    let profile = program_profile(1);
    HitInput {
        attacker: a,
        defender: input::monster_defender(&profile, &[ActiveState::default(); STATES_MAX], &NONE),
    }
}

/// States on the target before the hit so that every op of `base` + `tint` can act: a frozen
/// target for Cleaving Ice and Piercing Frost (a crit on a frozen target), Chill near the threshold
/// for the other Cold pairs (their Freeze ops). Resonance pass: a shocked target for Incinerating
/// Discharge, a frozen one for Ice Conduit, a bleeding one for Blackout.
fn setup_states(base: Aspect, tint: Aspect) -> [ActiveState; STATES_MAX] {
    let mut s = [ActiveState::default(); STATES_MAX];
    let before = |kind, power| ActiveState {
        kind,
        turns: 2,
        power,
        source: 9,
    };
    match (base, tint) {
        (Aspect::Lightning, Aspect::Fire) => s[0] = before(StateKind::Shock, 2_000),
        (Aspect::Lightning, Aspect::Cold) => s[0] = before(StateKind::Freeze, 0),
        (Aspect::Blood, Aspect::Blunt) => s[0] = before(StateKind::Bleeding, 1),
        _ => {}
    }
    if base == Aspect::Cold {
        s[0] = if matches!(tint, Aspect::Blunt | Aspect::Piercing) {
            ActiveState {
                kind: StateKind::Freeze,
                turns: 1,
                power: 0,
                source: 9,
            }
        } else {
            ActiveState {
                kind: StateKind::Chill,
                turns: 2,
                power: 6_000,
                source: 9,
            }
        };
    }
    s
}

/// One side of a resonance hit: the hit, then the spread on the cluster.
fn resonance_side(
    cfg: &BalanceConfig,
    tables: &ResonanceTables,
    input: &HitInput,
    src: &mut impl RollSource,
) -> (HitResult, Vec<Monster>, [BurningCell; BURNING_CELLS_MAX]) {
    let mut result = HitResult::default();
    resolve_hit_into(input, &cfg.combat, tables, src, &mut result);
    let mut ms = cluster(result.states);
    let units = ms.iter().map(|m| Cell { x: m.x, y: m.y }).collect();
    // The registry is read from disk once: the triple test resolves thousands of hits.
    static GOBLIN: std::sync::OnceLock<MonsterDefense> = std::sync::OnceLock::new();
    let goblin = *GOBLIN.get_or_init(|| program_defense(&sim_registry(), 1));
    let defense = vec![goblin; ms.len()];
    let mut cells = [BurningCell::default(); BURNING_CELLS_MAX];
    spread(
        cfg,
        &result,
        ATTACKER,
        0,
        &mut ms,
        &defense,
        &Open { units },
        &mut cells,
        1,
        0,
    );
    (result, ms, cells)
}

#[test]
fn every_resonance_pair_resolves_the_same_and_every_op_acts() {
    let cfg = sim_config();
    let registry = sim_registry();
    let (st, pt) = (registry.tables, program_tables(&registry));
    assert_eq!(st, pt);
    let mut pairs: Vec<(Aspect, Aspect, Option<Aspect>)> = Aspect::ALL
        .iter()
        .flat_map(|b| Aspect::ALL.iter().map(move |t| (*b, *t, None)))
        .filter(|(b, t, _)| b != t)
        .collect();
    assert_eq!(pairs.len(), 30);
    // A triple too: Piercing Frost of the Storm (docs 6.5).
    pairs.push((Aspect::Cold, Aspect::Piercing, Some(Aspect::Lightning)));
    // The triples pass: the pairs act the same with a triple record behind them (the triple
    // test below checks the records).
    for (base, tint, second) in pairs {
        let mechanics = st.pair(base, tint);
        assert!(
            !mechanics.generated,
            "{base:?} + {tint:?} has its own record"
        );
        let mut input = pair_input(&cfg, base, tint, second);
        input.defender.states = setup_states(base, tint);
        let mut triggered = 0;
        let mut acted = [false; deadroll_combat::RES_OPS_MAX];
        let mut kinds = [OpKind::None; deadroll_combat::RES_OPS_MAX];
        for i in 0..200u8 {
            let p = resonance_side(
                program_rules::BALANCE,
                &pt,
                &input,
                &mut SyscallRoll::new(seed(i)),
            );
            let s = resonance_side(&cfg, &st, &input, &mut VrfRoll::new(seed(i)));
            assert_eq!(s, p, "{base:?} + {tint:?} (+ {second:?}), seed {i}");
            let r = &s.0;
            let Some(res) = r.resonances[0].filter(|r| r.active) else {
                continue;
            };
            if second.is_some() && res.second_tint.is_none() {
                continue;
            }
            triggered += 1;
            for (k, op) in r.ops(0).enumerate() {
                kinds[k] = op.kind;
                acted[k] |= op.acted;
            }
        }
        assert!(triggered > 0, "{base:?} + {tint:?} never triggered");
        for (k, kind) in kinds
            .iter()
            .enumerate()
            .filter(|(_, k)| **k != OpKind::None)
        {
            assert!(
                acted[k],
                "{base:?} + {tint:?} (+ {second:?}): op {kind:?} never acted"
            );
        }
    }
}

/// States on the target before the hit besides `setup_states`: the ops of a triple that need one
/// (a frozen, shocked, burning, bleeding, stunned or penetrated target).
fn triple_setups(base: Aspect, tint: Aspect) -> Vec<[ActiveState; STATES_MAX]> {
    let mut out = vec![setup_states(base, tint)];
    for kind in [
        StateKind::Freeze,
        StateKind::Shock,
        StateKind::Burning,
        StateKind::Bleeding,
        StateKind::Stun,
        StateKind::Penetration,
    ] {
        let mut s = [ActiveState::default(); STATES_MAX];
        s[0] = ActiveState {
            kind,
            turns: 2,
            power: if kind == StateKind::Shock { 2_000 } else { 1 },
            source: 9,
        };
        out.push(s);
    }
    out
}

#[test]
fn every_triple_resolves_the_same_and_every_op_acts() {
    use deadroll_combat::tables::{triple_of, TRIPLES};
    let cfg = sim_config();
    let registry = sim_registry();
    let (st, pt) = (registry.tables, program_tables(&registry));
    assert_eq!(st, pt);
    assert_eq!(registry.triples.len(), deadroll_combat::tables::TRIPLES_LEN);
    for index in 0..TRIPLES {
        let [base, tint, second] = triple_of(index).unwrap();
        let record = st
            .triple(base, tint, second)
            .unwrap_or_else(|| panic!("{base:?} + {tint:?} + {second:?} has no record"));
        let mut acted = [false; deadroll_combat::RES_OPS_MAX];
        let mut kinds = [OpKind::None; deadroll_combat::RES_OPS_MAX];
        let mut origins = 0;
        for (n, states) in triple_setups(base, tint).into_iter().enumerate() {
            let mut input = pair_input(&cfg, base, tint, Some(second));
            input.defender.states = states;
            // The default setup gets more seeds: the others only fill the ops it cannot reach.
            let seeds = if n == 0 { 160u8 } else { 50 };
            for i in 0..seeds {
                let p = resonance_side(
                    program_rules::BALANCE,
                    &pt,
                    &input,
                    &mut SyscallRoll::new(seed(i)),
                );
                let s = resonance_side(&cfg, &st, &input, &mut VrfRoll::new(seed(i)));
                assert_eq!(
                    s, p,
                    "{base:?} + {tint:?} + {second:?}, setup {n}, seed {i}"
                );
                let r = &s.0;
                let Some(res) = r.resonances[0].filter(|r| r.active) else {
                    continue;
                };
                // The two tints can come out the other way round: that is another triple.
                if (res.tint, res.second_tint) != (tint, Some(second)) {
                    continue;
                }
                for (k, op) in r.ops(0).enumerate() {
                    kinds[k] = op.kind;
                    acted[k] |= op.acted;
                    if op.origin == deadroll_combat::OpOrigin::Triple {
                        origins += 1;
                    }
                }
            }
        }
        assert!(
            origins > 0,
            "{base:?} + {tint:?} + {second:?} never ran its record"
        );
        for (k, kind) in kinds
            .iter()
            .enumerate()
            .filter(|(_, k)| **k != OpKind::None)
        {
            assert!(
                acted[k],
                "{base:?} + {tint:?} + {second:?}: op {k} ({kind:?}) never acted; record {:?}",
                record.ops
            );
        }
    }
}

#[test]
fn a_miss_with_an_on_miss_triple_resolves_the_same_on_both_sides() {
    let cfg = sim_config();
    let registry = sim_registry();
    let (st, pt) = (registry.tables, program_tables(&registry));
    let mut input = pair_input(
        &cfg,
        Aspect::Piercing,
        Aspect::Lightning,
        Some(Aspect::Blunt),
    );
    input.defender.defense = 1_000;
    let mut shocked = 0;
    for i in 0..120u8 {
        let p = resonance_side(
            program_rules::BALANCE,
            &pt,
            &input,
            &mut SyscallRoll::new(seed(i)),
        );
        let s = resonance_side(&cfg, &st, &input, &mut VrfRoll::new(seed(i)));
        assert_eq!(s, p, "seed {i}");
        // A natural 20 always hits.
        if s.0.outcome == deadroll_combat::Outcome::Miss
            && deadroll_combat::states::has(&s.0.states, StateKind::Shock)
        {
            shocked += 1;
        }
    }
    assert!(shocked > 0, "Shock never landed on a miss");
}

#[test]
fn the_program_reads_the_monster_defenses_of_the_registry() {
    let registry = sim_registry();
    // Wolf: resists Cold; goblin: nothing.
    let wolf = program_profile(0);
    assert!(wolf.resist_bp[Aspect::Cold as usize] > 0);
    assert_eq!(program_profile(1).resist_bp, [0; Aspect::COUNT]);
    for id in 0..5u8 {
        assert_eq!(program_defense(&registry, id), registry.defense_of(id));
    }
}

// ---------------------------------------------------------------------------------------------
// Patch 5: map skills, passive skills and auras, reactive skills, the line target.

/// The demo set's skill of `category` (the first one), on both sides: the program's map skill
/// snapshot and the simulator's fighter.
fn both_sides(
    set: &[skill_presets::Preset],
    f: &Fighter,
    category: deadroll_skillcheck::Category,
) -> Option<(input::SkillStats, input::SkillStats)> {
    use deadroll_skillcheck::Category;
    let prog = set
        .iter()
        .find(|p| p.blocks.category == category)
        .map(record)?;
    let sim = match category {
        Category::Reactive => f.reactive.as_ref(),
        Category::Passive => f.passive.as_ref(),
        Category::Active => None,
    }?;
    Some((prog, sim.stats))
}

#[test]
fn map_skills_reactive_and_passive_snapshot_the_same() {
    use deadroll_session::state::{AuraData, MapSkillData};
    use deadroll_skillcheck::Category;
    let cfg = sim_config();
    for (c, set) in demo() {
        let f = sim_fighter(&c, set, &cfg, Gear::Kit);
        let ph = program_hero(&c, Gear::Kit);
        // Active skills: their own AP cost both ways (PARTY_AND_TURNS 5.1).
        let actives: Vec<input::SkillStats> = set
            .iter()
            .map(record)
            .filter(|r| program::skill_use(r) == program::SkillUse::Ok)
            .collect();
        for (p, s) in actives.iter().zip(&f.skills) {
            assert_eq!(p.ap_cost, s.stats.ap_cost);
            assert_eq!(
                program::action_ap(Some(p)),
                input::action_ap(&cfg, Some(&s.stats))
            );
            // The participation stores the snapshot and gives the same skill back.
            assert_eq!(input::SkillStats::from(&MapSkillData::from(p)), *p);
        }
        assert_eq!(program::action_ap(None), input::action_ap(&cfg, None));
        let (pr, sr) = both_sides(set, &f, Category::Reactive).expect("a reactive skill");
        assert_eq!(pr, sr);
        let (pp, sp) = both_sides(set, &f, Category::Passive).expect("a passive skill");
        assert_eq!(pp, sp);
        // The passive skill as the session keeps it (`set_ready`) and as the simulator does.
        let prog_aura = deadroll_rules::aura::passive_aura(program_rules::BALANCE, &ph, &pp)
            .map(AuraData::from)
            .map(deadroll_rules::aura::Aura::from);
        let sim_aura = deadroll_rules::aura::passive_aura(&cfg, &f.hero, &sp);
        assert!(sim_aura.is_some());
        assert_eq!(prog_aura, sim_aura);
    }
}

/// A `RollSource` over a closure (one body for both roll streams).
struct FnRoll<'a>(&'a mut dyn FnMut() -> u64);

impl RollSource for FnRoll<'_> {
    fn next_u64(&mut self) -> u64 {
        (self.0)()
    }
}

/// A counterattack of `skill` by `hero` on a fresh monster `id`: outcome, hit, monster, mods.
#[allow(clippy::too_many_arguments)]
fn counterattack(
    cfg: &BalanceConfig,
    tables: &ResonanceTables,
    hero: &HeroStats,
    skill: &input::SkillStats,
    profile: &MonsterProfile,
    id: u8,
    hp: u32,
    next: &mut dyn FnMut() -> u64,
) -> (resolve::AttackOutcome, HitResult, Monster, TimedMods) {
    let (mut m, mut mods) = (monster(id, hp), NONE);
    let (mut hin, mut hres) = (HitInput::default(), HitResult::default());
    let mut acc = AccuracyBuff::default();
    let out = resolve::attack_monster(
        cfg,
        tables,
        0,
        hero,
        Some(skill),
        &NONE,
        &mut acc,
        profile,
        &mut m,
        &mut mods,
        &NONE,
        &mut hin,
        &mut hres,
        &mut FnRoll(next),
    );
    (out, hres, m, mods)
}

#[test]
fn reactive_skills_resolve_the_same() {
    use deadroll_rules::{
        aura::Aura,
        reactive::{reaction, Event, Reaction},
    };
    use deadroll_session::state::AuraData;
    use deadroll_skillcheck::{Category, Trigger};
    let cfg = sim_config();
    let monsters = sim_monsters(&cfg);
    let registry = sim_registry();
    let (st, pt) = (registry.tables, program_tables(&registry));
    for (c, set) in demo() {
        let f = sim_fighter(&c, set, &cfg, Gear::Kit);
        let ph = program_hero(&c, Gear::Kit);
        let (pr, sr) = both_sides(set, &f, Category::Reactive).unwrap();
        let at = Cell { x: 4, y: 4 };
        let next_to = Cell { x: 5, y: 4 };
        // The event its trigger answers.
        let event = match pr.blocks.trigger {
            Trigger::Hit | Trigger::CritHit => Event::Hit { crit: true },
            Trigger::HpBelow => Event::HpDrop {
                before: c.hp_max,
                after: 1,
                hp_max: c.hp_max,
            },
            Trigger::EnemyAdjacent => Event::EnemyEntered,
            other => panic!("no demo reactive on {other:?}"),
        };
        let source = Some((0, next_to));
        let r = reaction(program_rules::BALANCE, &pr, 0, 0, at, &event, source, None);
        assert_eq!(r, reaction(&cfg, &sr, 0, 0, at, &event, source, None));
        for i in 0..SEEDS {
            match r.expect("the demo reactive fires") {
                Reaction::Strike { .. } => {
                    for &(id, profile, hp) in &monsters {
                        let mut p_src = SyscallRoll::new(seed(i));
                        let mut s_src = VrfRoll::new(seed(i));
                        let p = counterattack(
                            program_rules::BALANCE,
                            &pt,
                            &ph,
                            &pr,
                            &program_profile(id),
                            id,
                            hp,
                            &mut || p_src.next_u64(),
                        );
                        let s =
                            counterattack(&cfg, &st, &f.hero, &sr, &profile, id, hp, &mut || {
                                s_src.next_u64()
                            });
                        assert_eq!(s, p, "counterattack on {id}, seed {i}");
                    }
                }
                Reaction::Debuff { .. } => {
                    let effect = effect_action(pr.blocks.effect).unwrap();
                    for &(id, profile, hp) in &monsters {
                        let (mut pm, mut pmods) = (monster(id, hp), NONE);
                        let p = resolve::debuff_monster(
                            program_rules::BALANCE,
                            0,
                            &ph,
                            &pr,
                            effect,
                            &program_profile(id),
                            &mut pm,
                            &mut pmods,
                            &mut SyscallRoll::new(seed(i)),
                        );
                        let (mut sm, mut smods) = (monster(id, hp), NONE);
                        let s = resolve::debuff_monster(
                            &cfg,
                            0,
                            &f.hero,
                            &sr,
                            effect,
                            &profile,
                            &mut sm,
                            &mut smods,
                            &mut VrfRoll::new(seed(i)),
                        );
                        assert_eq!((s, sm, smods), (p, pm, pmods));
                    }
                }
                Reaction::Aura => {
                    let (proll, paura) = resolve::start_aura(
                        program_rules::BALANCE,
                        &ph,
                        &pr,
                        &mut SyscallRoll::new(seed(i)),
                    );
                    let (sroll, saura) =
                        resolve::start_aura(&cfg, &f.hero, &sr, &mut VrfRoll::new(seed(i)));
                    assert_eq!(sroll, proll);
                    // Through the session's account data and back.
                    assert_eq!(saura, paura.map(AuraData::from).map(Aura::from));
                    assert!(saura.is_some_and(|a| !a.passive && a.radius > 0));
                }
                Reaction::Buff { .. } => panic!("no demo reactive buffs without an aura"),
            }
        }
    }
}

#[test]
fn a_line_hits_the_same_units_in_the_same_order_on_the_same_dice() {
    use deadroll_rules::line::{line_cells, targets_on_line};
    use deadroll_skillcheck::Target;
    let cfg = sim_config();
    let registry = sim_registry();
    let (st, pt) = (registry.tables, program_tables(&registry));
    let (c, set) = &demo()[1];
    let f = sim_fighter(c, set, &cfg, Gear::Kit);
    let ph = program_hero(c, Gear::Kit);
    // Arc: the mage's lightning line.
    let arc = set
        .iter()
        .position(|p| p.blocks.target == Target::Line)
        .unwrap();
    let pskill = record(&set[arc]);
    let sskill = f
        .skills
        .iter()
        .find(|s| s.stats.blocks.target == Target::Line)
        .unwrap()
        .stats;
    assert_eq!(pskill, sskill);
    let reach = deadroll_rules::units::reach(pskill.ext.range, &cfg.players);
    assert_eq!(reach, program_rules::reach(pskill.ext.range));
    let line = line_cells(ATTACKER, Cell { x: 5, y: 5 }, reach, |c| {
        c.x < 12 && c.y < 12
    });
    let goblin = sim_monsters(&cfg)[1].1;
    for i in 0..SEEDS {
        let mut sides = Vec::new();
        for program_side in [true, false] {
            let mut ms = cluster([ActiveState::default(); STATES_MAX]);
            let units: Vec<(u8, Cell)> = ms
                .iter()
                .enumerate()
                .map(|(k, m)| (k as u8, Cell { x: m.x, y: m.y }))
                .collect();
            let (targets, n) = targets_on_line(&line, units.iter().copied());
            // The target, the one right behind it, the one further on the line.
            assert_eq!(&targets[..n], &[0, 1, 4]);
            let mut src_p = SyscallRoll::new(seed(i));
            let mut src_s = VrfRoll::new(seed(i));
            let mut results = Vec::new();
            for &k in &targets[..n] {
                let mut mods = NONE;
                let (mut hin, mut hres) = (HitInput::default(), HitResult::default());
                let mut acc = AccuracyBuff::default();
                let out = if program_side {
                    resolve::attack_monster(
                        program_rules::BALANCE,
                        &pt,
                        0,
                        &ph,
                        Some(&pskill),
                        &NONE,
                        &mut acc,
                        &program_profile(1),
                        &mut ms[k as usize],
                        &mut mods,
                        &NONE,
                        &mut hin,
                        &mut hres,
                        &mut src_p,
                    )
                } else {
                    resolve::attack_monster(
                        &cfg,
                        &st,
                        0,
                        &f.hero,
                        Some(&sskill),
                        &NONE,
                        &mut acc,
                        &goblin,
                        &mut ms[k as usize],
                        &mut mods,
                        &NONE,
                        &mut hin,
                        &mut hres,
                        &mut src_s,
                    )
                };
                results.push((out, hres.damage, hres.rolls_used, mods));
            }
            sides.push((results, ms));
        }
        assert_eq!(sides[0], sides[1], "seed {i}");
    }
}

// -------------------------------------------------------------------------------------------
// Patch 6: monster skills. The program copies a spawn's pool skills into `MonsterState` (the
// 16-byte session copy) and derives them with `BALANCE.skills`; the simulator spawns the same
// registry records with the JSON config.

/// The program's view of a pool skill: the session copy `open_session` writes, read back as
/// `Monsters::skill` does.
fn program_skill(
    k: &deadroll_skillcheck::monster::MonsterSkill,
) -> deadroll_rules::MonsterSkillStats {
    let copy = k.encode_session();
    let back = deadroll_skillcheck::monster::MonsterSkill::decode_session(&copy).unwrap();
    deadroll_rules::MonsterSkillStats::new(&back, &program_rules::BALANCE.skills)
}

/// The registry index of the skill named `name`.
fn skill_index(reg: &Registry, name: &str) -> usize {
    reg.skill_names.iter().position(|n| n == name).unwrap()
}

#[test]
fn monster_skills_snapshot_the_same() {
    let cfg = sim_config();
    let reg = sim_registry();
    for id in 0..5u8 {
        for elite in [false, true] {
            let s = spawn(&reg, id, 1, elite, None, &cfg).unwrap();
            let t = program_template(id);
            let pool = reg.pool(&t);
            let cats: Vec<_> = pool.iter().map(|k| k.unwrap().blocks.category).collect();
            let flags = if elite {
                deadroll_types::SPAWN_ELITE
            } else {
                0
            };
            let slots = t.slots(flags, &program_rules::BALANCE.monsters);
            let mask = deadroll_sim::setup::default_skills(&cats, slots);
            let chosen = deadroll_rules::monster::select_skills(&cats, mask, slots);
            for (slot, &i) in chosen.iter().enumerate() {
                let prog = (i != deadroll_rules::monster::NO_SKILL)
                    .then(|| program_skill(&pool[i as usize].unwrap()));
                assert_eq!(
                    s.skills[slot], prog,
                    "monster {id} elite {elite} slot {slot}"
                );
            }
        }
    }
}

/// A monster's skill (`None`: the natural attack) on a fresh hero: the hit, the rider on the
/// player's stored modifiers, HP and states.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn monster_hit(
    cfg: &BalanceConfig,
    tables: &ResonanceTables,
    profile: &MonsterProfile,
    skill: Option<&deadroll_rules::MonsterSkillStats>,
    hero: &HeroStats,
    src: &mut impl RollSource,
) -> (
    HitResult,
    u32,
    [ActiveState; STATES_MAX],
    TimedMods,
    resolve::AttackOutcome,
) {
    let (mut hp, mut states, mut own) = (30u32, [ActiveState::default(); STATES_MAX], NONE);
    let (mut inp, mut res) = (HitInput::default(), HitResult::default());
    let out = resolve::monster_attack(
        cfg,
        tables,
        deadroll_rules::MONSTER_UNIT_BASE,
        profile,
        skill.map(|k| &k.stats),
        &NONE,
        hero,
        &mut hp,
        &mut states,
        &mut own,
        &NONE,
        &mut inp,
        &mut res,
        src,
    );
    (res, hp, states, own, out)
}

/// Every damage skill of every shipped monster on the warrior, on the same dice.
#[test]
fn monster_skill_hits_resolve_the_same() {
    let cfg = sim_config();
    let reg = sim_registry();
    let (st, pt) = (reg.tables, program_tables(&reg));
    let (warrior, _) = &demo()[0];
    let wf = sim_fighter(warrior, &skill_presets::WARRIOR, &cfg, Gear::Kit);
    let ph = program_hero(warrior, Gear::Kit);
    let mut riders = 0;
    for id in 0..5u8 {
        let s = spawn(&reg, id, 1, false, None, &cfg).unwrap();
        let sim_profile = s.template.profile(1, &cfg.monsters);
        let prog_profile = program_profile(id);
        assert_eq!(sim_profile, prog_profile);
        for (k, name) in s.skills.iter().zip(&s.skill_names) {
            let Some(k) = k else { continue };
            if k.stats.blocks.purpose != Purpose::Damage {
                continue;
            }
            let prog_skill = program_skill(&reg.skills[skill_index(&reg, name)].unwrap());
            assert_eq!(prog_skill, *k, "{name}");
            let unit = deadroll_rules::MONSTER_UNIT_BASE;
            assert_eq!(
                input::monster_attacker(&cfg, unit, &sim_profile, Some(&k.stats), &NONE),
                program::monster_attacker(unit, &prog_profile, Some(&prog_skill.stats), &NONE)
            );
            for i in 0..SEEDS {
                let p = monster_hit(
                    program_rules::BALANCE,
                    &pt,
                    &prog_profile,
                    Some(&prog_skill),
                    &ph,
                    &mut SyscallRoll::new(seed(i)),
                );
                let q = monster_hit(
                    &cfg,
                    &st,
                    &sim_profile,
                    Some(k),
                    &wf.hero,
                    &mut VrfRoll::new(seed(i)),
                );
                assert_eq!(p, q, "{name} seed {i}");
                riders += p.4.rider.is_some() as u32;
            }
        }
    }
    assert!(riders > 0, "no skill rider rolled");
}

/// The chieftain's Net Throw on the warrior and the shaman's Mending and Quicken on a goblin:
/// the same rolls, the same landing.
#[test]
fn monster_debuffs_and_support_resolve_the_same() {
    let cfg = sim_config();
    let reg = sim_registry();
    let (warrior, _) = &demo()[0];
    let wf = sim_fighter(warrior, &skill_presets::WARRIOR, &cfg, Gear::Kit);
    let ph = program_hero(warrior, Gear::Kit);
    let named = |id: u8, name: &str| {
        let s = spawn(&reg, id, 1, false, None, &cfg).unwrap();
        let slot = s.skill_names.iter().position(|n| n == name).unwrap();
        let k = s.skills[slot].unwrap();
        (s, k)
    };
    let (chief, net) = named(2, "Net Throw");
    let prog_net = program_skill(&reg.skills[skill_index(&reg, "Net Throw")].unwrap());
    assert_eq!(prog_net, net);
    let effect = effect_action(net.stats.blocks.effect).unwrap();
    let mut landed = 0;
    for i in 0..SEEDS {
        let (mut ps, mut ss) = (
            [ActiveState::default(); STATES_MAX],
            [ActiveState::default(); STATES_MAX],
        );
        let (mut pm, mut sm) = (NONE, NONE);
        let p = resolve::monster_debuff_player(
            program_rules::BALANCE,
            6,
            &program_profile(2),
            &prog_net.stats,
            effect,
            &ph,
            30,
            &mut ps,
            &mut pm,
            &NONE,
            &mut SyscallRoll::new(seed(i)),
        );
        let s = resolve::monster_debuff_player(
            &cfg,
            6,
            &chief.template.profile(1, &cfg.monsters),
            &net.stats,
            effect,
            &wf.hero,
            30,
            &mut ss,
            &mut sm,
            &NONE,
            &mut VrfRoll::new(seed(i)),
        );
        assert_eq!((p, ps, pm), (s, ss, sm), "seed {i}");
        landed += (p.applied != deadroll_combat::Applied::None) as u32;
    }
    assert!(landed > 0);
    for name in ["Mending", "Quicken"] {
        let (shaman, k) = named(4, name);
        let effect = effect_action(k.stats.blocks.effect).unwrap();
        let prog = program_skill(&reg.skills[skill_index(&reg, name)].unwrap());
        assert_eq!(prog, k);
        for i in 0..SEEDS {
            let (mut pg, mut sg) = (monster(1, 12), monster(1, 12));
            pg.hp = 3;
            sg.hp = 3;
            let (mut pm, mut sm) = (NONE, NONE);
            let p = resolve::monster_support(
                program_rules::BALANCE,
                8,
                &program_profile(4),
                &prog.stats,
                effect,
                &mut pg,
                &mut pm,
                &mut SyscallRoll::new(seed(i)),
            );
            let s = resolve::monster_support(
                &cfg,
                8,
                &shaman.template.profile(1, &cfg.monsters),
                &k.stats,
                effect,
                &mut sg,
                &mut sm,
                &mut VrfRoll::new(seed(i)),
            );
            assert_eq!((p, pg, pm), (s, sg, sm), "{name} seed {i}");
        }
    }
}

/// Monster auras (the chieftain's Leader and Battle Cry) and the monster AI give the same on
/// both sides.
#[test]
fn monster_auras_and_ai_are_the_same() {
    use deadroll_rules::{
        aura::monster_passive_aura,
        monster_ai::{plan, AiMonster, AiUnit},
        resolve::monster_start_aura,
    };
    let cfg = sim_config();
    let reg = sim_registry();
    let chief = spawn(&reg, 2, 1, false, None, &cfg).unwrap();
    let leader = chief.skills[deadroll_rules::monster::PASSIVE_SLOT].unwrap();
    let cry = chief.skills[deadroll_rules::monster::REACTIVE_SLOT].unwrap();
    let sp = chief.template.profile(1, &cfg.monsters);
    let pp = program_profile(2);
    assert_eq!(
        monster_passive_aura(&cfg, &sp, &leader),
        monster_passive_aura(program_rules::BALANCE, &pp, &leader)
    );
    for i in 0..SEEDS {
        assert_eq!(
            monster_start_aura(&cfg, &sp, &cry, &mut VrfRoll::new(seed(i))),
            monster_start_aura(
                program_rules::BALANCE,
                &pp,
                &cry,
                &mut SyscallRoll::new(seed(i))
            )
        );
    }
    let shaman = spawn(&reg, 4, 1, false, None, &cfg).unwrap();
    let cooldowns = [0u8; deadroll_types::MONSTER_SLOTS];
    let kin = shaman.template.kin;
    let unit = |x, hp| AiUnit {
        cell: Cell { x, y: 5 },
        hp,
        hp_max: 10,
        alive: true,
        kin,
        ..AiUnit::default()
    };
    let heroes = [unit(9, 10)];
    let sp = shaman.template.profile(1, &cfg.monsters);
    let pp = program_profile(4);
    for hurt in [2, 9] {
        let monsters = [unit(5, 10), unit(6, hurt)];
        let me = |profile| AiMonster {
            index: 0,
            ap: 3,
            profile,
            skills: &shaman.skills,
            cooldowns: &cooldowns,
        };
        assert_eq!(
            plan(&cfg, &me(&sp), &heroes, &monsters),
            plan(program_rules::BALANCE, &me(&pp), &heroes, &monsters)
        );
    }
}
