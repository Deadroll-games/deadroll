//! Events (CONTRACTS.md, section 7). `PlayerReady`, `PlayerDied`, `FleeVoted`, `RewardsClaimed`
//! and `EscrowClaimed` are session-local.
use anchor_lang::prelude::*;
use deadroll_types::{ActiveState, SessionStatus, STATES_MAX};

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

/// One resolved attack. `breakdown` is the Borsh-encoded `deadroll_combat::HitResult` (every
/// roll and per-packet step); the website decodes it with the combat WASM and can replay it
/// from `seed`.
#[event]
pub struct HitResolved {
    pub session: Pubkey,
    pub action: u32,
    pub attacker: u8,
    pub target: u8,
    pub skill_slot: u8,
    /// The VRF value of this action.
    pub seed: [u8; 32],
    /// `deadroll_combat::Outcome`: 0 miss, 1 hit, 2 immune.
    pub outcome: u8,
    pub d20: u8,
    pub crit: bool,
    pub d100: u8,
    pub damage: u32,
    /// Resonance id per skill base, `0xFF` = none triggered.
    pub resonance: [u8; 3],
    /// Target states after the hit.
    pub states: [ActiveState; STATES_MAX],
    pub target_hp: u32,
    pub killed: bool,
    pub breakdown: Vec<u8>,
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
