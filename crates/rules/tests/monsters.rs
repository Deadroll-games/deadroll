//! Patch 6 monsters in the rules: registry templates and pools, monster skills on players and on
//! allied monsters, monster auras with the kin filter, and the simple monster AI
//! (docs/BESTIARY.md 3, 5, 7-8).
#![cfg(feature = "serde")]
use deadroll_combat::{
    support::find_mod, Applied, HitInput, HitResult, ModKind, Outcome, SeededRoll, TimedMods,
};
use deadroll_rules::{
    aura::{
        monster_passive_aura, AuraField, MonsterAuraField, MonsterAuras, MonsterBearer, PartyAuras,
    },
    input::{effect_action, EffectAction, HeroStats},
    items::starter_loadout,
    monster::{select_skills, MonsterSkillStats, NO_SKILL, PASSIVE_SLOT, REACTIVE_SLOT},
    monster_ai::{plan, AiChoice, AiMonster, AiTarget, AiUnit},
    registry::Registry,
    resolve::{monster_attack, monster_debuff_player, monster_start_aura, monster_support},
    BalanceConfig, MonsterProfile, MonsterTemplate,
};
use deadroll_skillcheck::Category;
use deadroll_types::{Aspect, Cell, Class, Monster, Race, RangeTag, MONSTER_SLOTS, PARTY_MAX};

const CFG: BalanceConfig = BalanceConfig::DEFAULT;
const WOLF: u8 = 0;
const GOBLIN: u8 = 1;
const CHIEFTAIN: u8 = 2;
const SHAMAN: u8 = 4;

fn c(x: u8, y: u8) -> Cell {
    Cell { x, y }
}

fn reg() -> Registry {
    Registry::shipped().unwrap()
}

fn hero() -> HeroStats {
    HeroStats {
        race: Race::Human,
        class: Class::Warrior,
        level: 1,
        stats: [6, 6, 8, 10],
        predisposition: Aspect::Blunt,
        predisposition_tier: 14,
        class_tier: 14,
        equipment: starter_loadout(Class::Warrior, &CFG.items).equipment(),
    }
}

/// The template, its level-1 profile and its pool skill `name`.
fn skill(r: &Registry, id: u8, name: &str) -> (MonsterTemplate, MonsterProfile, MonsterSkillStats) {
    let t = r.template(id).unwrap();
    let i = r.skill_names.iter().position(|n| n == name).unwrap();
    assert!(
        (t.pool_start as usize..(t.pool_start + t.pool_len) as usize).contains(&i),
        "{name} is not in the pool of {id}"
    );
    let s = MonsterSkillStats::new(&r.skills[i].unwrap(), &CFG.skills);
    (t, t.profile(1, &CFG.monsters), s)
}

fn monster(hp: u32, hp_max: u32) -> Monster {
    Monster {
        hp,
        hp_max,
        alive: hp > 0,
        ..Monster::default()
    }
}

#[test]
fn every_pool_fills_its_rank_slots() {
    let r = reg();
    for id in 0..5u8 {
        let t = r.template(id).unwrap();
        let pool: Vec<Category> = r
            .pool(&t)
            .iter()
            .map(|s| s.unwrap().blocks.category)
            .collect();
        let slots = t.slots(0, &CFG.monsters);
        let chosen = select_skills(&pool, u16::MAX, slots);
        let taken = chosen.iter().filter(|s| **s != NO_SKILL).count();
        let expected: usize = Category::ALL
            .iter()
            .map(|c| {
                pool.iter()
                    .filter(|p| *p == c)
                    .count()
                    .min(slots[*c as usize] as usize)
            })
            .sum();
        assert_eq!(taken, expected, "{id}");
    }
    // The chieftain takes Leader into its passive slot and Battle Cry into its reactive one.
    let t = r.template(CHIEFTAIN).unwrap();
    let pool: Vec<Category> = r
        .pool(&t)
        .iter()
        .map(|s| s.unwrap().blocks.category)
        .collect();
    let chosen = select_skills(&pool, u16::MAX, t.slots(0, &CFG.monsters));
    assert_ne!(chosen[PASSIVE_SLOT], NO_SKILL);
    assert_ne!(chosen[REACTIVE_SLOT], NO_SKILL);
}

#[test]
fn a_skill_hit_rides_its_effect_onto_the_player() {
    let r = reg();
    let (_, p, hamstring) = skill(&r, WOLF, "Hamstring");
    let h = hero();
    let (mut landed, mut natural_rode) = (false, false);
    for seed in 0..200 {
        for with_skill in [true, false] {
            let mut hp = 30;
            let mut states = Default::default();
            let mut own = TimedMods::default();
            let mut input = Box::<HitInput>::default();
            let mut result = Box::<HitResult>::default();
            let out = monster_attack(
                &CFG,
                &r.tables,
                4,
                &p,
                with_skill.then_some(&hamstring.stats),
                &TimedMods::default(),
                &h,
                &mut hp,
                &mut states,
                &mut own,
                &TimedMods::default(),
                &mut input,
                &mut result,
                &mut SeededRoll::new(seed),
            );
            assert_eq!(hp, 30u32.saturating_sub(result.damage));
            match out.rider {
                Some(rider) if rider.applied != Applied::None => {
                    assert!(find_mod(&own, ModKind::Slow).is_some());
                    landed = true;
                }
                Some(_) => assert!(result.outcome == Outcome::Hit),
                None => natural_rode |= !with_skill && find_mod(&own, ModKind::Slow).is_some(),
            }
        }
    }
    assert!(landed, "Hamstring never slowed in 200 seeds");
    assert!(!natural_rode);
}

#[test]
fn a_monster_debuff_rolls_against_the_player_defense() {
    let r = reg();
    let (_, p, net) = skill(&r, CHIEFTAIN, "Net Throw");
    let effect = effect_action(net.stats.blocks.effect).unwrap();
    let (mut hit, mut miss) = (0, 0);
    for seed in 0..200 {
        let mut states = Default::default();
        let mut own = TimedMods::default();
        let out = monster_debuff_player(
            &CFG,
            4,
            &p,
            &net.stats,
            effect,
            &hero(),
            20,
            &mut states,
            &mut own,
            &TimedMods::default(),
            &mut SeededRoll::new(seed),
        );
        if out.roll.outcome == Outcome::Hit {
            assert!(find_mod(&own, ModKind::Slow).is_some());
            hit += 1;
        } else {
            assert_eq!(own, TimedMods::default());
            miss += 1;
        }
    }
    assert!(hit > 0 && miss > 0, "{hit} / {miss}");
}

#[test]
fn the_shaman_heals_and_hastes_a_goblin() {
    let r = reg();
    let (_, p, mending) = skill(&r, SHAMAN, "Mending");
    let (_, _, quicken) = skill(&r, SHAMAN, "Quicken");
    assert!(mending.kin_only && quicken.kin_only);
    for seed in 0..50 {
        let mut goblin = monster(3, 10);
        let mut mods = TimedMods::default();
        let out = monster_support(
            &CFG,
            8,
            &p,
            &mending.stats,
            EffectAction::Heal,
            &mut goblin,
            &mut mods,
            &mut SeededRoll::new(seed),
        );
        assert!(goblin.hp > 3 && goblin.hp <= 10, "{seed}: {}", goblin.hp);
        assert!(out.roll.amount >= 1);
        // Never above the maximum, never raises the dead.
        let mut full = monster(10, 10);
        monster_support(
            &CFG,
            8,
            &p,
            &mending.stats,
            EffectAction::Heal,
            &mut full,
            &mut mods,
            &mut SeededRoll::new(seed),
        );
        assert_eq!(full.hp, 10);
        let mut dead = monster(0, 10);
        monster_support(
            &CFG,
            8,
            &p,
            &mending.stats,
            EffectAction::Heal,
            &mut dead,
            &mut mods,
            &mut SeededRoll::new(seed),
        );
        assert_eq!(dead.hp, 0);
        let mut quick = TimedMods::default();
        let out = monster_support(
            &CFG,
            8,
            &p,
            &quicken.stats,
            EffectAction::Mod(ModKind::Haste),
            &mut monster(10, 10),
            &mut quick,
            &mut SeededRoll::new(seed),
        );
        assert_ne!(out.applied, Applied::None);
        assert!(find_mod(&quick, ModKind::Haste).is_some());
    }
}

/// The living monsters of `auras` that bear an aura.
fn bearers(
    auras: &[MonsterAuras],
    cells: &[Cell],
    alive: &[bool],
    kins: &[u8],
) -> Vec<MonsterBearer> {
    (0..auras.len())
        .filter(|&i| alive[i] && MonsterBearer::has_aura(&auras[i]))
        .map(|i| MonsterBearer {
            index: i as u8,
            cell: cells[i],
            kin: kins[i],
            auras: auras[i],
        })
        .collect()
}

fn field_with<'a>(
    bearers: &'a [MonsterBearer],
    party: &'a PartyAuras,
    party_cells: &'a [Cell; PARTY_MAX],
) -> AuraField<'a> {
    AuraField {
        auras: party,
        cells: party_cells,
        alive: 0,
        monsters: MonsterAuraField { bearers },
    }
}

#[test]
fn leader_reaches_nearby_goblins_but_not_the_wolf_or_a_natural_attack() {
    let r = reg();
    let (t, p, leader) = skill(&r, CHIEFTAIN, "Leader");
    let aura = monster_passive_aura(&CFG, &p, &leader).unwrap();
    assert!(aura.kin_only && aura.passive && !aura.hostile);
    let mut auras = [[Default::default(); 2]; 4];
    auras[0][0] = aura;
    let goblin_kin = t.kin;
    let wolf_kin = r.template(WOLF).unwrap().kin;
    let cells = [c(5, 5), c(6, 6), c(9, 9), c(5, 6)];
    let alive = [true; 4];
    let kins = [goblin_kin, goblin_kin, goblin_kin, wolf_kin];
    let party = [[Default::default(); 2]; PARTY_MAX];
    let pc = [c(0, 0); PARTY_MAX];
    let list = bearers(&auras, &cells, &alive, &kins);
    let f = field_with(&list, &party, &pc);
    let emp = |i: u8, basic: bool| {
        let m = f.monster_mods(
            &TimedMods::default(),
            i,
            cells[i as usize],
            kins[i as usize],
            basic,
        );
        find_mod(&m, ModKind::Empowerment).is_some()
    };
    assert!(emp(0, false), "the bearer itself");
    assert!(emp(1, false), "a goblin in the radius");
    assert!(!emp(2, false), "a goblin out of the radius");
    assert!(!emp(3, false), "a wolf in the radius: not its kin");
    assert!(
        !emp(1, true),
        "a natural attack is not strengthened by a passive"
    );
    // A dead bearer's aura is gone.
    let dead = [false, true, true, true];
    let list = bearers(&auras, &cells, &dead, &kins);
    let f = field_with(&list, &party, &pc);
    let m = f.monster_mods(&TimedMods::default(), 1, cells[1], kins[1], false);
    assert!(find_mod(&m, ModKind::Empowerment).is_none());
    // Battle Cry: a temporary kin-only Protection aura.
    let (_, _, cry) = skill(&r, CHIEFTAIN, "Battle Cry");
    let (_, aura) = monster_start_aura(&CFG, &p, &cry, &mut SeededRoll::new(3));
    let aura = aura.unwrap();
    assert!(aura.kin_only && !aura.passive && aura.turns > 0);
    assert_eq!(aura.kind, ModKind::Protection);
}

#[test]
fn a_hostile_monster_aura_reaches_players() {
    let mut auras = [[Default::default(); 2]; 1];
    auras[0][0] = deadroll_rules::aura::Aura {
        kind: ModKind::Slow,
        power_bp: 2_000,
        radius: 2,
        turns: 3,
        hostile: true,
        ..Default::default()
    };
    let cells = [c(5, 5)];
    let list = bearers(&auras, &cells, &[true], &[0]);
    let party = [[Default::default(); 2]; PARTY_MAX];
    let pc = [c(6, 6), c(9, 9), c(0, 0), c(0, 0)];
    let f = AuraField {
        alive: 0b11,
        ..field_with(&list, &party, &pc)
    };
    let slow = |s: u8| {
        let m = f.hero_mods(&TimedMods::default(), s, pc[s as usize], false);
        find_mod(&m, ModKind::Slow).is_some()
    };
    assert!(slow(0));
    assert!(!slow(1));
}

fn unit(cell: Cell, hp: u32, hp_max: u32, kin: u8) -> AiUnit {
    AiUnit {
        cell,
        hp,
        hp_max,
        alive: hp > 0,
        kin,
        ..AiUnit::default()
    }
}

/// The selected skills of a template (every pool bit), by slot.
fn skills_of(r: &Registry, t: &MonsterTemplate) -> [Option<MonsterSkillStats>; MONSTER_SLOTS] {
    let pool = r.pool(t);
    let cats: Vec<Category> = pool.iter().map(|s| s.unwrap().blocks.category).collect();
    let chosen = select_skills(&cats, u16::MAX, t.slots(0, &CFG.monsters));
    chosen.map(|i| {
        (i != NO_SKILL).then(|| MonsterSkillStats::new(&pool[i as usize].unwrap(), &CFG.skills))
    })
}

#[test]
fn the_ai_heals_first_then_buffs_then_attacks() {
    let r = reg();
    let shaman = r.template(SHAMAN).unwrap();
    let p = shaman.profile(1, &CFG.monsters);
    let skills = skills_of(&r, &shaman);
    let slot_of = |name: &str| {
        skills
            .iter()
            .position(|s| {
                s.is_some_and(|s| {
                    r.skills.iter().zip(&r.skill_names).any(|(k, n)| {
                        n == name && MonsterSkillStats::new(&k.unwrap(), &CFG.skills) == s
                    })
                })
            })
            .unwrap() as u8
    };
    let kin = shaman.kin;
    let wolf = r.template(WOLF).unwrap().kin;
    let heroes = [unit(c(9, 5), 20, 30, 0)];
    let cooldowns = [0u8; MONSTER_SLOTS];
    let me = AiMonster {
        index: 0,
        ap: 3,
        profile: &p,
        skills: &skills,
        cooldowns: &cooldowns,
    };
    // A hurt goblin in reach: Mending on it.
    let monsters = [unit(c(5, 5), 10, 10, kin), unit(c(6, 5), 2, 10, kin)];
    let choice = plan(&CFG, &me, &heroes, &monsters).unwrap();
    assert_eq!(
        choice,
        AiChoice {
            slot: Some(slot_of("Mending")),
            target: AiTarget::Monster(1),
            range: RangeTag::Ranged,
            ap: skills[slot_of("Mending") as usize].unwrap().stats.ap_cost,
        }
    );
    // A hurt wolf is not its kin: no heal, Quicken on itself.
    let monsters = [unit(c(5, 5), 10, 10, kin), unit(c(6, 5), 2, 10, wolf)];
    let choice = plan(&CFG, &me, &heroes, &monsters).unwrap();
    assert_eq!(choice.slot, Some(slot_of("Quicken")));
    assert_eq!(choice.target, AiTarget::Monster(0));
    // Hasted already, everyone whole: Spark on the player.
    let mut hasted = unit(c(5, 5), 10, 10, kin);
    hasted.mods[0] = deadroll_combat::TimedMod {
        kind: ModKind::Haste,
        turns: 2,
        power_bp: 1,
        ..Default::default()
    };
    let monsters = [hasted];
    let choice = plan(&CFG, &me, &heroes, &monsters).unwrap();
    assert_eq!(choice.slot, Some(slot_of("Spark")));
    assert_eq!(choice.target, AiTarget::Player(0));
    // Everything on cooldown: the natural attack (a projectile reaches).
    let cooldowns = [1u8; MONSTER_SLOTS];
    let me = AiMonster {
        cooldowns: &cooldowns,
        ..me
    };
    let choice = plan(&CFG, &me, &heroes, &monsters).unwrap();
    assert_eq!(choice.slot, None);
    // Out of every reach: nothing, walk.
    let far = [unit(c(31, 31), 20, 30, 0)];
    assert_eq!(plan(&CFG, &me, &far, &monsters), None);
}

#[test]
fn the_ai_debuffs_before_it_strikes() {
    let r = reg();
    let chief = r.template(CHIEFTAIN).unwrap();
    let p = chief.profile(1, &CFG.monsters);
    let skills = skills_of(&r, &chief);
    let cooldowns = [0u8; MONSTER_SLOTS];
    let me = AiMonster {
        index: 0,
        ap: 5,
        profile: &p,
        skills: &skills,
        cooldowns: &cooldowns,
    };
    let monsters = [unit(c(5, 5), 30, 30, chief.kin)];
    let heroes = [unit(c(6, 5), 20, 30, 0)];
    let choice = plan(&CFG, &me, &heroes, &monsters).unwrap();
    let s = skills[choice.slot.unwrap() as usize].unwrap();
    assert_eq!(
        effect_action(s.stats.blocks.effect),
        Some(EffectAction::Mod(ModKind::Slow))
    );
    // A slowed player gets a damage skill.
    let mut slowed = heroes;
    slowed[0].mods[0] = deadroll_combat::TimedMod {
        kind: ModKind::Slow,
        turns: 2,
        power_bp: 1,
        ..Default::default()
    };
    let choice = plan(&CFG, &me, &slowed, &monsters).unwrap();
    let s = skills[choice.slot.unwrap() as usize].unwrap();
    assert_eq!(s.stats.blocks.purpose, deadroll_skillcheck::Purpose::Damage);
    let _ = GOBLIN;
}

#[test]
fn a_rooted_monster_only_reaches_what_it_reaches_from_where_it_stands() {
    let r = reg();
    let chief = r.template(CHIEFTAIN).unwrap();
    let p = chief.profile(1, &CFG.monsters);
    let skills = skills_of(&r, &chief);
    let cooldowns = [0u8; MONSTER_SLOTS];
    let me = AiMonster {
        index: 0,
        ap: 5,
        profile: &p,
        skills: &skills,
        cooldowns: &cooldowns,
    };
    let free = [unit(c(5, 5), 30, 30, chief.kin)];
    let mut rooted = free;
    rooted[0].mods[ModKind::Root as usize] = deadroll_combat::TimedMod {
        kind: ModKind::Root,
        turns: 2,
        power_bp: 1,
        ..Default::default()
    };
    // Some distance is reached by walking first: the rooted monster cannot.
    let walked = (6..20u8).find(|&x| {
        let heroes = [unit(c(x, 5), 20, 30, 0)];
        plan(&CFG, &me, &heroes, &free).is_some() && plan(&CFG, &me, &heroes, &rooted).is_none()
    });
    assert!(walked.is_some());
    // Next to it, rooted or not, it acts.
    let heroes = [unit(c(6, 5), 20, 30, 0)];
    assert!(plan(&CFG, &me, &heroes, &rooted).is_some());
}

#[test]
fn a_silenced_monster_has_only_its_natural_attack() {
    let r = reg();
    let chief = r.template(CHIEFTAIN).unwrap();
    let p = chief.profile(1, &CFG.monsters);
    let skills = skills_of(&r, &chief);
    assert!(
        skills.iter().any(|s| s.is_some()),
        "the chieftain has skills"
    );
    let cooldowns = [0u8; MONSTER_SLOTS];
    let me = AiMonster {
        index: 0,
        ap: 5,
        profile: &p,
        skills: &skills,
        cooldowns: &cooldowns,
    };
    let heroes = [unit(c(6, 5), 20, 30, 0)];
    let free = [unit(c(5, 5), 30, 30, chief.kin)];
    let mut silenced = free;
    silenced[0].mods[ModKind::Silenced as usize] = deadroll_combat::TimedMod {
        kind: ModKind::Silenced,
        turns: 2,
        power_bp: 10_000,
        ..Default::default()
    };
    let with_skill = (0..30u8)
        .filter_map(|x| {
            plan(
                &CFG,
                &me,
                &[unit(c(5 + x % 3, 5 + x / 3 % 3), 20, 30, 0)],
                &free,
            )
        })
        .any(|choice| choice.slot.is_some());
    assert!(with_skill, "a free chieftain uses a skill somewhere");
    for x in 0..30u8 {
        let heroes = [unit(c(5 + x % 3, 5 + x / 3 % 3), 20, 30, 0)];
        if let Some(choice) = plan(&CFG, &me, &heroes, &silenced) {
            assert_eq!(choice.slot, None, "silenced at {x}");
        }
    }
    assert!(plan(&CFG, &me, &heroes, &silenced).is_some());
}

#[test]
fn a_heal_does_nothing_to_a_monster_with_no_heal() {
    let r = reg();
    let (_, p, mending) = skill(&r, SHAMAN, "Mending");
    let mut no_heal = TimedMods::default();
    no_heal[ModKind::NoHeal as usize] = deadroll_combat::TimedMod {
        kind: ModKind::NoHeal,
        turns: 2,
        power_bp: 10_000,
        ..Default::default()
    };
    for seed in 0..20 {
        let mut goblin = monster(3, 10);
        monster_support(
            &CFG,
            8,
            &p,
            &mending.stats,
            EffectAction::Heal,
            &mut goblin,
            &mut no_heal,
            &mut SeededRoll::new(seed),
        );
        assert_eq!(goblin.hp, 3, "seed {seed}");
        let mut healed = monster(3, 10);
        monster_support(
            &CFG,
            8,
            &p,
            &mending.stats,
            EffectAction::Heal,
            &mut healed,
            &mut TimedMods::default(),
            &mut SeededRoll::new(seed),
        );
        assert!(healed.hp > 3);
    }
}

#[test]
fn remove_mods_takes_off_the_masked_kinds_only() {
    let mut mods = TimedMods::default();
    for kind in [ModKind::Weakening, ModKind::Slow, ModKind::Dazed] {
        mods[kind as usize] = deadroll_combat::TimedMod {
            kind,
            turns: 2,
            power_bp: 100,
            ..Default::default()
        };
    }
    deadroll_rules::resolve::remove_mods(
        &mut mods,
        deadroll_combat::support::mod_bit(ModKind::Slow)
            | deadroll_combat::support::mod_bit(ModKind::Dazed),
    );
    assert!(find_mod(&mods, ModKind::Weakening).is_some());
    assert!(find_mod(&mods, ModKind::Slow).is_none() && find_mod(&mods, ModKind::Dazed).is_none());
}
