//! Builds `deadroll_combat` hit and effect inputs from heroes, skills and monsters. Pure.
use deadroll_combat::{
    apply_mod, apply_state, apply_to_attacker, apply_to_defender, mod_range, mod_turns,
    states::freeze, Applied, Attacker, Carrier, Defender, Mod, ModFilter, ModKind, Skill, States,
    SupportInput, TimedMod, TimedMods, Tint,
};
use deadroll_skillcheck::{
    derive_with, Category, Effect, Purpose, SkillBlocks, SkillTuning, SKILL_ASPECTS_MAX,
};
use deadroll_types::{
    ActiveState, Aspect, CharacterData, Class, Form, Race, StateKind, STATES_MAX, STATS_COUNT,
};

use crate::{
    config::BalanceConfig,
    monster::MonsterProfile,
    player::{
        class_increase, class_source, class_weapon, heal_range, player_attack_bonus,
        player_crit_bp, player_defense, predisposition_added, support_increase,
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
    /// `1..=14`, see `Tier`.
    pub predisposition_tier: u8,
}

impl From<&CharacterData> for HeroStats {
    fn from(c: &CharacterData) -> Self {
        HeroStats {
            race: c.race,
            class: c.class,
            level: c.level,
            stats: c.stats,
            predisposition: c.predisposition,
            predisposition_tier: c.predisposition_tier,
        }
    }
}

/// A skill as stored with the character (`SkillRecord` without the name).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillStats {
    pub blocks: SkillBlocks,
    pub shares_bp: [u32; SKILL_ASPECTS_MAX],
    pub power_bp: u32,
    /// In the character's turns, `0` = none.
    pub cooldown: u8,
}

impl SkillStats {
    /// The stored values of `blocks`, derived with `t` (what `create_character` stores).
    pub fn derive(blocks: SkillBlocks, t: &SkillTuning) -> Self {
        let d = derive_with(&blocks, t);
        SkillStats {
            blocks,
            shares_bp: d.shares_bp,
            power_bp: d.power_bp,
            cooldown: d.cooldown,
        }
    }
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

/// Form of a player attack: the skill's, or a strike for the weapon's basic attack.
pub fn player_form(skill: Option<&SkillStats>) -> Form {
    skill.map_or(Form::Strike, |s| s.blocks.form)
}

/// A player attacking with `skill` (`None` = the weapon's basic attack). `unit` is the
/// turn-order id, written as the source of applied states. `mods`: the player's buffs and debuffs.
pub fn player_attacker(
    cfg: &BalanceConfig,
    unit: u8,
    c: &HeroStats,
    skill: Option<&SkillStats>,
    mods: &TimedMods,
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
            state_chance_bp: t.skill_state_chance_bp,
        },
        None => Skill::basic(source, Form::Strike, t.basic_state_chance_bp),
    };
    let mut attacker = Attacker {
        slot: unit,
        attack_bonus: player_attack_bonus(c.level, &c.stats, source, t),
        crit_chance_bp: player_crit_bp(&c.stats, t),
        weapon: class_weapon(c.class, t),
        skill: combat_skill,
        ..Attacker::default()
    };
    // Race trait: the predisposition adds a flat packet and tints resonances (CHARACTER 6.2).
    attacker.added[0] = predisposition_added(c.predisposition, t);
    attacker.tints[0] = Tint::new(c.predisposition, c.predisposition_tier, Carrier::Race);
    if let Some((tag, bp)) = class_increase(c.class, t) {
        attacker.increased[0] = Mod::new(ModFilter::Source(tag), bp);
    }
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
    let (defense, armor) = player_defense(c.class, &c.stats, &cfg.players);
    let mut defender = Defender {
        defense,
        armor,
        states: *states,
        ..Defender::default()
    };
    apply_to_defender(&mut defender, mods);
    defender
}

/// A monster attacking with its natural attack.
pub fn monster_attacker(
    cfg: &BalanceConfig,
    unit: u8,
    p: &MonsterProfile,
    mods: &TimedMods,
) -> Attacker {
    let mut attacker = Attacker {
        slot: unit,
        attack_bonus: p.attack_bonus,
        weapon: p.weapon,
        skill: Skill::basic(p.source, p.form, cfg.players.basic_state_chance_bp),
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
        states: *states,
        ..Defender::default()
    };
    apply_to_defender(&mut defender, mods);
    defender
}

/// Strength range of `action` cast by a `class` with a skill of `power_bp`: HP for a heal, bp for
/// a modifier, none (`(0, 0)`) for hard control.
pub fn effect_range(
    cfg: &BalanceConfig,
    class: Class,
    power_bp: u32,
    action: EffectAction,
) -> (u32, u32) {
    match action {
        EffectAction::Heal => heal_range(class, power_bp, &cfg.players),
        EffectAction::Mod(kind) => mod_range(
            kind,
            power_bp,
            support_increase(class, &cfg.players),
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
    let (min, max) = effect_range(cfg, c.class, skill.power_bp, action);
    SupportInput {
        hostile: defense.is_some(),
        attack_bonus: player_attack_bonus(c.level, &c.stats, skill.blocks.source, &cfg.players),
        defense: defense.unwrap_or(0),
        crit_chance_bp: player_crit_bp(&c.stats, &cfg.players),
        min,
        max,
    }
}

/// Lands `action` (not a heal) of strength `amount` on a unit: a modifier for its duration, or
/// the existing Stun / Freeze state. `aspect`: the skill's; `source`: the caster's turn-order id.
pub fn land_effect(
    cfg: &BalanceConfig,
    action: EffectAction,
    amount: u32,
    aspect: Aspect,
    source: u8,
    states: &mut States,
    mods: &mut TimedMods,
) -> Applied {
    match action {
        EffectAction::Mod(kind) => apply_mod(
            mods,
            TimedMod {
                kind,
                aspect,
                turns: mod_turns(cfg.support.duration(kind)),
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
