use anchor_lang::prelude::*;
use deadroll_types::{Cell, GameSessionData, PARTY_MAX};

use crate::rules::{Unit, EMPTY_UNIT};

/// The attack waiting for its VRF value (`core.pending_vrf`), resolved by `consume_randomness`.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct PendingAction {
    /// Turn-order id of the attacker.
    pub attacker: u8,
    /// Turn-order id of the target.
    pub target: u8,
    /// Skill slot of a player attack, `rules::BASE_ATTACK` for the weapon; monsters: the action.
    pub skill_slot: u8,
    /// The player side of the hit (attacker or target): the callback must receive exactly it.
    pub participation: Pubkey,
}

/// `["session", map]`: one party on one map.
#[account]
#[derive(InitSpace)]
pub struct GameSession {
    /// Frozen contract, first after the discriminator.
    pub core: GameSessionData,
    /// The map's `MapLayout`, read on every move.
    pub layout: Pubkey,
    pub width: u8,
    pub height: u8,
    /// Players are placed here when they join.
    pub entrance: Cell,
    /// DM session key allowed to move monsters and end their turns (`Pubkey::default()` = none).
    pub dm_actor: Pubkey,
    /// Used entries in `core.turn_order`.
    pub turn_order_len: u8,
    /// Bumped on every turn start; action points are granted lazily per `turn_seq`.
    pub turn_seq: u32,
    /// Unix time the current turn started (turn timeout, P1).
    pub turn_started_at: i64,
    /// Mirror of each player's `Participation` position, for occupancy checks.
    pub player_pos: [Cell; PARTY_MAX],
    /// Bit per player slot: alive.
    pub alive_mask: u8,
    /// Bit per player slot: ready in the lobby.
    pub ready_mask: u8,
    /// Valid while `core.pending_vrf`.
    pub pending: PendingAction,
    /// Bit per player slot: chose "Escape" and nobody took another action since
    /// (docs/PARTY_AND_TURNS.md, section 7.3).
    pub flee_mask: u8,
    /// Bit per player slot: `claim_rewards` done after the party ended.
    pub settled_mask: u8,
    /// Experience from killed monsters; every living player gets all of it on victory.
    pub xp_pool: u32,
    /// VRF value of the boss kill; the reward rolls derive from it.
    pub victory_seed: [u8; 32],
}

impl GameSession {
    /// The unit whose turn it is, or `None` before start or on a corrupt order.
    pub fn current_unit(&self) -> Option<Unit> {
        if self.turn_order_len == 0 {
            return None;
        }
        let id = *self.core.turn_order.get(self.core.turn_index as usize)?;
        if id == EMPTY_UNIT {
            return None;
        }
        Unit::from_id(id)
    }

    pub fn player_alive(&self, slot: u8) -> bool {
        slot < self.core.player_count && self.alive_mask & (1 << slot) != 0
    }

    /// Bits for every joined slot.
    pub fn full_mask(&self) -> u8 {
        ((1u16 << self.core.player_count) - 1) as u8
    }
}
