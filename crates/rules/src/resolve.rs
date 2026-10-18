//! One action resolved from its dice: what the program's VRF callback does with the state, without
//! accounts or events (the caller emits them from the results).
//!
//! Every function takes one [`RollSource`] for the whole action and keeps reading it: a skill's
//! effect rolls on the indices after the hit, so a stream at index `rolls_used` is the same as
//! `VrfRoll` / `SyscallRoll` started there. `HitInput` and `HitResult` come from the caller so the
//! program can keep them off the 4 KB SBF stack frame.
use deadroll_combat::{
    apply_mod, resolve_hit_into, resolve_rider, resolve_support, AccuracyBuff, Applied, EffectRoll,
    HitInput, HitResult, Outcome, ResonanceTables, RollSource, States, TimedMods,
};
use deadroll_types::Monster;

use crate::{
    aura::{aura_kind, reactive_aura, Aura},
    config::BalanceConfig,
    input::{
        buff_extra_turns, effect_action, effect_chance, effect_range, heal, land_effect,
        monster_attacker, monster_defender, monster_effect_range, monster_support_input,
        player_attacker, player_defender, support_input, take_damage, EffectAction, HeroStats,
        SkillStats,
    },
    monster::{MonsterProfile, MonsterSkillStats},
};

/// The effect of a damage skill after its hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RiderOutcome {
    pub roll: EffectRoll,
    pub applied: Applied,
}

/// What an attack did besides the `HitResult` (a player's on a monster, a monster's on a player).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttackOutcome {
    /// The target reached 0 HP (a monster's `alive` is cleared).
    pub died: bool,
    /// The skill's effect, rolled when the hit landed and the monster lives.
    pub rider: Option<RiderOutcome>,
}

/// A player's weapon (`skill = None`) or damage skill on a monster, then that skill's effect
/// (if any) when the hit landed and the monster lives: its own chance (certain on a crit).
/// The resonance's timed modifiers land on the monster here (`monster_mods`, its own), its
/// accuracy buff on `accuracy`; what it does to other monsters and cells is `crate::spread`.
/// `hero_mods` and `monster_view` are the modifiers the hit sees: their own plus the auras
/// over them (`crate::aura`).
#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub fn attack_monster(
    cfg: &BalanceConfig,
    tables: &ResonanceTables,
    attacker: u8,
    hero: &HeroStats,
    skill: Option<&SkillStats>,
    hero_mods: &TimedMods,
    accuracy: &mut AccuracyBuff,
    profile: &MonsterProfile,
    monster: &mut Monster,
    monster_mods: &mut TimedMods,
    monster_view: &TimedMods,
    input: &mut HitInput,
    result: &mut HitResult,
    src: &mut impl RollSource,
) -> AttackOutcome {
    input.attacker = player_attacker(cfg, attacker, hero, skill, hero_mods, accuracy);
    input.defender = monster_defender(profile, &monster.states, monster_view);
    resolve_hit_into(input, &cfg.combat, tables, src, result);
    for m in result.target_mods() {
        apply_mod(monster_mods, *m);
    }
    accuracy.apply(result.accuracy_buff);

    let (hp, died) = take_damage(monster.hp, result.damage);
    monster.hp = hp;
    monster.states = result.states;
    if died {
        monster.alive = false;
    }

    let mut rider = None;
    if let Some(skill) = skill {
        if let Some(effect) = effect_action(skill.blocks.effect) {
            if result.outcome == Outcome::Hit && monster.alive {
                let (min, max) = effect_range(cfg, hero, skill.power_bp, effect);
                let roll = resolve_rider(
                    effect_chance(cfg, hero, skill),
                    result.crit,
                    min,
                    max,
                    &cfg.combat,
                    src,
                );
                let applied = if roll.outcome == Outcome::Hit {
                    land_effect(
                        cfg,
                        effect,
                        roll.amount,
                        skill.blocks.aspects[0],
                        attacker,
                        0,
                        &mut monster.states,
                        monster_mods,
                    )
                } else {
                    Applied::None
                };
                rider = Some(RiderOutcome { roll, applied });
            }
        }
    }
    AttackOutcome { died, rider }
}

/// A debuff skill on a monster: it rolls to land (d20 against the monster's defense).
#[allow(clippy::too_many_arguments)]
pub fn debuff_monster(
    cfg: &BalanceConfig,
    caster: u8,
    hero: &HeroStats,
    skill: &SkillStats,
    effect: EffectAction,
    profile: &MonsterProfile,
    monster: &mut Monster,
    monster_mods: &mut TimedMods,
    src: &mut impl RollSource,
) -> RiderOutcome {
    let defense = monster_defender(profile, &monster.states, monster_mods).defense;
    let input = support_input(cfg, hero, skill, effect, Some(defense));
    let roll = resolve_support(&input, &cfg.combat, src);
    let applied = if roll.outcome == Outcome::Hit && monster.alive {
        land_effect(
            cfg,
            effect,
            roll.amount,
            skill.blocks.aspects[0],
            caster,
            0,
            &mut monster.states,
            monster_mods,
        )
    } else {
        Applied::None
    };
    RiderOutcome { roll, applied }
}

/// A heal or buff on the caster or an ally: no roll to land, a crit roll and the d100 strength.
/// A dead target (`hp == 0`) is never raised. `on_self`: the caster is the target (its own buffs
/// on itself last longer with Endurance).
#[allow(clippy::too_many_arguments)]
pub fn support_ally(
    cfg: &BalanceConfig,
    caster: u8,
    hero: &HeroStats,
    skill: &SkillStats,
    effect: EffectAction,
    on_self: bool,
    hp: &mut u32,
    hp_max: u32,
    states: &mut States,
    mods: &mut TimedMods,
    src: &mut impl RollSource,
) -> RiderOutcome {
    let input = support_input(cfg, hero, skill, effect, None);
    let roll = resolve_support(&input, &cfg.combat, src);
    let applied = if *hp == 0 {
        Applied::None
    } else if effect == EffectAction::Heal {
        *hp = heal(*hp, hp_max, roll.amount);
        Applied::None
    } else {
        land_effect(
            cfg,
            effect,
            roll.amount,
            skill.blocks.aspects[0],
            caster,
            buff_extra_turns(cfg, hero, on_self, effect),
            states,
            mods,
        )
    };
    RiderOutcome { roll, applied }
}

/// A reactive skill's temporary aura around its bearer (docs/SKILL_BUILDER.md 3.5): no roll to
/// land, a crit roll and the d100 strength, like a buff. `None` with the roll when the skill is
/// not an aura.
pub fn start_aura(
    cfg: &BalanceConfig,
    hero: &HeroStats,
    skill: &SkillStats,
    src: &mut impl RollSource,
) -> (EffectRoll, Option<Aura>) {
    let Some(kind) = aura_kind(skill) else {
        return (EffectRoll::default(), None);
    };
    let input = support_input(cfg, hero, skill, EffectAction::Mod(kind), None);
    let roll = resolve_support(&input, &cfg.combat, src);
    (roll, reactive_aura(cfg, skill, roll.amount))
}

/// A monster's natural attack (`skill = None`) or damage skill on a player, then that skill's
/// effect when the hit landed and the player lives: the rider's own chance (certain on a crit),
/// on the player's stored modifiers `hero_own`. `hero_view`: the modifiers the hit sees (its own
/// plus the auras over it).
#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub fn monster_attack(
    cfg: &BalanceConfig,
    tables: &ResonanceTables,
    attacker: u8,
    profile: &MonsterProfile,
    skill: Option<&SkillStats>,
    monster_mods: &TimedMods,
    hero: &HeroStats,
    hp: &mut u32,
    states: &mut States,
    hero_own: &mut TimedMods,
    hero_view: &TimedMods,
    input: &mut HitInput,
    result: &mut HitResult,
    src: &mut impl RollSource,
) -> AttackOutcome {
    input.attacker = monster_attacker(cfg, attacker, profile, skill, monster_mods);
    input.defender = player_defender(cfg, hero, states, hero_view);
    resolve_hit_into(input, &cfg.combat, tables, src, result);
    let (left, died) = take_damage(*hp, result.damage);
    *hp = left;
    *states = result.states;

    let mut rider = None;
    if let Some(skill) = skill {
        if let Some(effect) = effect_action(skill.blocks.effect) {
            if result.outcome == Outcome::Hit && *hp > 0 {
                let (min, max) = monster_effect_range(cfg, profile, skill.power_bp, effect);
                let roll = resolve_rider(
                    cfg.support.rider_chance_pct,
                    result.crit,
                    min,
                    max,
                    &cfg.combat,
                    src,
                );
                let applied = if roll.outcome == Outcome::Hit {
                    land_effect(
                        cfg,
                        effect,
                        roll.amount,
                        skill.blocks.aspects[0],
                        attacker,
                        0,
                        states,
                        hero_own,
                    )
                } else {
                    Applied::None
                };
                rider = Some(RiderOutcome { roll, applied });
            }
        }
    }
    AttackOutcome { died, rider }
}

/// A monster's debuff skill on a player: it rolls to land (d20 + the monster's accuracy against
/// the player's defense with `hero_view`), then lands on its stored modifiers `hero_own`.
#[allow(clippy::too_many_arguments)]
pub fn monster_debuff_player(
    cfg: &BalanceConfig,
    caster: u8,
    profile: &MonsterProfile,
    skill: &SkillStats,
    effect: EffectAction,
    hero: &HeroStats,
    hp: u32,
    states: &mut States,
    hero_own: &mut TimedMods,
    hero_view: &TimedMods,
    src: &mut impl RollSource,
) -> RiderOutcome {
    let defense = player_defender(cfg, hero, states, hero_view).defense;
    let input = monster_support_input(cfg, profile, skill, effect, Some(defense));
    let roll = resolve_support(&input, &cfg.combat, src);
    let applied = if roll.outcome == Outcome::Hit && hp > 0 {
        land_effect(
            cfg,
            effect,
            roll.amount,
            skill.blocks.aspects[0],
            caster,
            0,
            states,
            hero_own,
        )
    } else {
        Applied::None
    };
    RiderOutcome { roll, applied }
}

/// A monster's heal or buff on itself or an allied monster: no roll to land, a crit roll and the
/// d100 strength, like a player's (docs/BESTIARY.md 8.1: the Goblin Shaman). A dead target is
/// never raised; monsters have no Endurance, so a buff on itself lasts its plain duration.
#[allow(clippy::too_many_arguments)]
pub fn monster_support(
    cfg: &BalanceConfig,
    caster: u8,
    profile: &MonsterProfile,
    skill: &SkillStats,
    effect: EffectAction,
    target: &mut Monster,
    mods: &mut TimedMods,
    src: &mut impl RollSource,
) -> RiderOutcome {
    let input = monster_support_input(cfg, profile, skill, effect, None);
    let roll = resolve_support(&input, &cfg.combat, src);
    let applied = if !target.alive || target.hp == 0 {
        Applied::None
    } else if effect == EffectAction::Heal {
        target.hp = heal(target.hp, target.hp_max, roll.amount);
        Applied::None
    } else {
        land_effect(
            cfg,
            effect,
            roll.amount,
            skill.blocks.aspects[0],
            caster,
            0,
            &mut target.states,
            mods,
        )
    };
    RiderOutcome { roll, applied }
}

/// A monster's reactive aura (docs/SKILL_BUILDER.md 3.5): like [`start_aura`] with the
/// monster's numbers.
pub fn monster_start_aura(
    cfg: &BalanceConfig,
    profile: &MonsterProfile,
    skill: &MonsterSkillStats,
    src: &mut impl RollSource,
) -> (EffectRoll, Option<Aura>) {
    let Some(kind) = aura_kind(&skill.stats) else {
        return (EffectRoll::default(), None);
    };
    let input = monster_support_input(cfg, profile, &skill.stats, EffectAction::Mod(kind), None);
    let roll = resolve_support(&input, &cfg.combat, src);
    let aura = reactive_aura(cfg, &skill.stats, roll.amount).map(|a| Aura {
        kin_only: skill.kin_only,
        ..a
    });
    (roll, aura)
}
