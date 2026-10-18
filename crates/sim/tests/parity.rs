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
use deadroll_character::state::SkillRecord;
use deadroll_combat::{
    resolve_hit, resolve_hit_into, AccuracyBuff, HitInput, HitResult, ModKind, OpKind,
    ResonanceTables, RollSource, TimedMod, TimedMods, VrfRoll,
};
use deadroll_rules::{
    input::{self, effect_action, EffectAction},
    items::Equipment,
    monster::p0,
    registry::Registry,
    resolve,
    spread::{spread, BurningCell, Field, BURNING_CELLS_MAX},
    BalanceConfig, HeroStats, MonsterDefense, MonsterProfile,
};
use deadroll_session::{
    combat_input as program, rules as program_rules, state::participation::hero_of,
    vrf::SyscallRoll,
};
use deadroll_sim::{
    config_io,
    presets::{HeroPreset, MonsterPreset, Presets, SkillPreset},
    setup::{fighter, Fighter, Issues},
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
}; deadroll_combat::MODS_MAX];

const SEEDS: u8 = 64;

/// The simulator's registry: `assets/registry` through its JSON loader.
fn sim_registry() -> Registry {
    Registry::shipped().unwrap()
}

/// The program's view of the same data: the table bodies decoded as `Monsters::tables_into`
/// does, and the defenses as `read_bestiary` reads them.
fn program_tables(r: &Registry) -> ResonanceTables {
    ResonanceTables::from_bytes(&r.aspects, &r.traits, &r.resonances).unwrap()
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

/// What `create_character` stores for a preset skill.
fn record(p: &skill_presets::Preset) -> SkillRecord {
    let d = derive(&p.blocks);
    SkillRecord {
        blocks: p.blocks,
        shares_bp: d.shares_bp,
        power_bp: d.power_bp,
        fusion: d.fusion,
        cooldown: d.cooldown,
        ..SkillRecord::default()
    }
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
            round_trip(&SkillPreset::from_blocks(p.name, &p.blocks)),
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

/// P0 monsters as JSON presets, at level 1, with their registry defenses.
fn sim_monsters(cfg: &BalanceConfig) -> Vec<(u8, MonsterProfile, u32)> {
    let registry = sim_registry();
    (0..5u8)
        .map(|id| {
            let preset: MonsterPreset = round_trip(&MonsterPreset::from_base(
                "m",
                Some(id),
                &p0::monster_base(id).unwrap(),
            ));
            let base = preset.base();
            let defense = preset.defense(&registry);
            (id, base.profile(1, &cfg.monsters, &defense), base.hp_at(1))
        })
        .collect()
}

/// The program's profile of monster `id` at level 1.
fn program_profile(id: u8) -> MonsterProfile {
    program_rules::monster_profile(id, 1, &program_defense(&sim_registry(), id)).unwrap()
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
    let died = resolve::monster_attack(
        cfg,
        tables,
        deadroll_rules::MONSTER_UNIT_BASE,
        profile,
        &NONE,
        hero,
        &mut hp,
        &mut states,
        &NONE,
        &mut input,
        &mut result,
        src,
    );
    (result, hp, states, died)
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
        let base = p0::monster_base(id).unwrap();
        let record = deadroll_session::state::external::BestiaryRecord {
            speed: base.speed,
            initiative: base.initiative,
            hp: base.hp,
            hp_per_level: base.hp_per_level,
            defense: program_defense(&sim_registry(), id),
        };
        assert_eq!(hp, program_rules::monster_hp(&record, 1));
        assert_eq!(
            deadroll_rules::monster::monster_ap(base.speed, &cfg.monsters),
            program_rules::monster_ap(base.speed)
        );
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
        let usable: Vec<SkillRecord> = set
            .iter()
            .map(record)
            .filter(|r| program::skill_use(r) == program::SkillUse::Ok)
            .collect();
        assert_eq!(f.skills.len(), usable.len());
        for (sim, prog) in f.skills.iter().zip(&usable) {
            assert_eq!(sim.stats, program::skill_stats(prog));
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
        let records: Vec<SkillRecord> = set
            .iter()
            .map(record)
            .filter(|r| program::skill_use(r) == program::SkillUse::Ok)
            .collect();
        let mut choices: Vec<(Option<SkillRecord>, Option<input::SkillStats>)> = vec![(None, None)];
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
                        &program_defense(&registry, id),
                        &NONE,
                    )
                    .unwrap(),
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
                        rec.as_ref().map(program::skill_stats).as_ref(),
                        &NONE,
                        &mut p_acc,
                        &program_profile(id),
                        &mut pm,
                        &mut pmods,
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
        let prog = program::monster_attacker(
            unit,
            &monster(id, hp),
            &program_defense(&registry, id),
            &NONE,
        )
        .unwrap();
        assert_eq!(input::monster_attacker(&cfg, unit, &profile, &NONE), prog);
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
        let records: Vec<SkillRecord> = set
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
                                &program::skill_stats(rec),
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
                            &program::skill_stats(rec),
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
    let hp = p0::monster_base(1).unwrap().hp_at(1);
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
    let skill = input::SkillStats::derive(blocks, &cfg.skills);
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
/// for the other Cold pairs (their Freeze ops).
fn setup_states(base: Aspect, tint: Aspect) -> [ActiveState; STATES_MAX] {
    let mut s = [ActiveState::default(); STATES_MAX];
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
    let defense = vec![program_defense(&sim_registry(), 1); ms.len()];
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
