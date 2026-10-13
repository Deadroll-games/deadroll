//! Events (CONTRACTS.md, section 7). `PlayerReady`, `InitiativeRolled`, `TurnTicked`,
//! `TurnSkipped`, `ModsTicked`, `EffectResolved`, `SkillUsed`, `PlayerDied`, `FleeVoted`, `RewardsClaimed` and `EscrowClaimed`
//! are session-local.
use anchor_lang::prelude::*;
use deadroll_character::state::SKILLS_MAX;
use deadroll_combat::MODS_MAX;
use deadroll_types::{ActiveState, SessionStatus, StateKind, STATES_MAX, TURN_ORDER_MAX};

use deadroll_skillcheck::Effect;

use crate::{
    hit_event::{Applied, EffectRollData, HitResultData},
    state::TimedModData,
};

#[event]
pub struct SessionOpened {
    pub session: Pubkey,
    pub map: Pubkey,
    pub dm: Pubkey,
    pub price_lamports: u64,
    pub monsters: u8,
}

#[event]
pub struct PlayerJoined {
    pub session: Pubkey,
    pub character: Pubkey,
    pub player: Pubkey,
    pub slot: u8,
    pub contribution: u64,
    pub collected: u64,
}

#[event]
pub struct PlayerReady {
    pub session: Pubkey,
    pub slot: u8,
    pub ready: bool,
}

#[event]
pub struct TurnStarted {
    pub session: Pubkey,
    /// Turn-order id: `0..4` player slot, `4 + i` monster `i`.
    pub unit: u8,
    pub round: u16,
    pub turn_seq: u32,
}

/// The start of a unit's turn (`start_turn`), one per turn: over-time damage, hard control,
/// durations and cooldowns counted down (docs/COMBAT_MATH.md, 7.5; SKILL_BUILDER.md, 8).
#[event]
pub struct TurnTicked {
    pub session: Pubkey,
    /// Turn-order id.
    pub unit: u8,
    pub turn_seq: u32,
    /// Burning + Bleeding damage dealt now.
    pub dot_damage: u32,
    /// HP after it.
    pub hp: u32,
    /// The over-time damage killed the unit (`MonsterKilled` / `PlayerDied` came first).
    pub died: bool,
    /// Freeze or Stun: the turn is skipped (`TurnSkipped`, then the next `TurnStarted`).
    pub skipped: bool,
    /// `deadroll_combat::states::state_bit` of every state that ended now.
    pub expired: u8,
    /// The unit's states after the tick.
    pub states: [ActiveState; STATES_MAX],
    /// Cooldowns after the tick, own turn starts until usable: a player's by `SkillSet` slot,
    /// a monster's in the first `MONSTER_SKILLS_MAX` entries.
    pub cooldowns: [u8; SKILLS_MAX],
}

/// Buffs and debuffs at the start of a unit's turn (`start_turn`, right after `TurnTicked`).
#[event]
pub struct ModsTicked {
    pub session: Pubkey,
    /// Turn-order id.
    pub unit: u8,
    pub turn_seq: u32,
    /// `deadroll_combat::support::mod_bit` of every buff or debuff that ended now.
    pub expired: u8,
    /// The unit's buffs and debuffs after the tick (`turns`: own turn starts left).
    pub mods: [TimedModData; MODS_MAX],
    /// Action points of this turn: speed with Haste and Slow.
    pub action_points: u8,
}

/// Hard control took the unit's turn: `start_turn` ended it on its own.
#[event]
pub struct TurnSkipped {
    pub session: Pubkey,
    pub unit: u8,
    pub turn_seq: u32,
    /// Freeze or Stun.
    pub state: StateKind,
}

#[event]
pub struct Moved {
    pub session: Pubkey,
    pub unit: u8,
    pub x: u8,
    pub y: u8,
    pub action_points: u8,
}

/// An attack waits for its VRF value.
#[event]
pub struct ActionRequested {
    pub session: Pubkey,
    /// `GameSession.action_counter` of this action; `HitResolved` carries the same number.
    pub action: u32,
    pub attacker: u8,
    pub target: u8,
    /// Player skill slot (`0xFF` = weapon), or the monster action.
    pub skill_slot: u8,
    pub action_points: u8,
}

/// A player used an active skill (`act`); it is now on cooldown.
#[event]
pub struct SkillUsed {
    pub session: Pubkey,
    /// Turn-order id of the player.
    pub unit: u8,
    pub skill_slot: u8,
    /// The skill's cooldown: it misses this many of the player's next own turns.
    pub cooldown: u8,
    /// `Participation.cooldowns` after the use: own turn starts until each slot is usable.
    pub cooldowns: [u8; SKILLS_MAX],
}

/// One resolved attack, in the shape the website reads (app/src/chain/types.ts). `result` is
/// the full `deadroll_combat::HitResult` (every roll and per-packet step, Borsh layout of
/// crates/combat/wasm/README.md); the website can replay it from `seed`. Written by
/// `hit_event::emit_hit_resolved`, never built on the stack.
#[event]
pub struct HitResolved {
    pub session: Pubkey,
    /// Turn-order ids: `0..4` player slot, `4 + i` monster `i`.
    pub attacker: u8,
    pub target: u8,
    /// The VRF value of this action.
    pub seed: [u8; 32],
    /// `GameSession.action_counter` of this action, the same number as its `ActionRequested`.
    pub action_counter: u32,
    pub result: HitResultData,
}

/// A resolved heal, buff or debuff (`act` with such a skill: instead of `HitResolved`), or the
/// effect of a damage skill after its hit (`rider`: right after that action's `HitResolved`).
/// Every roll is in `roll`; the website can replay it from `seed` (`deadroll_combat::support`).
#[event]
pub struct EffectResolved {
    pub session: Pubkey,
    /// Turn-order ids: the caster and the unit the effect went to.
    pub caster: u8,
    pub target: u8,
    /// The VRF value of this action.
    pub seed: [u8; 32],
    /// `GameSession.action_counter` of this action, the same number as its `ActionRequested`.
    pub action_counter: u32,
    pub skill_slot: u8,
    /// The skill's effect.
    pub effect: Effect,
    /// The effect of a damage skill, rolled after the hit on the next dice of the same seed.
    pub rider: bool,
    pub roll: EffectRollData,
    /// What it did: `Added` / `Refreshed` (a buff, debuff or Stun), `Froze`, or `None` (a miss,
    /// a heal, or no free slot).
    pub applied: Applied,
    /// The target's HP after (a heal raises it, up to its maximum).
    pub hp: u32,
    /// The target's states and buffs / debuffs after.
    pub states: [ActiveState; STATES_MAX],
    pub mods: [TimedModData; MODS_MAX],
}

/// The turn order, rolled once from the VRF value of `start_session`: per unit `d20 +
/// initiative`, higher first (ties: higher initiative, then lower unit id). The first `len`
/// entries are used. `TurnStarted` for the first unit follows.
#[event]
pub struct InitiativeRolled {
    pub session: Pubkey,
    pub seed: [u8; 32],
    pub action_counter: u32,
    pub len: u8,
    /// Turn-order ids, in turn order.
    pub order: [u8; TURN_ORDER_MAX],
    /// The d20 of `order[i]`.
    pub d20: [u8; TURN_ORDER_MAX],
    /// The initiative bonus of `order[i]`.
    pub initiative: [u16; TURN_ORDER_MAX],
}

#[event]
pub struct MonsterKilled {
    pub session: Pubkey,
    pub monster: u8,
    pub bestiary_id: u8,
    pub by: u8,
    pub xp: u32,
    pub boss: bool,
}

/// A player's character reached 0 HP; `character.mark_dead` ran in the same callback.
#[event]
pub struct PlayerDied {
    pub session: Pubkey,
    pub character: Pubkey,
    pub slot: u8,
    pub by: u8,
}

#[event]
pub struct SessionEnded {
    pub session: Pubkey,
    pub status: SessionStatus,
    pub round: u16,
}

#[event]
pub struct FleeVoted {
    pub session: Pubkey,
    pub slot: u8,
    pub flee_mask: u8,
    pub alive_mask: u8,
}

#[event]
pub struct RewardsClaimed {
    pub session: Pubkey,
    pub character: Pubkey,
    pub slot: u8,
    pub xp: u32,
    pub resources: [u32; 8],
}

#[event]
pub struct EscrowClaimed {
    pub session: Pubkey,
    pub dm: Pubkey,
    pub amount: u64,
}
