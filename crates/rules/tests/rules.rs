//! The shared rules: the P0 table against the registry data, scaling, and the config driving
//! every number. The program's own tests (programs/session) pin the default values.
use deadroll_combat::{ModKind, SeededRoll, TimedMod, TimedMods};
use deadroll_rules::{
    input::{
        effect_action, effect_range, land_effect, player_attacker, skill_use, target_side,
        EffectAction, HeroStats, Side, SkillStats, SkillUse,
    },
    monster::{monster_ap, monster_hp, p0},
    player::{class_increase, hp_max, player_ap, stats_valid},
    start_of_turn,
    units::{in_range, roll_initiative},
    BalanceConfig, Unit,
};
use deadroll_skillcheck::{presets, Effect, Purpose};
use deadroll_types::{ActiveState, Aspect, Cell, Class, Form, Race, StateKind, STATES_MAX};

const NONE: TimedMods = [TimedMod {
    kind: ModKind::Haste,
    aspect: Aspect::Fire,
    turns: 0,
    power_bp: 0,
    source: 0,
}; deadroll_combat::MODS_MAX];

#[test]
fn p0_table_matches_the_registry_bestiary() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/registry/bestiary.json"
    );
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let records = json["records"].as_array().unwrap();
    assert!(records.len() >= 5);
    for r in records {
        let id = r["id"].as_u64().unwrap() as u8;
        let Some(base) = p0::monster_base(id) else {
            continue;
        };
        assert_eq!(base.speed as u64, r["speed"].as_u64().unwrap(), "{id}");
        assert_eq!(
            base.initiative as u64,
            r["initiative"].as_u64().unwrap(),
            "{id}"
        );
        assert_eq!(base.hp as u64, r["hp"].as_u64().unwrap(), "{id}");
        assert_eq!(
            base.hp_per_level as u64,
            r["hp_per_level"].as_u64().unwrap(),
            "{id}"
        );
        assert_eq!(base.boss, r["role"] == "Boss", "{id}");
    }
    assert!(p0::monster_base(5).is_none());
}

#[test]
fn monsters_grow_with_level_and_scaling() {
    let s = BalanceConfig::DEFAULT.monsters;
    let wolf = p0::monster_base(0).unwrap();
    let l0 = wolf.profile(0, &s);
    let l3 = wolf.profile(3, &s);
    // min + 3/2, max + 3, attack + 3.
    assert_eq!(
        (l3.weapon.min, l3.weapon.max),
        (l0.weapon.min + 1, l0.weapon.max + 3)
    );
    assert_eq!(l3.attack_bonus, l0.attack_bonus + 3);
    assert_eq!((l0.xp, l3.xp), (10, 30));
    assert_eq!(l3.resist_bp[Aspect::Cold as usize], 5_000);
    let flat = deadroll_rules::MonsterScaling {
        weapon_min_per_level_bp: 0,
        weapon_max_per_level_bp: 0,
        attack_per_level_bp: 0,
        ..s
    };
    assert_eq!(wolf.profile(9, &flat).weapon, l0.weapon);
    assert_eq!(monster_hp(12, 3, 2), 18);
    assert_eq!(monster_hp(0, 0, 9), 1);
    assert_eq!(monster_ap(0, &s), 1);
    assert_eq!(monster_ap(200, &s), 12);
}

#[test]
fn the_config_drives_the_rules() {
    let mut cfg = BalanceConfig::DEFAULT;
    assert_eq!(player_ap(Race::ForestElf, Class::Mage, 20, &cfg.players), 6);
    cfg.players.ap_base[Race::ForestElf as usize][Class::Mage as usize] = 7;
    cfg.players.ap_dex_div = 5;
    assert_eq!(
        player_ap(Race::ForestElf, Class::Mage, 20, &cfg.players),
        11
    );
    // A zero divisor is treated as 1, never a panic.
    cfg.players.ap_dex_div = 0;
    assert_eq!(player_ap(Race::Human, Class::Warrior, 3, &cfg.players), 7);

    assert_eq!(class_increase(Class::Support, &cfg.players), None);
    cfg.players.class_increase_bp[Class::Support as usize] = 1_500;
    assert!(class_increase(Class::Support, &cfg.players).is_some());

    cfg.players.strike_range = 2;
    assert!(in_range(
        Form::Strike,
        Cell { x: 0, y: 0 },
        Cell { x: 2, y: 1 },
        &cfg.players
    ));
    assert_eq!(
        hp_max(Class::Warrior, &[5, 5, 5, 10], 3, &cfg.players),
        30 + 20 + 12
    );
    assert!(stats_valid(&[10, 5, 5, 10], &cfg.players));
    cfg.players.stat_points_start = 12;
    assert!(!stats_valid(&[10, 5, 5, 10], &cfg.players));

    // Skill numbers come from the skill table.
    let hero = HeroStats {
        race: Race::Human,
        class: Class::Warrior,
        level: 1,
        stats: [10, 7, 5, 8],
        predisposition: Aspect::Piercing,
        predisposition_tier: 14,
    };
    let cleave = presets::WARRIOR[3].blocks;
    let a = player_attacker(
        &BalanceConfig::DEFAULT,
        0,
        &hero,
        Some(&SkillStats::derive(cleave, &cfg.skills)),
        &NONE,
    );
    cfg.skills.line_counter_bp = 0;
    let b = player_attacker(
        &cfg,
        0,
        &hero,
        Some(&SkillStats::derive(cleave, &cfg.skills)),
        &NONE,
    );
    assert!(b.skill.power_bp > a.skill.power_bp);
}

#[test]
fn effects_map_to_actions_and_sides() {
    for p in presets::WARRIOR
        .iter()
        .chain(&presets::MAGE)
        .chain(&presets::SUPPORT)
    {
        let s = SkillStats::derive(p.blocks, &BalanceConfig::DEFAULT.skills);
        let side = target_side(Some(&s));
        assert_eq!(
            side == Side::Ally,
            p.blocks.purpose == Purpose::Buff,
            "{}",
            p.name
        );
        if skill_use(&s) == SkillUse::Ok && p.blocks.purpose != Purpose::Damage {
            assert!(effect_action(p.blocks.effect).is_some(), "{}", p.name);
        }
    }
    assert_eq!(target_side(None), Side::Enemy);
    assert_eq!(effect_action(Effect::None), None);
    let cfg = BalanceConfig::DEFAULT;
    assert_eq!(
        effect_range(&cfg, Class::Mage, 10_000, EffectAction::Stun),
        (0, 0)
    );
    let mut states = [ActiveState::default(); STATES_MAX];
    let mut mods = NONE;
    let mut long = cfg;
    long.support.mod_duration[ModKind::Slow as usize] = 4;
    land_effect(
        &long,
        EffectAction::Mod(ModKind::Slow),
        3_000,
        Aspect::Cold,
        1,
        &mut states,
        &mut mods,
    );
    assert_eq!(mods[0].turns, deadroll_combat::mod_turns(4));
    long.combat.state_turns[StateKind::Stun as usize] = 3;
    land_effect(
        &long,
        EffectAction::Stun,
        0,
        Aspect::Blunt,
        1,
        &mut states,
        &mut mods,
    );
    assert!(states
        .iter()
        .any(|s| s.kind == StateKind::Stun && s.turns == 3));
}

#[test]
fn turn_start_and_initiative_are_shared() {
    let mut states = [ActiveState::default(); STATES_MAX];
    states[0] = ActiveState {
        kind: StateKind::Bleeding,
        turns: 2,
        power: 4,
        source: 1,
    };
    states[1] = ActiveState {
        kind: StateKind::Stun,
        turns: 1,
        power: 0,
        source: 1,
    };
    let mut cooldowns = [2u8, 0];
    let t = start_of_turn(&mut states, &mut NONE.clone(), &mut cooldowns, 10);
    assert_eq!(
        (t.dot_damage, t.hp, t.died, t.skip, t.dot_source),
        (4, 6, false, Some(StateKind::Stun), 1)
    );
    assert_eq!(cooldowns, [1, 0]);
    let units = [(Unit::Player(0).id(), 5), (Unit::Monster(0).id(), 5)];
    let a = roll_initiative(&units, &mut SeededRoll::new(3));
    assert_eq!(a, roll_initiative(&units, &mut SeededRoll::new(3)));
    assert_eq!(a.len, 2);
}

#[test]
#[cfg(feature = "serde")]
fn config_serializes() {
    let cfg = BalanceConfig::DEFAULT;
    let text = serde_json::to_string(&cfg).unwrap();
    assert!(text.contains("\"armor_k\":100"));
    assert_eq!(serde_json::from_str::<BalanceConfig>(&text).unwrap(), cfg);
}
