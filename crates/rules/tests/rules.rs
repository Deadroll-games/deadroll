//! The shared rules: the registry bestiary templates, scaling, and the config driving
//! every number. The program's own tests (programs/session) pin the default values.
use deadroll_combat::{AccuracyBuff, ModKind, SeededRoll, TimedMod, TimedMods};
use deadroll_rules::{
    input::{
        effect_action, effect_range, land_effect, player_attacker, skill_use, target_side,
        EffectAction, HeroStats, Side, SkillStats, SkillUse,
    },
    monster::{monster_ap, monster_hp, MonsterTemplate, Rank},
    player::{class_increase, hp_max, player_ap, stats_valid},
    start_of_turn,
    units::{in_range, roll_initiative},
    BalanceConfig, Unit,
};
use deadroll_skillcheck::{presets, Effect, Purpose};
use deadroll_types::{ActiveState, Aspect, Cell, Class, Race, RangeTag, StateKind, STATES_MAX};

const NONE: TimedMods = [TimedMod {
    kind: ModKind::Haste,
    aspect: Aspect::Fire,
    turns: 0,
    power_bp: 0,
    source: 0,
}; deadroll_combat::MODS_MAX];

#[cfg(feature = "serde")]
#[test]
fn every_bestiary_template_decodes_with_one_boss() {
    let reg = deadroll_rules::registry::Registry::shipped().unwrap();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/registry/bestiary.json"
    );
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let records = json["records"].as_array().unwrap();
    assert!(records.len() >= 5);
    let mut bosses = 0;
    for r in records {
        let id = r["id"].as_u64().unwrap() as u8;
        let t = reg.template(id).unwrap();
        assert_eq!(t.speed as u64, r["speed"].as_u64().unwrap(), "{id}");
        assert_eq!(t.accuracy as u64, r["accuracy"].as_u64().unwrap(), "{id}");
        assert_eq!(t.evasion as u64, r["evasion"].as_u64().unwrap(), "{id}");
        assert_eq!(t.armor as u64, r["armor"].as_u64().unwrap(), "{id}");
        assert_eq!(t.xp as u64, r["xp"].as_u64().unwrap(), "{id}");
        assert_eq!(
            t.weapon.min as u64,
            r["damage_min"].as_u64().unwrap(),
            "{id}"
        );
        assert_eq!(
            t.weapon.max as u64,
            r["damage_max"].as_u64().unwrap(),
            "{id}"
        );
        assert_eq!(t.is_boss(), r["role"] == "Boss", "{id}");
        bosses += t.is_boss() as u32;
        // Every pool skill exists and passed the monster builder.
        assert!(reg.pool(&t).iter().all(|s| s.is_some()), "{id}");
        assert_eq!(reg.monster_names[id as usize], r["name"].as_str().unwrap());
    }
    assert_eq!(bosses, 1);
    assert_eq!(reg.template(5), None);
    assert_eq!(reg.skill_names.len(), reg.skills.len());
}

#[cfg(feature = "serde")]
#[test]
fn monsters_grow_with_level_and_scaling() {
    let s = BalanceConfig::DEFAULT.monsters;
    let reg = deadroll_rules::registry::Registry::shipped().unwrap();
    let wolf: MonsterTemplate = reg.template(0).unwrap();
    let l0 = wolf.profile(0, &s);
    let l3 = wolf.profile(3, &s);
    // min + 3/2, max + 3, attack + 3.
    assert_eq!(
        (l3.weapon.min, l3.weapon.max),
        (l0.weapon.min + 1, l0.weapon.max + 3)
    );
    assert_eq!(l3.attack_bonus, l0.attack_bonus + 3);
    assert_eq!((l0.xp, l3.xp), (wolf.xp, wolf.xp * 3));
    // Defenses come from the record: the wolf resists Cold at the scaling's level.
    assert_eq!(l3.resist_bp[Aspect::Cold as usize], s.resist_bp);
    assert_eq!(l3.resist_bp.iter().filter(|r| **r > 0).count(), 1);
    assert_eq!((l3.immune_aspects, l3.immune_states), (0, 0));
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
    // An elite has more HP and an extra active slot; a boss is never elite.
    let elite = deadroll_types::SPAWN_ELITE;
    assert_eq!(wolf.hp_at(1, elite, &s), wolf.hp_at(1, 0, &s) * 3 / 2);
    assert_eq!(wolf.slots(elite, &s)[0], wolf.slots(0, &s)[0] + 1);
    let boss = reg.template(2).unwrap();
    assert_eq!(boss.rank(elite), Rank::Boss);
    assert_eq!(boss.hp_at(1, elite, &s), boss.hp_at(1, 0, &s));
    assert_eq!(boss.slots(0, &s), [5, 1, 1]);
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

    assert_eq!(class_increase(Class::Support, 14, &cfg.players), None);
    cfg.players.class_increase_by_tier_bp[Class::Support as usize][13] = 1_500;
    assert!(class_increase(Class::Support, 14, &cfg.players).is_some());
    // Each tier reads its own entry.
    assert_eq!(class_increase(Class::Support, 13, &cfg.players), None);

    cfg.players.melee_range = 2;
    assert!(in_range(
        RangeTag::Melee,
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
        class_tier: 14,
        equipment: Default::default(),
    };
    let cleave = presets::WARRIOR[3].blocks;
    let a = player_attacker(
        &BalanceConfig::DEFAULT,
        0,
        &hero,
        Some(&SkillStats::derive(
            cleave,
            presets::WARRIOR[3].ext,
            &cfg.skills,
        )),
        &NONE,
        &AccuracyBuff::default(),
    );
    cfg.skills.line_counter_bp = 0;
    let b = player_attacker(
        &cfg,
        0,
        &hero,
        Some(&SkillStats::derive(
            cleave,
            presets::WARRIOR[3].ext,
            &cfg.skills,
        )),
        &NONE,
        &AccuracyBuff::default(),
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
        let s = SkillStats::derive(p.blocks, p.ext, &BalanceConfig::DEFAULT.skills);
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
        effect_range(&cfg, &mage(), 10_000, EffectAction::Stun),
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
        0,
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
        0,
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

fn mage() -> HeroStats {
    HeroStats {
        race: Race::ForestElf,
        class: Class::Mage,
        level: 1,
        stats: [5, 7, 10, 8],
        predisposition: Aspect::Cold,
        predisposition_tier: 14,
        class_tier: 14,
        equipment: Default::default(),
    }
}
