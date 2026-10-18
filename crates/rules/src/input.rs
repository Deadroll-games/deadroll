//! Builds `deadroll_combat` hit and effect inputs from heroes, skills and monsters. Pure.
use deadroll_combat::modifiers::more;
use deadroll_combat::DamageRange;
use deadroll_combat::{
    apply_mod, apply_state, apply_to_attacker, apply_to_defender, mod_range, mod_turns,
    states::freeze, AccuracyBuff, Applied, Attacker, Carrier, Defender, Mod, ModFilter, ModKind,
    Skill, States, SupportInput, TimedMod, TimedMods, Tint,
};
use deadroll_skillcheck::{
    derive_with, Category, Effect, Purpose, SkillBlocks, SkillExt, SkillTuning, SKILL_ASPECTS_MAX,
};
use deadroll_types::{
    ActiveState, Aspect, CharacterData, Class, Form, Race, RangeTag, Stat, StateKind, STATES_MAX,
    STATS_COUNT,
};

use crate::{
    config::BalanceConfig,
    items::{
        basic_attack, effective_stats, item_bonus, item_tints, weapon_range, Equipment, ITEM_TINTS,
    },
    monster::MonsterProfile,
    player::{
        class_increase, class_source, effect_stat, heal_range, heal_scale, hp_max, player_accuracy,
        player_defense, player_initiative, player_speed, predisposition_added, self_buff_turns,
        source_stat, speed_to_ap, stat_damage_increases, state_chance_bonus, support_increase,
    },
};

/// What the rules need from a character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeroStats {
    pub race: Race,
    pub class: Class,
    pub level: u16,
    /// Indexed by `Stat`.
    pub stats: [u16; STATS_COUNT],
    pub predisposition: Aspect,
    /// Race trait tier, `1..=14`, see `Tier`.
    pub predisposition_tier: u8,
    /// Class trait tier, `1..=14` (docs/CHARACTER.md, section 6.3).
    pub class_tier: u8,
    /// What the hero wears (`crate::items`).
    pub equipment: Equipment,
}

impl HeroStats {
    /// A character's core with its class trait tier (kept outside the frozen core), wearing
    /// nothing.
    pub fn new(c: &CharacterData, class_tier: u8) -> Self {
        HeroStats {
            race: c.race,
            class: c.class,
            level: c.level,
            stats: c.stats,
            predisposition: c.predisposition,
            predisposition_tier: c.predisposition_tier,
            class_tier,
            equipment: Equipment::EMPTY,
        }
    }

    pub fn with_equipment(self, equipment: Equipment) -> Self {
        HeroStats { equipment, ..self }
    }
}

/// Character numbers with the worn items (docs/CHARACTER.md, section 3: derived stats come from
/// stats, traits and equipment).
pub mod hero {
    use super::*;

    /// Core stats plus item stat bonuses.
    pub fn stats(cfg: &BalanceConfig, c: &HeroStats) -> [u16; STATS_COUNT] {
        effective_stats(&c.stats, &c.equipment, &cfg.items)
    }

    /// The damage base: the main-hand weapon, or bare hands.
    pub fn weapon(cfg: &BalanceConfig, c: &HeroStats) -> DamageRange {
        weapon_range(&c.equipment, &cfg.items)
    }

    /// Accuracy, the `d20` bonus of every attack and hostile effect: Dexterity and items.
    pub fn accuracy(cfg: &BalanceConfig, c: &HeroStats) -> u32 {
        let base = player_accuracy(&stats(cfg, c), &cfg.players);
        base.saturating_add(item_bonus(&c.equipment, &cfg.items).attack)
    }

    /// Crit chance besides a natural 20: items (traits and skills give none in the first
    /// version).
    pub fn crit_pct(cfg: &BalanceConfig, c: &HeroStats) -> u32 {
        item_bonus(&c.equipment, &cfg.items).crit_pct
    }

    /// Evasion (the hit threshold) and armor: class and stats, plus items.
    pub fn defense(cfg: &BalanceConfig, c: &HeroStats) -> (u32, u32) {
        let (defense, armor) = player_defense(c.class, &stats(cfg, c), &cfg.players);
        let items = item_bonus(&c.equipment, &cfg.items);
        (
            defense.saturating_add(items.defense),
            armor.saturating_add(items.armor),
        )
    }

    /// Action points per turn: speed from race, class and Dexterity, then items.
    pub fn ap(cfg: &BalanceConfig, c: &HeroStats) -> u8 {
        let dex = stats(cfg, c)[Stat::Dexterity as usize];
        let items = item_bonus(&c.equipment, &cfg.items);
        let speed = player_speed(c.race, c.class, dex, &cfg.players);
        speed_to_ap(speed, items.speed, items.speed_counter_bp, &cfg.players)
    }

    /// Initiative: Intelligence and items.
    pub fn initiative(cfg: &BalanceConfig, c: &HeroStats) -> u16 {
        let items = item_bonus(&c.equipment, &cfg.items);
        player_initiative(&stats(cfg, c), &cfg.players).saturating_add(items.initiative)
    }

    /// Increase on state and effect chances from `stat`, bp.
    pub fn state_chance(cfg: &BalanceConfig, c: &HeroStats, stat: Stat) -> u32 {
        state_chance_bonus(&stats(cfg, c), stat, &cfg.players)
    }

    /// Extra turns of the hero's own buffs on itself (Endurance).
    pub fn self_buff_turns(cfg: &BalanceConfig, c: &HeroStats) -> u8 {
        super::self_buff_turns(&stats(cfg, c), &cfg.players)
    }

    /// HP the items add: flat HP plus their Endurance.
    pub fn item_hp(cfg: &BalanceConfig, c: &HeroStats) -> u32 {
        let items = item_bonus(&c.equipment, &cfg.items);
        let endurance = items.stats[Stat::Endurance as usize];
        items
            .hp
            .saturating_add(u32::from(endurance).saturating_mul(cfg.players.hp_per_endurance))
    }

    /// Maximum HP from the hero's own numbers, items included. The program adds [`item_hp`] to the
    /// stored `CharacterData.hp_max`, which is this without items.
    pub fn hp_max(cfg: &BalanceConfig, c: &HeroStats) -> u32 {
        super::hp_max(c.class, &c.stats, c.level, &cfg.players).saturating_add(item_hp(cfg, c))
    }
}

/// A skill as stored with the character (`SkillRecord` and its patch 5 `SkillExtRecord`, without
/// the text).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillStats {
    pub blocks: SkillBlocks,
    pub shares_bp: [u32; SKILL_ASPECTS_MAX],
    pub power_bp: u32,
    /// In the character's turns, `0` = none.
    pub cooldown: u8,
    /// Range, timing, the HP threshold of a reactive trigger.
    pub ext: SkillExt,
    /// Action points to use it (`0` for reactive and passive skills).
    pub ap_cost: u8,
}

impl SkillStats {
    /// The stored values of `blocks`, derived with `t` (what `create_character` stores).
    pub fn derive(blocks: SkillBlocks, ext: SkillExt, t: &SkillTuning) -> Self {
        let d = derive_with(&blocks, t);
        SkillStats {
            blocks,
            shares_bp: d.shares_bp,
            power_bp: d.power_bp,
            cooldown: d.cooldown,
            ext,
            ap_cost: d.ap_cost,
        }
    }
}

/// Action points of an action: the skill's own cost, or the basic attack's
/// (docs/PARTY_AND_TURNS.md, section 5.1).
pub fn action_ap(cfg: &BalanceConfig, skill: Option<&SkillStats>) -> u8 {
    skill.map_or(cfg.players.basic_attack_ap, |s| s.ap_cost)
}

/// Why a skill cannot be used as an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillUse {
    Ok,
    /// Reactive and passive skills are not actions.
    NotActive,
    /// A buff or debuff without an effect (never passes the builder; the program checks anyway).
    NoEffect,
}

pub fn skill_use(skill: &SkillStats) -> SkillUse {
    if skill.blocks.category != Category::Active {
        SkillUse::NotActive
    } else if skill.blocks.purpose != Purpose::Damage
        && effect_action(skill.blocks.effect).is_none()
    {
        SkillUse::NoEffect
    } else {
        SkillUse::Ok
    }
}

/// Which side an action targets (docs/SKILL_BUILDER.md, section 2: a buff goes on the caster or
/// an ally, a debuff on an enemy).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// A monster: the weapon, damage and debuff skills.
    Enemy,
    /// A player of the party, the caster included: heal and buff skills.
    Ally,
}

/// The side of `skill` (`None` = the weapon's basic attack).
pub fn target_side(skill: Option<&SkillStats>) -> Side {
    match skill.map(|s| s.blocks.purpose) {
        None => Side::Enemy,
        Some(purpose) => match purpose {
            Purpose::Buff => Side::Ally,
            Purpose::Damage | Purpose::Debuff => Side::Enemy,
        },
    }
}

/// What a skill's effect does to its target (docs/SKILL_BUILDER.md, section 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectAction {
    /// HP back, up to the maximum.
    Heal,
    /// A timed modifier (`deadroll_combat::support`).
    Mod(ModKind),
    /// Hard control: the existing states.
    Stun,
    Freeze,
}

/// `None` for `Effect::None` (pure damage).
pub fn effect_action(effect: Effect) -> Option<EffectAction> {
    Some(match effect {
        Effect::None => return None,
        Effect::Weakening => EffectAction::Mod(ModKind::Weakening),
        Effect::Slow => EffectAction::Mod(ModKind::Slow),
        Effect::Vulnerability => EffectAction::Mod(ModKind::Vulnerability),
        Effect::Stun => EffectAction::Stun,
        Effect::Freeze => EffectAction::Freeze,
        Effect::Haste => EffectAction::Mod(ModKind::Haste),
        Effect::Empowerment => EffectAction::Mod(ModKind::Empowerment),
        Effect::Protection => EffectAction::Mod(ModKind::Protection),
        Effect::Heal => EffectAction::Heal,
    })
}

/// Form of a player attack: the skill's, or the weapon's basic attack's.
pub fn player_form(cfg: &BalanceConfig, c: &HeroStats, skill: Option<&SkillStats>) -> Form {
    skill.map_or_else(
        || basic_attack(&c.equipment, &cfg.items).1,
        |s| s.blocks.form,
    )
}

/// Range of a player action: the skill's, or the weapon's basic attack's (a sword strikes in
/// melee, a wand shoots, docs/SKILL_BUILDER.md 3.1).
pub fn player_range(cfg: &BalanceConfig, c: &HeroStats, skill: Option<&SkillStats>) -> RangeTag {
    skill.map_or_else(|| basic_attack(&c.equipment, &cfg.items).0, |s| s.ext.range)
}

/// Tint slots of a player: race, class, then items.
pub const RACE_TINT: usize = 0;
pub const CLASS_TINT: usize = 1;
pub const ITEM_TINT: usize = 2;

/// A player attacking with `skill` (`None` = the weapon's basic attack). `unit` is the
/// turn-order id, written as the source of applied states. `mods`: the player's buffs and
/// debuffs; `accuracy`: its accuracy buff from a resonance (Pale Flame).
pub fn player_attacker(
    cfg: &BalanceConfig,
    unit: u8,
    c: &HeroStats,
    skill: Option<&SkillStats>,
    mods: &TimedMods,
    accuracy: &AccuracyBuff,
) -> Attacker {
    let t = &cfg.players;
    let source = skill.map_or(class_source(c.class), |s| s.blocks.source);
    let combat_skill = match skill {
        Some(s) => Skill {
            aspects: s.blocks.aspects,
            aspect_count: s.blocks.aspect_count,
            shares_bp: s.shares_bp,
            power_bp: s.power_bp,
            source,
            form: s.blocks.form,
            state_chance_pct: t.skill_state_chance_pct,
        },
        None => Skill::basic(source, player_form(cfg, c, None), t.basic_state_chance_pct),
    };
    let mut attacker = Attacker {
        slot: unit,
        attack_bonus: hero::accuracy(cfg, c),
        crit_chance_pct: hero::crit_pct(cfg, c),
        weapon: hero::weapon(cfg, c),
        skill: combat_skill,
        // The source picks the stat that scales its states; the aspect picks the state.
        state_chance_bonus_bp: hero::state_chance(cfg, c, source_stat(source)),
        tempo_bonus_bp: hero::state_chance(cfg, c, Stat::Dexterity),
        ..Attacker::default()
    };
    // Race trait: the predisposition adds a flat packet and tints resonances (CHARACTER 6.2),
    // both at the race trait tier.
    attacker.added[0] = predisposition_added(c.predisposition, c.predisposition_tier, t);
    attacker.tints[RACE_TINT] = Tint::new(c.predisposition, c.predisposition_tier, Carrier::Race);
    // CHARACTER 8.1 gives no class an aspect in the first version: the class slot stays empty.
    // Worn items with an aspect tint too (the weapon's own aspect, SKILL_BUILDER 5.2).
    attacker.tints[ITEM_TINT..ITEM_TINT + ITEM_TINTS].copy_from_slice(&item_tints(
        &c.equipment,
        &cfg.items,
        skill.is_none(),
    ));
    let (bases, count) = attacker.skill.bases(attacker.weapon.aspect);
    attacker.attack_bonus = attacker
        .attack_bonus
        .saturating_add(accuracy.bonus_for(&bases[..count]));
    // Class trait at its own tier, then the core stats in slots 1-3 (empty when 0).
    if let Some((tag, bp)) = class_increase(c.class, c.class_tier, t) {
        attacker.increased[0] = Mod::new(ModFilter::Source(tag), bp);
    }
    let stats = stat_damage_increases(&hero::stats(cfg, c), t);
    attacker.increased[1..=stats.len()].copy_from_slice(&stats);
    apply_to_attacker(&mut attacker, mods);
    attacker
}

/// A player as the target.
pub fn player_defender(
    cfg: &BalanceConfig,
    c: &HeroStats,
    states: &[ActiveState; STATES_MAX],
    mods: &TimedMods,
) -> Defender {
    let (defense, armor) = hero::defense(cfg, c);
    let mut defender = Defender {
        defense,
        armor,
        states: *states,
        ..Defender::default()
    };
    apply_to_defender(&mut defender, mods);
    defender
}

/// A monster attacking with its natural attack (`skill = None`, like a basic attack) or a damage
/// skill: the natural attack is its weapon base (docs/BESTIARY.md 2, SKILL_BUILDER.md 5.1-5.2).
/// Monsters have no core stats, so no stat increases, and no tints (no predisposition yet).
pub fn monster_attacker(
    cfg: &BalanceConfig,
    unit: u8,
    p: &MonsterProfile,
    skill: Option<&SkillStats>,
    mods: &TimedMods,
) -> Attacker {
    let skill = match skill {
        Some(s) => Skill {
            aspects: s.blocks.aspects,
            aspect_count: s.blocks.aspect_count,
            shares_bp: s.shares_bp,
            power_bp: s.power_bp,
            source: s.blocks.source,
            form: s.blocks.form,
            state_chance_pct: cfg.players.skill_state_chance_pct,
        },
        None => Skill::basic(p.source, p.form, cfg.players.basic_state_chance_pct),
    };
    let mut attacker = Attacker {
        slot: unit,
        attack_bonus: p.attack_bonus,
        weapon: p.weapon,
        skill,
        ..Attacker::default()
    };
    apply_to_attacker(&mut attacker, mods);
    attacker
}

/// A monster as the target.
pub fn monster_defender(p: &MonsterProfile, states: &States, mods: &TimedMods) -> Defender {
    let mut defender = Defender {
        defense: p.defense,
        armor: p.armor,
        resist_bp: p.resist_bp,
        immune_aspects: p.immune_aspects,
        immune_states: p.immune_states,
        states: *states,
        ..Defender::default()
    };
    apply_to_defender(&mut defender, mods);
    defender
}

/// Strength range of `action` cast by `c` with a skill of `power_bp`: HP for a heal (from the
/// hero's weapon), bp for a modifier, none (`(0, 0)`) for hard control.
pub fn effect_range(
    cfg: &BalanceConfig,
    c: &HeroStats,
    power_bp: u32,
    action: EffectAction,
) -> (u32, u32) {
    match action {
        EffectAction::Heal => heal_range(
            c.class,
            c.class_tier,
            hero::weapon(cfg, c),
            power_bp,
            &cfg.players,
        ),
        EffectAction::Mod(kind) => mod_range(
            kind,
            power_bp,
            support_increase(c.class, c.class_tier, &cfg.players),
            &cfg.support,
        ),
        EffectAction::Stun | EffectAction::Freeze => (0, 0),
    }
}

/// The rolls of a heal, buff or debuff cast by `c` with `skill`. `defense`: the enemy's, for a
/// debuff (it rolls to land); `None` for a heal or buff on an ally.
pub fn support_input(
    cfg: &BalanceConfig,
    c: &HeroStats,
    skill: &SkillStats,
    action: EffectAction,
    defense: Option<u32>,
) -> SupportInput {
    let (min, max) = effect_range(cfg, c, skill.power_bp, action);
    SupportInput {
        hostile: defense.is_some(),
        attack_bonus: hero::accuracy(cfg, c),
        defense: defense.unwrap_or(0),
        crit_chance_pct: hero::crit_pct(cfg, c),
        min,
        max,
    }
}

/// Strength range of `action` cast by a monster with a skill of `power_bp`: a heal from its natural
/// attack, a modifier with no increase (monsters have no class trait), none for hard control.
pub fn monster_effect_range(
    cfg: &BalanceConfig,
    p: &MonsterProfile,
    power_bp: u32,
    action: EffectAction,
) -> (u32, u32) {
    match action {
        EffectAction::Heal => heal_scale(p.weapon, power_bp, 0, &cfg.players),
        EffectAction::Mod(kind) => mod_range(kind, power_bp, 0, &cfg.support),
        EffectAction::Stun | EffectAction::Freeze => (0, 0),
    }
}

/// The rolls of a monster's heal, buff or debuff: its accuracy, no crit chance besides a natural
/// 20. `defense`: the target's for a debuff, `None` on an ally.
pub fn monster_support_input(
    cfg: &BalanceConfig,
    p: &MonsterProfile,
    skill: &SkillStats,
    action: EffectAction,
    defense: Option<u32>,
) -> SupportInput {
    let (min, max) = monster_effect_range(cfg, p, skill.power_bp, action);
    SupportInput {
        hostile: defense.is_some(),
        attack_bonus: p.attack_bonus,
        defense: defense.unwrap_or(0),
        crit_chance_pct: 0,
        min,
        max,
    }
}

/// Chance of a damage skill's effect on a hit (the rider): the native chance, increased by the
/// stat of the effect (Dexterity for tempo, else the skill's source).
pub fn effect_chance(cfg: &BalanceConfig, c: &HeroStats, skill: &SkillStats) -> u32 {
    let base = cfg.support.rider_chance_pct;
    match effect_stat(skill.blocks.effect, skill.blocks.source) {
        Some(stat) => {
            let bonus = hero::state_chance(cfg, c, stat);
            more(base as u64, bonus).min(u32::MAX as u64) as u32
        }
        None => base,
    }
}

/// Extra turns of a modifier `c` lands: Endurance lengthens its own buffs on itself
/// (`on_self`), nothing else (CHARACTER 2).
pub fn buff_extra_turns(
    cfg: &BalanceConfig,
    c: &HeroStats,
    on_self: bool,
    action: EffectAction,
) -> u8 {
    match action {
        EffectAction::Mod(kind) if on_self && !kind.is_harmful() => hero::self_buff_turns(cfg, c),
        EffectAction::Mod(_) | EffectAction::Heal | EffectAction::Stun | EffectAction::Freeze => 0,
    }
}

/// Lands `action` (not a heal) of strength `amount` on a unit: a modifier for its duration plus
/// `extra_turns` ([`buff_extra_turns`]), or the existing Stun / Freeze state. `aspect`: the
/// skill's; `source`: the caster's turn-order id.
#[allow(clippy::too_many_arguments)]
pub fn land_effect(
    cfg: &BalanceConfig,
    action: EffectAction,
    amount: u32,
    aspect: Aspect,
    source: u8,
    extra_turns: u8,
    states: &mut States,
    mods: &mut TimedMods,
) -> Applied {
    match action {
        EffectAction::Mod(kind) => apply_mod(
            mods,
            TimedMod {
                kind,
                aspect,
                turns: mod_turns(cfg.support.duration(kind).saturating_add(extra_turns)),
                power_bp: amount,
                source,
            },
        ),
        EffectAction::Stun => apply_state(
            states,
            ActiveState {
                kind: StateKind::Stun,
                turns: cfg.combat.turns(StateKind::Stun),
                power: 0,
                source,
            },
        ),
        EffectAction::Freeze => freeze(states, source, &cfg.combat),
        EffectAction::Heal => Applied::None,
    }
}

/// HP after a heal: up to the maximum, never down (docs/COMBAT_MATH.md, 11).
pub fn heal(hp: u32, hp_max: u32, amount: u32) -> u32 {
    hp.saturating_add(amount).min(hp_max).max(hp)
}

/// HP after `damage`, and whether that killed (HP never goes below 0).
pub fn take_damage(hp: u32, damage: u32) -> (u32, bool) {
    let left = hp.saturating_sub(damage);
    (left, hp > 0 && left == 0)
}
