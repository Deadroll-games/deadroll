use anchor_lang::prelude::*;
use deadroll_types::{Cell, GameSessionData, Tile, MONSTERS_MAX, PARTY_MAX};

use crate::rules::{Unit, EMPTY_UNIT};

/// The roll waiting for its VRF value (`core.pending_vrf`), resolved by `consume_randomness`:
/// an attack, or the initiative roll of `start_session` ([`PendingAction::initiative`]).
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
    /// For the initiative roll: player slot 0 (the callback's account list is fixed).
    pub participation: Pubkey,
}

impl PendingAction {
    /// The initiative roll: no attacker, no target.
    pub fn initiative(participation: Pubkey) -> Self {
        Self {
            attacker: EMPTY_UNIT,
            target: EMPTY_UNIT,
            skill_slot: 0,
            participation,
        }
    }

    pub fn is_initiative(&self) -> bool {
        self.attacker == EMPTY_UNIT
    }
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
    // Registry rules, copied when the party opens so they stay fixed for it.
    /// Bestiary speed (action points per turn) of each monster, by monster index.
    pub monster_speed: [u8; MONSTERS_MAX],
    /// Bestiary initiative of each monster, by monster index.
    pub monster_initiative: [u16; MONSTERS_MAX],
    /// Initiative of each player slot, set at `join_session`.
    pub player_initiative: [u16; PARTY_MAX],
    /// Step cost in action points per `Tile`, 0 = cannot be entered (registry `Tileset`).
    pub tile_cost: [u8; Tile::COUNT],
    /// `turn_seq` in which the current unit used its one attack or active skill
    /// (docs/PARTY_AND_TURNS.md, 5.1). Only the current unit acts, so one field serves all.
    pub acted_seq: u32,
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

    /// The current unit already used its attack or active skill this turn.
    pub fn acted(&self) -> bool {
        self.acted_seq == self.turn_seq
    }

    /// Bits for every joined slot.
    pub fn full_mask(&self) -> u8 {
        ((1u16 << self.core.player_count) - 1) as u8
    }
}

#[cfg(test)]
mod tests {
    use crate::state::test_support::session;
    use deadroll_types::Cell;

    #[test]
    fn one_action_per_turn() {
        let mut s = session(&[Cell { x: 0, y: 0 }]);
        s.turn_seq = 3;
        assert!(!s.acted());
        s.acted_seq = s.turn_seq;
        assert!(s.acted());
        // The next turn (any unit) may act again.
        s.turn_seq += 1;
        assert!(!s.acted());
    }
}
