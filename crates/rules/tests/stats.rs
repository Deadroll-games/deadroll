//! Core stats and trait tiers (docs/CHARACTER.md, sections 2-6): what each stat scales, which
//! stat scales a state (the skill's power source decides), trait tiers, level-up points.
use deadroll_combat::{mod_turns, modifiers::sum_matching, ModKind, TimedMod, TimedMods, MODS_MAX};
use deadroll_rules::{
    input::{
        buff_extra_turns, effect_chance, hero, land_effect, player_attacker, EffectAction,
        HeroStats, SkillStats,
    },
    items::Equipment,
    player::{
        effect_stat, level_for, level_up_points, predisposition_added, source_stat, spend_points,
        tier_index, SpendError,
    },
    BalanceConfig,
};
use deadroll_skillcheck::{Category, Effect, Purpose, SkillBlocks};
use deadroll_types::{
    ActiveState, Aspect, Class, Form, ItemBase, Race, Source, Stat, BP, STATES_MAX,
};

const CFG: BalanceConfig = BalanceConfig::DEFAULT;
const NONE: TimedMods = [TimedMod {
    kind: ModKind::Haste,
    aspect: Aspect::Fire,
    turns: 0,
    power_bp: 0,
    source: 0,
}; MODS_MAX];

fn hero(class: Class, stats: [u16; 4]) -> HeroStats {
    HeroStats {
        race: Race::Human,
        class,
        level: 1,
        stats,
        predisposition: Aspect::Piercing,
        predisposition_tier: 14,
        class_tier: 14,
        equipment: Equipment::EMPTY,
    }
}

fn skill(
    purpose: Purpose,
    source: Source,
    form: Form,
    aspect: Aspect,
    effect: Effect,
) -> SkillStats {
    let blocks = SkillBlocks {
        category: Category::Active,
        purpose,
        source,
        form,
        aspects: [aspect, Aspect::default(), Aspect::default()],
        aspect_count: 1,
        effect,
        ..SkillBlocks::default()
    };
    SkillStats::derive(blocks, &CFG.skills)
}

fn damage(source: Source, form: Form, aspect: Aspect) -> SkillStats {
    skill(Purpose::Damage, source, form, aspect, Effect::None)
}

/// Summed increase on a packet of `aspect` from `skill`'s source and form.
fn increase(h: &HeroStats, s: &SkillStats, aspect: Aspect) -> u32 {
    let a = player_attacker(&CFG, 0, h, Some(s), &NONE);
    sum_matching(&a.increased, aspect, a.skill.source, a.skill.form)
}

#[test]
fn stat_damage_goes_by_source_and_form() {
    let t = &CFG.players;
    // A mage (no martial class increase) with only Strength, then only Dexterity, then only
    // Intelligence above zero.
    let strong = hero(Class::Support, [10, 0, 0, 0]);
    let quick = hero(Class::Support, [0, 10, 0, 0]);
    let smart = hero(Class::Support, [0, 0, 10, 0]);
    let martial_strike = damage(Source::Combat, Form::Strike, Aspect::Blunt);
    let martial_shot = damage(Source::Combat, Form::Projectile, Aspect::Piercing);
    let magic_strike = damage(Source::Magical, Form::Strike, Aspect::Fire);
    let magic_shot = damage(Source::Magical, Form::Projectile, Aspect::Cold);

    // Strength: martial strikes only.
    assert_eq!(
        increase(&strong, &martial_strike, Aspect::Blunt),
        10 * t.str_damage_bp
    );
    assert_eq!(increase(&strong, &martial_shot, Aspect::Piercing), 0);
    assert_eq!(increase(&strong, &magic_strike, Aspect::Fire), 0);
    // Dexterity: martial projectiles only.
    assert_eq!(
        increase(&quick, &martial_shot, Aspect::Piercing),
        10 * t.dex_projectile_bp
    );
    assert_eq!(increase(&quick, &martial_strike, Aspect::Blunt), 0);
    assert_eq!(increase(&quick, &magic_shot, Aspect::Cold), 0);
    // Intelligence: every magical packet, strike or projectile; no martial one.
    assert_eq!(
        increase(&smart, &magic_strike, Aspect::Fire),
        10 * t.int_magic_bp
    );
    assert_eq!(
        increase(&smart, &magic_shot, Aspect::Cold),
        10 * t.int_magic_bp
    );
    assert_eq!(increase(&smart, &martial_strike, Aspect::Blunt), 0);
}

#[test]
fn accuracy_from_dexterity_and_items_only() {
    let t = &CFG.players;
    let base = hero(Class::Warrior, [5, 10, 5, 5]);
    let acc = hero::accuracy(&CFG, &base);
    assert_eq!(acc, t.accuracy_base + 10 / t.accuracy_dex_div);
    // Level, Strength and Intelligence do not move it.
    let other = HeroStats {
        level: 4,
        stats: [30, 10, 30, 5],
        ..base
    };
    assert_eq!(hero::accuracy(&CFG, &other), acc);
    // Every attack and skill uses the same accuracy, whatever its source.
    let magic = damage(Source::Magical, Form::Projectile, Aspect::Cold);
    let a = player_attacker(&CFG, 0, &other, Some(&magic), &NONE);
    let b = player_attacker(&CFG, 0, &other, None, &NONE);
    assert_eq!((a.attack_bonus, b.attack_bonus), (acc, acc));
}

#[test]
fn crit_only_from_items() {
    let mut cfg = CFG;
    let nimble = hero(Class::Warrior, [5, 30, 5, 5]);
    assert_eq!(hero::crit_pct(&cfg, &nimble), 0);
    cfg.items.bases[ItemBase::Sword as usize].crit_pct = 6;
    let mut eq = Equipment::EMPTY;
    eq.set(deadroll_types::EquipSlot::MainHand, Some(ItemBase::Sword));
    let armed = nimble.with_equipment(eq);
    assert_eq!(hero::crit_pct(&cfg, &armed), 6);
    assert_eq!(
        player_attacker(&cfg, 0, &armed, None, &NONE).crit_chance_pct,
        6
    );
}

#[test]
fn the_source_picks_the_state_stat() {
    let t = &CFG.players;
    assert_eq!(source_stat(Source::Combat), Stat::Strength);
    assert_eq!(source_stat(Source::Magical), Stat::Intelligence);
    let h = hero(Class::Warrior, [10, 20, 30, 5]);
    // A blood mage bleeds through Intelligence, a fire warrior burns through Strength.
    let blood_magic = damage(Source::Magical, Form::Projectile, Aspect::Blood);
    let fire_blade = damage(Source::Combat, Form::Strike, Aspect::Fire);
    let a = player_attacker(&CFG, 0, &h, Some(&blood_magic), &NONE);
    assert_eq!(a.state_chance_bonus_bp, 30 * t.int_state_chance_bp);
    let a = player_attacker(&CFG, 0, &h, Some(&fire_blade), &NONE);
    assert_eq!(a.state_chance_bonus_bp, 10 * t.str_state_chance_bp);
    // The basic attack uses the class's source.
    let mage = HeroStats {
        class: Class::Mage,
        ..h
    };
    assert_eq!(
        player_attacker(&CFG, 0, &mage, None, &NONE).state_chance_bonus_bp,
        30 * t.int_state_chance_bp
    );
    assert_eq!(
        player_attacker(&CFG, 0, &h, None, &NONE).state_chance_bonus_bp,
        10 * t.str_state_chance_bp
    );
}

#[test]
fn effect_chance_by_effect_stat() {
    let t = &CFG.players;
    let rider = CFG.support.rider_chance_pct;
    let scaled = |stat_points: u32, per: u32| {
        (rider as u64 * (BP as u64 + (stat_points * per) as u64) / BP as u64) as u32
    };
    let h = hero(Class::Warrior, [10, 20, 30, 5]);
    // Tempo follows Dexterity whatever the source.
    for source in [Source::Combat, Source::Magical] {
        assert_eq!(effect_stat(Effect::Slow, source), Some(Stat::Dexterity));
        assert_eq!(effect_stat(Effect::Haste, source), Some(Stat::Dexterity));
        let slow = skill(
            Purpose::Damage,
            source,
            Form::Projectile,
            Aspect::Cold,
            Effect::Slow,
        );
        assert_eq!(
            effect_chance(&CFG, &h, &slow),
            scaled(20, t.dex_state_chance_bp)
        );
    }
    // Hard control and debuffs follow the source.
    let stun = skill(
        Purpose::Damage,
        Source::Combat,
        Form::Strike,
        Aspect::Blunt,
        Effect::Stun,
    );
    assert_eq!(
        effect_chance(&CFG, &h, &stun),
        scaled(10, t.str_state_chance_bp)
    );
    let freeze = skill(
        Purpose::Damage,
        Source::Magical,
        Form::Projectile,
        Aspect::Cold,
        Effect::Freeze,
    );
    assert_eq!(
        effect_chance(&CFG, &h, &freeze),
        scaled(30, t.int_state_chance_bp)
    );
    let weaken = skill(
        Purpose::Damage,
        Source::Combat,
        Form::Strike,
        Aspect::Fire,
        Effect::Weakening,
    );
    assert_eq!(
        effect_chance(&CFG, &h, &weaken),
        scaled(10, t.str_state_chance_bp)
    );
    let expose = skill(
        Purpose::Damage,
        Source::Magical,
        Form::Strike,
        Aspect::Blood,
        Effect::Vulnerability,
    );
    assert_eq!(
        effect_chance(&CFG, &h, &expose),
        scaled(30, t.int_state_chance_bp)
    );
    // Effects that never roll a chance have no stat.
    for e in [
        Effect::None,
        Effect::Empowerment,
        Effect::Protection,
        Effect::Heal,
    ] {
        assert_eq!(effect_stat(e, Source::Combat), None);
    }
}

#[test]
fn endurance_lengthens_own_buffs_on_self_only() {
    let t = &CFG.players;
    let tough = hero(Class::Support, [5, 5, 5, 10]);
    let haste = EffectAction::Mod(ModKind::Haste);
    let extra = (10 / t.self_buff_end_div).min(t.self_buff_turns_max as u16) as u8;
    assert!(extra > 0);
    assert_eq!(buff_extra_turns(&CFG, &tough, true, haste), extra);
    // On an ally, a debuff, or hard control: nothing.
    assert_eq!(buff_extra_turns(&CFG, &tough, false, haste), 0);
    assert_eq!(
        buff_extra_turns(&CFG, &tough, true, EffectAction::Mod(ModKind::Slow)),
        0
    );
    assert_eq!(buff_extra_turns(&CFG, &tough, true, EffectAction::Stun), 0);
    // Capped.
    let huge = hero(Class::Support, [5, 5, 5, 500]);
    assert_eq!(
        buff_extra_turns(&CFG, &huge, true, haste),
        t.self_buff_turns_max
    );
    // The modifier lands for its duration plus the extra turns.
    let mut states = [ActiveState::default(); STATES_MAX];
    let mut mods = NONE;
    land_effect(
        &CFG,
        haste,
        3_000,
        Aspect::Blood,
        0,
        extra,
        &mut states,
        &mut mods,
    );
    let turns = CFG.support.duration(ModKind::Haste) + extra;
    assert_eq!(mods[0].turns, mod_turns(turns));
}

#[test]
fn trait_tiers_are_separate() {
    let t = &CFG.players;
    assert_eq!(
        (tier_index(1), tier_index(14), tier_index(0), tier_index(99)),
        (0, 13, 0, 13)
    );
    // Race tier: the added packet; class tier: the class increase.
    let worst = predisposition_added(Aspect::Blood, 14, t);
    let best = predisposition_added(Aspect::Blood, 1, t);
    assert!(best.max > worst.max && best.min >= worst.min);
    let low = hero(Class::Warrior, [10, 5, 5, 5]);
    let race_up = HeroStats {
        predisposition_tier: 1,
        ..low
    };
    let class_up = HeroStats {
        class_tier: 1,
        ..low
    };
    let a = |h: &HeroStats| player_attacker(&CFG, 0, h, None, &NONE);
    assert_eq!(
        a(&race_up).added[0],
        predisposition_added(Aspect::Piercing, 1, t)
    );
    assert_eq!(a(&race_up).increased[0], a(&low).increased[0]);
    assert_eq!(a(&class_up).added[0], a(&low).added[0]);
    assert!(a(&class_up).increased[0].bp > a(&low).increased[0].bp);
}

#[test]
fn levels_and_points() {
    let t = &CFG.players;
    assert_eq!(level_for(0, t), 1);
    assert_eq!(level_for(t.level_xp[1] - 1, t), 1);
    assert_eq!(level_for(t.level_xp[1], t), 2);
    assert_eq!(level_for(u32::MAX, t), t.level_xp.len() as u16);
    assert_eq!(level_up_points(1, 3, t), Some(2 * t.stat_points_per_level));
    assert_eq!(level_up_points(3, 3, t), Some(0));
    assert_eq!(level_up_points(1, 300, t), None);
}

#[test]
fn spending_points() {
    let stats = [10, 5, 5, 10];
    assert_eq!(
        spend_points(&stats, 4, &[1, 0, 2, 1]),
        Ok(([11, 5, 7, 11], 0))
    );
    assert_eq!(
        spend_points(&stats, 4, &[0, 1, 0, 0]),
        Ok(([10, 6, 5, 10], 3))
    );
    assert_eq!(spend_points(&stats, 4, &[0; 4]), Err(SpendError::Nothing));
    assert_eq!(spend_points(&stats, 0, &[0; 4]), Err(SpendError::Nothing));
    assert_eq!(
        spend_points(&stats, 2, &[1, 1, 1, 0]),
        Err(SpendError::NotEnough)
    );
    assert_eq!(
        spend_points(&stats, 0, &[1, 0, 0, 0]),
        Err(SpendError::NotEnough)
    );
    assert_eq!(
        spend_points(&[u16::MAX, 5, 5, 5], 3, &[1, 0, 0, 0]),
        Err(SpendError::Overflow)
    );
}
