//! Parity: the simulator and the session program give the same results on the same dice.
//!
//! The program side is the program's own code (`deadroll_session::combat_input`, `rules`, the
//! `SyscallRoll` of the VRF callback) fed with its tables and the character program's records.
//! The simulator side starts from JSON presets (round-tripped, so edits to the preset folder do
//! not move this test) and the default `BalanceConfig` loaded from JSON, and rolls with
//! `VrfRoll` (`crates/combat`), the same stream as `SyscallRoll`.
//!
//! Equipment: the program side wears what the character program's own gear code gives
//! (`deadroll_character::state::Gear`: the starter kit, then unequip), snapshotted the way
//! `set_ready` does (`Participation::hero`); the simulator side wears the hero preset's
//! `equipment` (left out = the starter kit). Every hero fights in each [`Gear`].
use deadroll_character::state::Gear as CharacterGear;
use deadroll_character::state::SkillRecord;
use deadroll_combat::{
    resolve_hit, resolve_hit_into, HitInput, HitResult, ModKind, RollSource, TimedMod, TimedMods,
    VrfRoll,
};
use deadroll_rules::{
    input::{self, effect_action, EffectAction},
    items::Equipment,
    monster::p0,
    resolve, BalanceConfig, HeroStats, MonsterProfile,
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
    ActiveState, Aspect, CharacterData, Class, EquipSlot, ItemBase, Monster, Race, STATES_MAX,
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
    hero_of(c, &l.equipment())
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
        skills: ids,
        inline_skills: vec![],
        equipment: preset_equipment(c.class, gear),
    });
    let mut issues = Issues::default();
    let f = fighter(&presets, "demo", &hero, cfg, &mut issues);
    assert!(issues.errors.is_empty(), "{:?}", issues.errors);
    f
}

/// P0 monsters as JSON presets, at level 1.
fn sim_monsters(cfg: &BalanceConfig) -> Vec<(u8, MonsterProfile, u32)> {
    (0..5u8)
        .map(|id| {
            let preset: MonsterPreset = round_trip(&MonsterPreset::from_base(
                "m",
                Some(id),
                &p0::monster_base(id).unwrap(),
            ));
            let base = preset.base();
            (id, base.profile(1, &cfg.monsters), base.hp_at(1))
        })
        .collect()
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
    profile: &MonsterProfile,
    hero: &deadroll_rules::HeroStats,
    hp: u32,
    src: &mut impl RollSource,
) -> (HitResult, u32, [ActiveState; STATES_MAX], bool) {
    let (mut hp, mut states) = (hp, [ActiveState::default(); STATES_MAX]);
    let (mut input, mut result) = (HitInput::default(), HitResult::default());
    let died = resolve::monster_attack(
        cfg,
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
    for (input, s, damage) in [
        (golden::warrior_vs_wolf(), golden::SEED, 8),
        (golden::mage_vs_goblin(), golden::SEED, 13),
        (golden::mage_vs_goblin(), golden::RESONANCE_SEED, 14),
    ] {
        let mut on_chain = HitResult::default();
        resolve_hit_into(
            &input,
            &program_rules::BALANCE.combat,
            &mut SyscallRoll::new(s),
            &mut on_chain,
        );
        let sim = resolve_hit(&input, &cfg.combat, &mut VrfRoll::new(s));
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
    }
    for (id, profile, hp) in sim_monsters(&cfg) {
        assert_eq!(
            Some(profile),
            program_rules::monster_profile(id, 1),
            "monster {id}"
        );
        let base = p0::monster_base(id).unwrap();
        let record = deadroll_session::state::external::BestiaryRecord {
            speed: base.speed,
            initiative: base.initiative,
            hp: base.hp,
            hp_per_level: base.hp_per_level,
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
        assert_eq!(f.hero, HeroStats::from(&c).with_equipment(f.hero.equipment));
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
                let prog_input = HitInput {
                    attacker: program::player_attacker(0, &ph, rec.as_ref(), &NONE),
                    defender: program::monster_defender(&monster(id, hp), &NONE).unwrap(),
                };
                let sim_input = HitInput {
                    attacker: input::player_attacker(&cfg, 0, &f.hero, stats.as_ref(), &NONE),
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
                    let p_out = resolve::attack_monster(
                        program_rules::BALANCE,
                        0,
                        &ph,
                        rec.as_ref().map(program::skill_stats).as_ref(),
                        &NONE,
                        &program_rules::monster_profile(id, 1).unwrap(),
                        &mut pm,
                        &mut pmods,
                        &mut pin,
                        &mut pres,
                        &mut SyscallRoll::new(seed(i)),
                    );
                    // Simulator: presets, the JSON table and `VrfRoll`.
                    let (mut sm, mut smods) = (monster(id, hp), NONE);
                    let (mut sin, mut sres) = (HitInput::default(), HitResult::default());
                    let s_out = resolve::attack_monster(
                        &cfg,
                        0,
                        &f.hero,
                        stats.as_ref(),
                        &NONE,
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
                    assert_eq!((sm, smods), (pm, pmods));
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
        let prog = program::monster_attacker(unit, &monster(id, hp), &NONE).unwrap();
        assert_eq!(input::monster_attacker(&cfg, unit, &profile, &NONE), prog);
        let ph = program_hero(warrior, gear);
        let states = [ActiveState::default(); STATES_MAX];
        assert_eq!(
            input::player_defender(&cfg, &wf.hero, &states, &NONE),
            program::player_defender(&ph, &states, &NONE)
        );
        let program_profile = program_rules::monster_profile(id, 1).unwrap();
        for i in 0..SEEDS {
            let p = bite(
                program_rules::BALANCE,
                &program_profile,
                &ph,
                program_rules::party_hp_max(warrior.hp_max, &ph),
                &mut SyscallRoll::new(seed(i)),
            );
            let s = bite(
                &cfg,
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
                                &program_rules::monster_profile(id, 1).unwrap(),
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
                    for i in 0..SEEDS {
                        let start = c.hp_max / 3;
                        let mut ally = (start, [ActiveState::default(); STATES_MAX], NONE);
                        let p = resolve::support_ally(
                            program_rules::BALANCE,
                            0,
                            &ph,
                            &program::skill_stats(rec),
                            effect,
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
                            &mut sim_ally.0,
                            f.hp_max,
                            &mut sim_ally.1,
                            &mut sim_ally.2,
                            &mut VrfRoll::new(seed(i)),
                        );
                        assert_eq!((s, sim_ally), (p, ally));
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
    let weapon = |h: &HeroStats| program::player_attacker(0, h, None, &NONE).weapon;
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
