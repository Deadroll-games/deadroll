//! Patch 5 skills in the rules: passive skills and auras, reactive triggers and reactions, the
//! line target (docs/SKILL_BUILDER.md, sections 2, 3.2, 3.3, 3.5, 5.3).
use deadroll_combat::states::state_bit;
use deadroll_combat::{
    mod_turns, support::find_mod, HitResult, ModKind, Outcome, ScriptedRoll, TimedMod, TimedMods,
};
use deadroll_rules::aura::{passive_aura, reactive_aura};
use deadroll_rules::{
    aura::{tick_auras, Aura, AuraField, MonsterAuraField, PartyAuras, PERMANENT},
    input::{action_ap, effect_range, EffectAction, HeroStats, SkillStats},
    items::starter_loadout,
    line::{line_cells, targets_on_line},
    reactive::{dropped_below, entered_next_to, fires, hit_events, reaction, Event, Reaction},
    resolve::start_aura,
    BalanceConfig,
};
use deadroll_skillcheck::{presets, Effect, Purpose, SkillExt, Trigger};
use deadroll_types::{Aspect, Cell, Class, Race, RangeTag, StateKind, PARTY_MAX};

const CFG: BalanceConfig = BalanceConfig::DEFAULT;

fn c(x: u8, y: u8) -> Cell {
    Cell { x, y }
}

fn hero(class: Class) -> HeroStats {
    HeroStats {
        race: Race::Human,
        class,
        level: 1,
        stats: [6, 6, 8, 10],
        predisposition: Aspect::Blunt,
        predisposition_tier: 14,
        class_tier: 14,
        equipment: starter_loadout(class, &CFG.items).equipment(),
    }
}

fn stats(p: &presets::Preset) -> SkillStats {
    SkillStats::derive(p.blocks, p.ext, &CFG.skills)
}

fn none() -> TimedMods {
    TimedMods::default()
}

fn power(mods: &TimedMods, kind: ModKind) -> u32 {
    find_mod(mods, kind).map_or(0, |i| mods[i].power_bp)
}

#[test]
fn passive_skills_work_at_the_fixed_point_of_their_range() {
    let support = hero(Class::Support);
    let ward = stats(&presets::SUPPORT[5]); // Cold Ward: Protection aura, radius 2
    let a = passive_aura(&CFG, &support, &ward).unwrap();
    let (min, max) = effect_range(
        &CFG,
        &support,
        ward.power_bp,
        EffectAction::Mod(ModKind::Protection),
    );
    assert_eq!(a.power_bp, min + (max - min) / 2);
    assert_eq!(
        (a.kind, a.aspect, a.radius),
        (ModKind::Protection, Aspect::Cold, 2)
    );
    assert!(a.passive && !a.hostile && !a.self_only && a.turns == PERMANENT);
    // Iron Skin has no aura: on the bearer only.
    let skin = passive_aura(&CFG, &hero(Class::Warrior), &stats(&presets::WARRIOR[5])).unwrap();
    assert!(skin.self_only);
    // A skill that is not passive has no permanent aura.
    assert_eq!(
        passive_aura(&CFG, &support, &stats(&presets::SUPPORT[0])),
        None
    );
}

fn party(auras: PartyAuras) -> PartyAuras {
    auras
}

fn aura(kind: ModKind, power_bp: u32, radius: u8) -> Aura {
    Aura {
        kind,
        aspect: Aspect::Cold,
        power_bp,
        radius,
        turns: PERMANENT,
        hostile: false,
        passive: true,
        self_only: false,
        kin_only: false,
        weights_pct: [0; Aspect::COUNT],
    }
}

#[test]
fn an_aura_reaches_its_bearer_and_allies_in_its_radius() {
    let mut auras = party([[Aura::default(); 2]; PARTY_MAX]);
    auras[0][0] = aura(ModKind::Protection, 3_000, 2);
    let cells = [c(5, 5), c(7, 7), c(8, 5), c(5, 5)];
    let field = AuraField {
        auras: &auras,
        cells: &cells,
        alive: 0b1111,
        monsters: MonsterAuraField::EMPTY,
    };
    assert_eq!(
        power(
            &field.hero_mods(&none(), 0, cells[0], false),
            ModKind::Protection
        ),
        3_000
    );
    assert_eq!(
        power(
            &field.hero_mods(&none(), 1, cells[1], false),
            ModKind::Protection
        ),
        3_000
    );
    assert_eq!(
        power(
            &field.hero_mods(&none(), 2, cells[2], false),
            ModKind::Protection
        ),
        0
    );
    assert_eq!(
        power(
            &field.hero_mods(&none(), 3, cells[3], false),
            ModKind::Protection
        ),
        3_000
    );
    // A dead bearer's aura is gone.
    let dead = AuraField {
        alive: 0b1110,
        ..field
    };
    assert_eq!(
        power(
            &dead.hero_mods(&none(), 1, cells[1], false),
            ModKind::Protection
        ),
        0
    );
    // Monsters never take a friendly aura.
    assert_eq!(
        power(
            &field.monster_mods(&none(), 0, cells[1], 0, false),
            ModKind::Protection
        ),
        0
    );
}

#[test]
fn identical_auras_do_not_stack_the_strongest_acts() {
    let mut auras = [[Aura::default(); 2]; PARTY_MAX];
    auras[0][0] = aura(ModKind::Protection, 2_000, 3);
    auras[1][0] = aura(ModKind::Protection, 4_500, 3);
    auras[2][1] = Aura {
        passive: false,
        turns: 3,
        ..aura(ModKind::Protection, 3_000, 3)
    };
    let cells = [c(1, 1), c(2, 2), c(3, 3), c(1, 2)];
    let field = AuraField {
        auras: &auras,
        cells: &cells,
        alive: 0b1111,
        monsters: MonsterAuraField::EMPTY,
    };
    let mods = field.hero_mods(&none(), 3, cells[3], false);
    assert_eq!(power(&mods, ModKind::Protection), 4_500);
    assert_eq!(mods.iter().filter(|m| !m.is_empty()).count(), 1);
    // A stronger own buff of the same kind stays; a weaker one gives way to the aura.
    let mut own = none();
    own[0] = TimedMod {
        kind: ModKind::Protection,
        aspect: Aspect::Fire,
        turns: 2,
        power_bp: 6_000,
        source: 3,
        weights_pct: [0; Aspect::COUNT],
    };
    assert_eq!(
        power(
            &field.hero_mods(&own, 3, cells[3], false),
            ModKind::Protection
        ),
        6_000
    );
    own[0].power_bp = 1_000;
    let merged = field.hero_mods(&own, 3, cells[3], false);
    assert_eq!(power(&merged, ModKind::Protection), 4_500);
    // The view keeps the buff's own duration.
    assert_eq!(merged[0].turns, 2);
}

#[test]
fn the_basic_attack_is_not_strengthened_by_passive_skills() {
    let mut auras = [[Aura::default(); 2]; PARTY_MAX];
    auras[0][0] = aura(ModKind::Empowerment, 3_000, 2);
    let cells = [c(1, 1), c(2, 1), c(0, 0), c(0, 0)];
    let field = AuraField {
        auras: &auras,
        cells: &cells,
        alive: 0b11,
        monsters: MonsterAuraField::EMPTY,
    };
    assert_eq!(
        power(
            &field.hero_mods(&none(), 1, cells[1], false),
            ModKind::Empowerment
        ),
        3_000
    );
    assert_eq!(
        power(
            &field.hero_mods(&none(), 1, cells[1], true),
            ModKind::Empowerment
        ),
        0
    );
    // A reactive (temporary) aura does strengthen it.
    let mut temporary = auras;
    temporary[0][0].passive = false;
    let field = AuraField {
        auras: &temporary,
        ..field
    };
    assert_eq!(
        power(
            &field.hero_mods(&none(), 1, cells[1], true),
            ModKind::Empowerment
        ),
        3_000
    );
}

#[test]
fn a_passive_without_aura_works_on_its_bearer_only() {
    let mut auras = [[Aura::default(); 2]; PARTY_MAX];
    auras[0][0] = Aura {
        self_only: true,
        radius: 0,
        ..aura(ModKind::Protection, 2_500, 0)
    };
    // The whole party on the entrance cell.
    let cells = [c(1, 1); PARTY_MAX];
    let field = AuraField {
        auras: &auras,
        cells: &cells,
        alive: 0b1111,
        monsters: MonsterAuraField::EMPTY,
    };
    assert_eq!(
        power(
            &field.hero_mods(&none(), 0, cells[0], false),
            ModKind::Protection
        ),
        2_500
    );
    assert_eq!(
        power(
            &field.hero_mods(&none(), 1, cells[1], false),
            ModKind::Protection
        ),
        0
    );
}

#[test]
fn a_debuff_aura_works_on_the_enemies_around() {
    let mut auras = [[Aura::default(); 2]; PARTY_MAX];
    auras[2][0] = Aura {
        hostile: true,
        ..aura(ModKind::Slow, 3_000, 2)
    };
    let cells = [c(0, 0), c(0, 0), c(4, 4), c(0, 0)];
    let field = AuraField {
        auras: &auras,
        cells: &cells,
        alive: 0b0100,
        monsters: MonsterAuraField::EMPTY,
    };
    assert_eq!(
        power(
            &field.monster_mods(&none(), 0, c(6, 5), 0, false),
            ModKind::Slow
        ),
        3_000
    );
    assert_eq!(
        power(
            &field.monster_mods(&none(), 0, c(7, 4), 0, false),
            ModKind::Slow
        ),
        0
    );
    // Not on the bearer or its allies.
    assert_eq!(
        power(&field.hero_mods(&none(), 2, cells[2], false), ModKind::Slow),
        0
    );
}

#[test]
fn a_temporary_aura_lasts_its_effect_duration() {
    let stand = stats(&presets::SUPPORT[4]); // Last Stand: reactive Protection aura
    let a = reactive_aura(&CFG, &stand, 2_000, None).unwrap();
    let duration = CFG.support.duration(ModKind::Protection);
    assert_eq!(a.turns, mod_turns(duration));
    assert!(!a.passive && !a.hostile && a.radius == 2);
    let mut slots = [Aura::default(), a];
    let mut covered = 0;
    for _ in 0..10 {
        let ended = tick_auras(&mut slots);
        if !slots[1].is_empty() {
            covered += 1;
        } else if ended != 0 {
            assert_eq!(ended, 0b10);
        }
    }
    assert_eq!(covered, duration);
    // Permanent ones never end.
    let mut perm = [aura(ModKind::Haste, 1, 1), Aura::default()];
    for _ in 0..300 {
        assert_eq!(tick_auras(&mut perm), 0);
    }
    assert_eq!(
        reactive_aura(&CFG, &stats(&presets::WARRIOR[4]), 1, None),
        None
    );
}

#[test]
fn hp_thresholds_fire_only_when_crossed() {
    // 40% of 50 is 20.
    assert!(dropped_below(20, 19, 50, 40));
    assert!(dropped_below(50, 1, 50, 40));
    assert!(!dropped_below(19, 10, 50, 40), "already below");
    assert!(!dropped_below(30, 20, 50, 40), "still at the threshold");
    assert!(!dropped_below(30, 0, 50, 40), "the dead do not react");
}

#[test]
fn each_trigger_fires_on_its_own_event() {
    let mut s = stats(&presets::WARRIOR[4]); // Riposte: Hit
    let hit = Event::Hit { crit: false };
    let crit = Event::Hit { crit: true };
    let drop = Event::HpDrop {
        before: 30,
        after: 10,
        hp_max: 50,
    };
    let ally = Event::AllyHpDrop {
        ally: 1,
        before: 30,
        after: 10,
        hp_max: 50,
    };
    let table = [
        (Trigger::Hit, [true, true, false, false, false, false]),
        (Trigger::CritHit, [false, true, false, false, false, false]),
        (Trigger::HpBelow, [false, false, true, false, false, false]),
        (
            Trigger::AllyHpBelow,
            [false, false, false, true, false, false],
        ),
        (
            Trigger::Controlled,
            [false, false, false, false, true, false],
        ),
        (
            Trigger::EnemyAdjacent,
            [false, false, false, false, false, true],
        ),
    ];
    for (trigger, want) in table {
        s.blocks.trigger = trigger;
        s.ext.hp_below_pct = if trigger.uses_threshold() { 40 } else { 0 };
        let got = [
            hit,
            crit,
            drop,
            ally,
            Event::Controlled,
            Event::EnemyEntered,
        ]
        .map(|e| fires(&s, &e));
        assert_eq!(got, want, "{trigger:?}");
    }
    // Only reactive skills react.
    let active = stats(&presets::WARRIOR[0]);
    assert!(!fires(&active, &hit));
}

#[test]
fn a_hit_makes_its_events() {
    let mut r = HitResult {
        outcome: Outcome::Hit,
        crit: true,
        landed: state_bit(StateKind::Stun),
        ..HitResult::default()
    };
    let e = hit_events(&r, 30, 25, 40);
    assert_eq!(e[0], Some(Event::Hit { crit: true }));
    assert_eq!(
        e[1],
        Some(Event::HpDrop {
            before: 30,
            after: 25,
            hp_max: 40
        })
    );
    assert_eq!(e[2], Some(Event::Controlled));
    r.outcome = Outcome::Miss;
    r.landed = 0;
    assert_eq!(hit_events(&r, 30, 30, 40), [None, None, None]);
    // A Freeze from Chill buildup counts as control too.
    r.outcome = Outcome::Hit;
    r.landed = state_bit(StateKind::Freeze);
    assert_eq!(hit_events(&r, 30, 29, 40)[2], Some(Event::Controlled));
}

#[test]
fn an_enemy_enters_next_to_the_bearer() {
    assert!(entered_next_to(c(5, 1), c(3, 2), c(2, 2)));
    assert!(
        !entered_next_to(c(3, 1), c(3, 2), c(2, 2)),
        "it was next to it already"
    );
    assert!(
        !entered_next_to(c(6, 1), c(5, 2), c(2, 2)),
        "not next to it"
    );
}

#[test]
fn reactions_need_their_reach_and_no_cooldown() {
    let riposte = stats(&presets::WARRIOR[4]); // melee counterattack
    let hit = Event::Hit { crit: false };
    let at = c(4, 4);
    let near = Some((3, c(5, 4)));
    let far = Some((3, c(8, 4)));
    assert_eq!(
        reaction(&CFG, &riposte, 0, 0, at, &hit, near, None),
        Some(Reaction::Strike { monster: 3 })
    );
    assert_eq!(reaction(&CFG, &riposte, 0, 0, at, &hit, far, None), None);
    assert_eq!(reaction(&CFG, &riposte, 1, 0, at, &hit, near, None), None);
    assert_eq!(reaction(&CFG, &riposte, 0, 0, at, &hit, None, None), None);

    let snap = stats(&presets::MAGE[4]); // Cold Snap: debuff on the enemy that came close
    assert_eq!(
        reaction(&CFG, &snap, 0, 1, at, &Event::EnemyEntered, near, None),
        Some(Reaction::Debuff { monster: 3 })
    );

    let stand = stats(&presets::SUPPORT[4]); // Last Stand: an aura at 40%
    let drop = Event::HpDrop {
        before: 30,
        after: 10,
        hp_max: 50,
    };
    assert_eq!(
        reaction(&CFG, &stand, 0, 2, at, &drop, None, None),
        Some(Reaction::Aura)
    );

    // A buff without an aura: on the bearer, or on the ally whose HP dropped (in reach).
    let mut shield = stand;
    shield.blocks.aura_radius = 0;
    assert_eq!(
        reaction(&CFG, &shield, 0, 2, at, &drop, None, None),
        Some(Reaction::Buff { slot: 2 })
    );
    shield.blocks.trigger = Trigger::AllyHpBelow;
    let ally = Event::AllyHpDrop {
        ally: 0,
        before: 30,
        after: 10,
        hp_max: 50,
    };
    assert_eq!(
        reaction(&CFG, &shield, 0, 2, at, &ally, None, Some(c(5, 5))),
        Some(Reaction::Buff { slot: 0 })
    );
    assert_eq!(
        reaction(&CFG, &shield, 0, 2, at, &ally, None, Some(c(6, 6))),
        None
    );
    shield.ext.range = RangeTag::Ranged;
    assert_eq!(
        reaction(&CFG, &shield, 0, 2, at, &ally, None, Some(c(6, 6))),
        Some(Reaction::Buff { slot: 0 })
    );
}

#[test]
fn a_temporary_aura_rolls_like_a_buff() {
    let support = hero(Class::Support);
    let stand = stats(&presets::SUPPORT[4]);
    // No crit chance from the starter kit: one d100 for the strength.
    let (roll, a) = start_aura(&CFG, &support, &stand, &mut ScriptedRoll::new([100]));
    assert_eq!(roll.outcome, Outcome::Hit);
    let (_, max) = effect_range(
        &CFG,
        &support,
        stand.power_bp,
        EffectAction::Mod(ModKind::Protection),
    );
    assert_eq!(a.unwrap().power_bp, max);
    let (_, none) = start_aura(
        &CFG,
        &support,
        &stats(&presets::WARRIOR[4]),
        &mut ScriptedRoll::new([0]),
    );
    assert_eq!(none, None);
}

#[test]
fn skills_and_the_basic_attack_cost_their_own_action_points() {
    assert_eq!(action_ap(&CFG, None), CFG.players.basic_attack_ap);
    assert_eq!(action_ap(&CFG, Some(&stats(&presets::WARRIOR[1]))), 3); // Stun
    assert_eq!(action_ap(&CFG, Some(&stats(&presets::WARRIOR[0]))), 2);
}

#[test]
fn a_line_hits_every_unit_on_it_in_order() {
    let line = line_cells(c(1, 1), c(3, 1), CFG.players.ranged_range, |x| x.x < 7);
    assert_eq!(line.cells().len(), 5);
    let monsters = [(0u8, c(5, 1)), (1, c(3, 1)), (2, c(3, 2)), (3, c(7, 1))];
    let (t, n) = targets_on_line(&line, monsters.iter().copied());
    assert_eq!(&t[..n], &[1, 0], "the wall at x = 7 ends the line");
}

#[test]
fn heal_and_purpose_stay_wired() {
    // The mechanic of Heal stays for later: a buff with its own action.
    assert_eq!(
        deadroll_rules::input::effect_action(Effect::Heal),
        Some(EffectAction::Heal)
    );
    assert_eq!(presets::SUPPORT[4].blocks.purpose, Purpose::Buff);
    assert_eq!(SkillExt::MELEE.range, RangeTag::Melee);
}
