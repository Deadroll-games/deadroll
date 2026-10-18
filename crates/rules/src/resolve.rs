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
        monster_attacker, monster_defender, player_attacker, player_defender, support_input,
        take_damage, EffectAction, HeroStats, SkillStats,
    },
    monster::MonsterProfile,
};

/// The effect of a damage skill after its hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RiderOutcome {
    pub roll: EffectRoll,
    pub applied: Applied,
}

/// What a player's attack on a monster did besides the `HitResult`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttackOutcome {
    /// The monster reached 0 HP (`alive` is cleared).
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

/// A monster's natural attack on a player; returns whether the player died.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub fn monster_attack(
    cfg: &BalanceConfig,
    tables: &ResonanceTables,
    attacker: u8,
    profile: &MonsterProfile,
    monster_mods: &TimedMods,
    hero: &HeroStats,
    hp: &mut u32,
    states: &mut States,
    hero_mods: &TimedMods,
    input: &mut HitInput,
    result: &mut HitResult,
    src: &mut impl RollSource,
) -> bool {
    input.attacker = monster_attacker(cfg, attacker, profile, monster_mods);
    input.defender = player_defender(cfg, hero, states, hero_mods);
    resolve_hit_into(input, &cfg.combat, tables, src, result);
    let (left, died) = take_damage(*hp, result.damage);
    *hp = left;
    *states = result.states;
    died
}
