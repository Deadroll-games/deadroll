//! Fights, batches, presets and the config file.
use std::path::PathBuf;

use deadroll_rules::BalanceConfig;
use deadroll_sim::{
    config_io, fight,
    presets::{EncounterSlot, HeroPreset, Kind, Presets, SkillPreset},
    random_builds, resolve_fight, resolve_party, run_batch, Arena, EncounterPreset, Outcome,
    Progress, RandomBuilds, Settings,
};
use deadroll_skillcheck::{Category, Effect, Purpose, Target, Trigger};
use deadroll_types::{Aspect, Class, Form, RangeTag, Source, TimeTag};

fn shipped() -> Presets {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/balance/presets");
    let (p, errors) = Presets::load(&root);
    assert!(errors.is_empty(), "{errors:?}");
    p
}

fn party() -> Vec<String> {
    vec!["human_warrior".into(), "elf_mage".into()]
}

#[test]
fn shipped_presets_load_and_resolve() {
    let p = shipped();
    let cfg = BalanceConfig::DEFAULT;
    for id in p.ids(Kind::Heroes) {
        let (f, issues) = resolve_party(&p, std::slice::from_ref(&id), &cfg);
        assert!(issues.errors.is_empty(), "{id}: {:?}", issues.errors);
        assert!(!f[0].skills.is_empty(), "{id}");
    }
    for id in p.ids(Kind::Encounters) {
        let (f, issues) = resolve_fight(&p, &id, &BalanceConfig::DEFAULT);
        assert!(issues.errors.is_empty(), "{id}: {:?}", issues.errors);
        assert!(f.is_some());
    }
}

#[test]
fn a_fight_is_deterministic_and_ends() {
    let p = shipped();
    let cfg = BalanceConfig::DEFAULT;
    let (team, _) = resolve_party(&p, &party(), &cfg);
    for enc in ["wolves", "goblins", "chieftain", "demo_map"] {
        let (f, _) = resolve_fight(&p, enc, &BalanceConfig::DEFAULT);
        let f = f.unwrap();
        let settings = Settings::default();
        for seed in 0..40 {
            let a = fight(&cfg, &team, &f, settings, seed);
            assert_eq!(a, fight(&cfg, &team, &f, settings, seed));
            assert!(a.rounds >= 1 && a.rounds <= settings.max_rounds);
            assert_eq!(a.slots.len(), team.len());
            if a.outcome == Outcome::Defeat {
                assert!(a.slots.iter().all(|s| s.died));
            }
        }
    }
    // A round cap of 1 against the boss pack cannot be won in time.
    let (f, _) = resolve_fight(&p, "wolves", &BalanceConfig::DEFAULT);
    let short = Settings {
        max_rounds: 1,
        ..Settings::default()
    };
    let s = fight(&cfg, &team, &f.unwrap(), short, 3);
    assert_eq!((s.outcome, s.rounds), (Outcome::Draw, 1));
}

#[test]
fn batches_do_not_depend_on_threads_and_follow_the_config() {
    let p = shipped();
    let cfg = BalanceConfig::DEFAULT;
    let (team, _) = resolve_party(&p, &party(), &cfg);
    let (f, _) = resolve_fight(&p, "wolves", &BalanceConfig::DEFAULT);
    let f = f.unwrap();
    let settings = Settings::default();
    let run = |cfg: &BalanceConfig| {
        run_batch(cfg, &team, &f, settings, 400, 7, &Progress::default()).unwrap()
    };
    let many = run(&cfg);
    let one = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap()
        .install(|| run(&cfg));
    assert_eq!(many, one);
    assert_eq!(many.fights, 400);
    assert_eq!(many.victories + many.defeats + many.draws, 400);
    assert!(many.win_rate > 0.0);
    assert!(many.slots.iter().all(|s| s.dpt.mean > 0.0));
    assert!(many.hits_to_kill.samples > 0);
    // Heavier armor on the heroes and weaker attacks change the result.
    let mut tanky = cfg;
    tanky.players.armor = [500; 3];
    tanky.combat.crit_more_bp = 0;
    let other = run(&tanky);
    assert_ne!(other, many);
    assert_eq!(other.crit_share, 0.0);
    assert!(other.win_rate >= many.win_rate);
}

#[test]
fn cancel_stops_a_batch() {
    let p = shipped();
    let cfg = BalanceConfig::DEFAULT;
    let (team, _) = resolve_party(&p, &party(), &cfg);
    let (f, _) = resolve_fight(&p, "wolves", &BalanceConfig::DEFAULT);
    let progress = Progress::default();
    progress
        .cancel
        .store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(run_batch(
        &cfg,
        &team,
        &f.unwrap(),
        Settings::default(),
        100,
        1,
        &progress
    )
    .is_none());
}

#[test]
fn random_builds_are_valid_and_ranked() {
    let p = shipped();
    let cfg = BalanceConfig::DEFAULT;
    let (team, _) = resolve_party(&p, &party(), &cfg);
    let (f, _) = resolve_fight(&p, "goblins", &BalanceConfig::DEFAULT);
    let rb = RandomBuilds {
        count: 60,
        fights_each: 6,
        class: Some(Class::Mage),
        slot: 1,
        seed: 11,
    };
    let progress = Progress::default();
    let top = random_builds(
        &cfg,
        &p,
        &team,
        &f.unwrap(),
        Settings::default(),
        rb,
        &progress,
    )
    .unwrap();
    assert_eq!(top.len(), 10);
    assert!(top.windows(2).all(|w| w[0].dpt >= w[1].dpt));
    for b in &top {
        assert_eq!(b.hero.class, Class::Mage);
        assert!(b.issues.errors.is_empty(), "{:?}", b.issues.errors);
        for s in &b.hero.inline_skills {
            assert!(s.check_for(Class::Mage).is_ok());
        }
    }
    assert_eq!(progress.fraction(), 1.0);
}

fn skill(source: Source, purpose: Purpose, effect: Effect, aspects: Vec<Aspect>) -> SkillPreset {
    SkillPreset {
        name: "test".into(),
        category: Category::Active,
        purpose,
        source,
        form: Form::Projectile,
        target: Target::Single,
        aspects,
        effect,
        trigger: Trigger::None,
        aura_radius: 0,
        range: RangeTag::Ranged,
        timing: TimeTag::Instant,
        hp_below_pct: 0,
        description: String::new(),
    }
}

#[test]
fn invalid_skills_never_fight_and_say_why() {
    let mut p = shipped();
    // A warrior may not start with a magical skill; Freeze needs Cold; four aspects never pass.
    p.skills.insert(
        "magic".into(),
        skill(
            Source::Magical,
            Purpose::Damage,
            Effect::None,
            vec![Aspect::Fire],
        ),
    );
    p.skills.insert(
        "bad_freeze".into(),
        skill(
            Source::Combat,
            Purpose::Damage,
            Effect::Freeze,
            vec![Aspect::Fire],
        ),
    );
    p.skills.insert(
        "four".into(),
        skill(
            Source::Combat,
            Purpose::Damage,
            Effect::None,
            vec![Aspect::Fire, Aspect::Cold, Aspect::Blood, Aspect::Blunt],
        ),
    );
    let mut hero = p.heroes["human_warrior"].clone();
    hero.skills = vec![
        "magic".into(),
        "bad_freeze".into(),
        "four".into(),
        "javelin".into(),
        "nope".into(),
    ];
    p.heroes.insert("broken".into(), hero);
    let (team, issues) = resolve_party(&p, &["broken".into()], &BalanceConfig::DEFAULT);
    assert_eq!(team[0].skills.len(), 1, "only the javelin fights");
    let text = issues.errors.join("\n");
    assert!(
        text.contains("magic") && text.contains("power source"),
        "{text}"
    );
    assert!(
        text.contains("bad_freeze") && text.contains("Freeze needs Cold"),
        "{text}"
    );
    assert!(
        text.contains("four") && text.contains("number of aspects"),
        "{text}"
    );
    assert!(text.contains("no skill `nope`"), "{text}");
}

#[test]
fn presets_save_and_load_by_file() {
    let dir = std::env::temp_dir().join(format!("deadroll-sim-presets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut p = shipped();
    let id = p.free_id(Kind::Encounters, "Wolves");
    assert_eq!(id, "wolves_2");
    p.encounters.insert(
        id.clone(),
        EncounterPreset {
            name: "Wolves".into(),
            monsters: vec![EncounterSlot {
                monster: "wolf".into(),
                level: 3,
                count: 5,
                elite: true,
                skills: Some(0b1110),
            }],
            arena: Arena::default(),
            approach: deadroll_sim::presets::Approach::Back,
            then: vec![deadroll_sim::presets::PackStep {
                encounter: "goblins".into(),
                approach: deadroll_sim::presets::Approach::Front,
                guarded: true,
            }],
            guarded: false,
        },
    );
    p.save(&dir).unwrap();
    let (back, errors) = Presets::load(&dir);
    assert!(errors.is_empty());
    assert_eq!(back, p);
    // Deleting a preset removes its file on the next save.
    p.heroes.remove("elf_mage");
    p.save(&dir).unwrap();
    assert!(!dir.join("heroes/elf_mage.json").exists());
    assert_eq!(Presets::load(&dir).0, p);
    // A broken file is reported, the others still load.
    std::fs::write(dir.join("heroes/bad.json"), "{ not json").unwrap();
    let (again, errors) = Presets::load(&dir);
    assert_eq!(errors.len(), 1);
    assert_eq!(again.heroes.len(), p.heroes.len());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_enum_names_are_rejected_with_the_options() {
    let text = r#"{"name":"x","race":"Dwarf","class":"Mage","level":1,
        "stats":{"Strength":5},"predisposition":"Cold","tier":14,"skills":[]}"#;
    let err = serde_json::from_str::<HeroPreset>(text)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("Dwarf") && err.contains("Human") && err.contains("ForestElf"),
        "{err}"
    );
}

#[test]
fn config_json_round_trips_and_fills_gaps_from_defaults() {
    let mut cfg = BalanceConfig::DEFAULT;
    cfg.combat.armor_k = 77;
    cfg.players.ap_base[1][2] = 9;
    cfg.support.mod_duration[3] = 4;
    assert_eq!(
        config_io::from_json(&config_io::to_json(&cfg)).unwrap(),
        cfg
    );
    // Missing groups, missing keys and short arrays keep the defaults.
    let partial = r#"{ "combat": { "armor_k": 50, "state_turns": [9] }, "players": { "basic_attack_ap": 3 } }"#;
    let loaded = config_io::from_json(partial).unwrap();
    let mut expected = BalanceConfig::DEFAULT;
    expected.combat.armor_k = 50;
    expected.combat.state_turns[0] = 9;
    expected.players.basic_attack_ap = 3;
    assert_eq!(loaded, expected);
    assert!(config_io::from_json("nope").is_err());
}

#[test]
fn hero_warnings_follow_the_creation_rules() {
    let p = shipped();
    let mut h = p.heroes["elf_mage"].clone();
    assert!(h.warnings(&BalanceConfig::DEFAULT.players).is_empty());
    h.predisposition = Aspect::Blood;
    h.stats[0] += 1;
    h.tier = 0;
    let w = h.warnings(&BalanceConfig::DEFAULT.players).join("\n");
    assert!(
        w.contains("predisposition") && w.contains("tier") && w.contains("stats"),
        "{w}"
    );
}

#[test]
fn hero_equipment_is_json_by_name_and_checked() {
    use deadroll_rules::items::Equipment;
    use deadroll_sim::setup::{fighter, Issues};
    use deadroll_types::{EquipSlot, ItemBase};

    let p = shipped();
    let cfg = BalanceConfig::DEFAULT;
    // Left out: the class starter kit. The shipped armorless warrior: a sword only.
    let kit = &p.heroes["human_warrior"];
    assert_eq!(kit.equipment, None);
    let bare = &p.heroes["human_warrior_unarmed"];
    let mut sword = Equipment::EMPTY;
    sword.set(EquipSlot::MainHand, Some(ItemBase::Sword));
    assert_eq!(bare.equipment, Some(sword));
    let mut issues = Issues::default();
    let with_kit = fighter(&p, "human_warrior", kit, &cfg, &mut issues);
    let without = fighter(&p, "human_warrior_unarmed", bare, &cfg, &mut issues);
    assert!(issues.errors.is_empty(), "{:?}", issues.errors);
    assert_eq!(
        with_kit.hero.equipment.get(EquipSlot::Body),
        Some(ItemBase::LeatherArmor)
    );
    assert_eq!(without.hero.equipment.get(EquipSlot::Body), None);

    // By name in JSON; `{}` is nothing worn.
    let json = serde_json::to_value(bare).unwrap();
    assert_eq!(json["equipment"], serde_json::json!({"MainHand": "Sword"}));
    let mut naked = bare.clone();
    naked.equipment = Some(Equipment::EMPTY);
    assert_eq!(
        serde_json::to_value(&naked).unwrap()["equipment"],
        serde_json::json!({})
    );
    let bad = serde_json::json!({"MainHand": "Axe"});
    let mut v = json.clone();
    v["equipment"] = bad;
    let err = serde_json::from_value::<HeroPreset>(v)
        .unwrap_err()
        .to_string();
    assert!(err.contains("unknown ItemBase `Axe`"), "{err}");

    // The same rule as the character program: a sword is not body armor.
    let mut wrong = bare.clone();
    let mut eq = Equipment::EMPTY;
    eq.set(EquipSlot::Body, Some(ItemBase::Sword));
    wrong.equipment = Some(eq);
    let mut issues = Issues::default();
    let f = fighter(&p, "wrong", &wrong, &cfg, &mut issues);
    assert!(f.hero.equipment.is_empty());
    assert!(
        issues.errors.iter().any(|e| e.contains("Sword in Body")),
        "{:?}",
        issues.errors
    );
}

#[test]
fn armor_changes_the_fights() {
    let p = shipped();
    let cfg = BalanceConfig::DEFAULT;
    let (f, _) = resolve_fight(&p, "chieftain", &BalanceConfig::DEFAULT);
    let f = f.unwrap();
    let run = |hero: &str| {
        let (team, issues) = resolve_party(&p, &[hero.to_string()], &cfg);
        assert!(issues.errors.is_empty(), "{:?}", issues.errors);
        run_batch(
            &cfg,
            &team,
            &f,
            Settings::default(),
            400,
            9,
            &Progress::default(),
        )
        .unwrap()
    };
    let armored = run("human_warrior");
    let bare = run("human_warrior_unarmed");
    // Same seeds, same sword: only the leather armor differs.
    assert!(
        armored.win_rate > bare.win_rate,
        "win rate {} vs {}",
        armored.win_rate,
        bare.win_rate
    );
    assert!(armored.slots[0].death_rate < bare.slots[0].death_rate);
}

#[test]
fn shipped_item_preset_and_app_table_match_the_defaults() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |rel: &str| {
        std::fs::read_to_string(root.join(rel))
            .unwrap()
            .replace("\r\n", "\n")
    };
    let hint = "stale: regenerate with `cargo run -p deadroll-sim --example export_balance`";
    assert_eq!(
        read("tools/balance/presets/items/default.json"),
        config_io::items_to_json(&BalanceConfig::DEFAULT.items),
        "{hint} -- items"
    );
    assert_eq!(
        read("app/src/chain/balance.json"),
        config_io::to_json(&BalanceConfig::DEFAULT),
        "{hint} -- config"
    );
    // An item file laid over the defaults: a partial one keeps the rest.
    let items =
        config_io::items_from_json(r#"{"bases": {"Robe": {"armor": 9}}, "level_gap": 2}"#).unwrap();
    assert_eq!(
        items.bases[deadroll_types::ItemBase::Robe as usize].armor,
        9
    );
    assert_eq!(items.level_gap, 2);
    assert_eq!(
        items.starter_kits,
        BalanceConfig::DEFAULT.items.starter_kits
    );
    let kits =
        config_io::items_from_json(r#"{"starter_kits": {"Mage": ["Wand", null, null, null]}}"#)
            .unwrap();
    assert_eq!(
        kits.starter_kits[Class::Mage as usize],
        [Some(deadroll_types::ItemBase::Wand), None, None, None]
    );
}

/// Arena update: an encounter's `then` packs are fought one arena after the other, the party
/// carried over; the approach picks who moves first.
#[test]
fn packs_are_fought_one_arena_after_the_other() {
    use deadroll_rules::arena::FirstSide;
    let p = shipped();
    let cfg = BalanceConfig::DEFAULT;
    let (f, issues) = resolve_fight(&p, "demo_arenas", &cfg);
    assert!(issues.errors.is_empty(), "{:?}", issues.errors);
    let f = f.unwrap();
    assert_eq!(f.first, FirstSide::Monsters);
    assert_eq!(f.then.len(), 2);
    assert_eq!(f.then[0].first, FirstSide::Party);
    assert_eq!(f.then[1].first, FirstSide::Monsters);
    let (back, _) = resolve_fight(&p, "wolves_back", &cfg);
    assert_eq!(back.unwrap().first, FirstSide::Party);

    let ids: Vec<String> = ["human_warrior", "elf_mage", "human_support"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let (team, _) = resolve_party(&p, &ids, &cfg);
    let mut won = 0;
    for seed in 0..40 {
        let s = fight(&cfg, &team, &f, Settings::default(), seed);
        assert!((1..=3).contains(&s.packs));
        assert!(s.hp_left <= s.hp_max && s.hp_max > 0);
        match s.outcome {
            // Every pack won, each paid its experience.
            Outcome::Victory => {
                won += 1;
                assert_eq!((s.packs, s.packs_won), (3, 3));
                assert!(s.xp > 0);
            }
            _ => assert_eq!(s.packs_won, s.packs - 1),
        }
    }
    assert!(won > 0);
}
