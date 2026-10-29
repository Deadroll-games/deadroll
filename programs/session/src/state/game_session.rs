use anchor_lang::prelude::*;
use deadroll_rules::aura::{
    Aura, AuraField, MonsterAuraField, PartyAuras, PlayerAuras, AURAS_PER_PLAYER,
};
use deadroll_types::{
    Aspect, Cell, GameSessionData, MapPoint, PackLayout, ARENA_MONSTERS_MAX, CHESTS_MAX,
    MONSTERS_MAX, PACKS_MAX, PARTY_MAX, TILE_KINDS_MAX,
};

use super::{
    auras_to_rules, AuraData, PendingMonsterReactiveData, PendingReactiveData, ReactiveData,
};
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
    /// Bit per player slot: settled after the party ended (`settle_party` sets every bit,
    /// `claim_after_timeout` its own).
    pub settled_mask: u8,
    /// VRF value of the boss kill.
    pub victory_seed: [u8; 32],
    // Registry rules, copied when the party opens so they stay fixed for it.
    /// Bestiary speed (action points per turn) of each monster, by monster index.
    pub monster_speed: [u8; MONSTERS_MAX],
    /// Bestiary initiative of each monster, by monster index.
    pub monster_initiative: [u16; MONSTERS_MAX],
    /// Initiative of each player slot, set at `join_session`.
    pub player_initiative: [u16; PARTY_MAX],
    /// `turn_seq` in which the current unit used its one attack or active skill
    /// (docs/PARTY_AND_TURNS.md, 5.1). Only the current unit acts, so one field serves all.
    pub acted_seq: u32,
    /// `turn_seq` whose start ran (`start_turn`): states ticked, cooldowns counted down. Every
    /// other turn instruction waits for it.
    pub started_seq: u32,
    /// The ally `Participation` a pending heal or buff targets when it is not the caster: the
    /// callback's `target_participation`. `Pubkey::default()` = none.
    pub pending_target: Pubkey,
    // Patch 5: skills beyond the one action (docs/SKILL_BUILDER.md, 3.2-3.5).
    /// Each player slot's `Participation`, set at `join_session`: the party accounts every VRF
    /// callback gets after its fixed list.
    pub participations: [Pubkey; PARTY_MAX],
    /// Each player's reactive skill on the map, set at `set_ready(true)`.
    pub reactives: [ReactiveData; PARTY_MAX],
    /// Each player's auras: its passive skill's (permanent, set at `set_ready(true)`) and its
    /// reactive skill's (temporary).
    pub auras: [[AuraData; AURAS_PER_PLAYER]; PARTY_MAX],
    /// A pending roll that only resolves reactives (`monster_move`, `start_turn`); `kind == NONE`
    /// for every other roll.
    pub pending_reactive: PendingReactiveData,
    // Patch 6: monster skills (docs/BESTIARY.md 3).
    /// A pending roll that only resolves monster reactives (`move_unit`, `start_turn`);
    /// `kind == NONE` for every other roll.
    pub pending_monster_reactive: PendingMonsterReactiveData,
    /// Players down in a VRF callback whose death is not finalized yet (`character.mark_dead`),
    /// one bit per slot: the queue `start_turn` and the claims finalize first, in slot order.
    /// While it is not empty no turn starts and no action is taken.
    pub pending_deaths: u8,
    // Patch 9: resistances.
    /// Each player's auras' aspect weights (a Resistance, `deadroll_rules::aura::Aura::weights_pct`),
    /// by aura slot like `auras`.
    pub aura_weights: [[[u8; Aspect::COUNT]; AURAS_PER_PLAYER]; PARTY_MAX],
    // Patch 11: rewards (docs/PARTY_AND_TURNS.md 8-9, MAP_DESCRIPTION.md 7.1), copied from the map
    // at `open_session`.
    /// The map's tier and minimum players (`start_session` needs that many).
    pub tier: u8,
    pub min_players: u8,
    /// The visible chests: cell and reward table id; `0..chest_count` used.
    pub chests: [MapPoint; CHESTS_MAX],
    pub chest_count: u8,
    /// The reward table of the boss event (registry id).
    pub boss_table: u8,
    /// Bit per chest: opened (then it no longer occupies its cell).
    pub chests_opened: u8,
    /// The chest whose roll is pending (`open_chest`), `NO_CHEST` for every other roll.
    pub pending_chest: u8,
    /// Each player's share of the rewards (`deadroll_rules::loot::reward_share`), set at
    /// `set_ready(true)`: experience in basis points, and a bit per slot whose rewards are rolled.
    pub xp_bp: [u16; PARTY_MAX],
    pub loot_mask: u8,
    /// Bit per monster killed where no party accounts were at hand (over-time damage in
    /// `start_turn`): its experience and body rewards go out with the next callback, or at the
    /// settlement.
    pub unpaid_kills: u32,
    /// Unix time the party ended (Victory, Wiped, Fled): the DM settles before
    /// `ended_at + settle_timeout_secs`, the players claim after.
    pub ended_at: i64,
    /// The map's DM-set numbers: no living monster within `chest_radius` of a chest or of any
    /// living player when it opens; the DM's time to settle after the end.
    pub chest_radius: u8,
    pub settle_timeout_secs: u32,
    // Patch 13: tiles (docs/MAP_DESCRIPTION.md 4.1, 4.2). Replaces the patch 1 `[u8; 7]` by `Tile`.
    /// Step cost in action points per tile type id (`TileKind`, a cell byte's bits 7-3), 0 = cannot
    /// be entered (registry `Tileset`, copied at `open_session`).
    pub tile_cost: [u8; TILE_KINDS_MAX],
    // Arena update (docs/PARTY_AND_TURNS.md 4.1, MAP_DESCRIPTION.md 17): the party walks the map
    // in a circle and fights one pack at a time in the pack's arena.
    /// The map or an arena, and the two steps between them.
    pub mode: ArenaMode,
    /// The pack the party is engaged with or fighting, `NO_PACK` on the map. Its monsters are the
    /// live slots of `MonsterState`.
    pub arena_pack: u8,
    /// The player who stepped next to the pack: their side is where the party enters.
    pub engaged_by: u8,
    /// The map's packs, copied at `open_session`: the visible monster's map cell, the facing,
    /// the arena size.
    pub packs: [PackLayout; PACKS_MAX],
    /// Bit per pack number still on the map.
    pub packs_alive: u8,
    /// Kept while the party is in an arena (`width`, `height`, `player_pos`, the turn order
    /// and the round are the arena's then): the map's size, each player's map cell before the
    /// fight, the map circle, its index and round.
    pub map_width: u8,
    pub map_height: u8,
    pub map_pos: [Cell; PARTY_MAX],
    pub map_order: [u8; PARTY_MAX],
    pub map_order_len: u8,
    pub map_turn_index: u8,
    pub map_round: u16,
    /// The arena's initiative timeline (docs/PARTY_AND_TURNS.md 6): each unit's time until its
    /// turn, by turn-order id (players `0..PARTY_MAX`, then the arena's monsters).
    pub unit_time: [u32; PARTY_MAX + ARENA_MONSTERS_MAX],
    /// Each player's Dexterity, set at `set_ready(true)`: it wins a tie on the timeline.
    pub player_dex: [u16; PARTY_MAX],
}

/// Arena update: where the party is.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub enum ArenaMode {
    /// On the map: the players move in a circle, the monsters stand still.
    #[default]
    Map,
    /// A player stepped next to a pack's visible monster: `enter_arena` comes next.
    Engaged,
    /// Fighting `arena_pack` in its arena, on the timeline.
    Arena,
    /// The arena's pack is dead: `leave_arena` pays it and brings the party back.
    Won,
}

/// `GameSession.arena_pack` on the map.
pub const NO_PACK: u8 = 0xFF;

/// `GameSession.pending_chest` when no chest roll is pending.
pub const NO_CHEST: u8 = 0xFF;

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

    /// `start_turn` ran for the current turn.
    pub fn turn_started(&self) -> bool {
        self.started_seq == self.turn_seq
    }

    /// The current unit already used its attack or active skill this turn.
    pub fn acted(&self) -> bool {
        self.acted_seq == self.turn_seq
    }

    /// A closed chest stands on `c` (patch 11: it occupies its cell until opened). Arena update:
    /// chests are on the map only.
    pub fn closed_chest_at(&self, c: Cell) -> bool {
        self.mode == ArenaMode::Map && self.closed_chests().any(|p| p == c)
    }

    /// The cells of the closed chests.
    pub fn closed_chests(&self) -> impl Iterator<Item = Cell> + '_ {
        self.chest_list()
            .iter()
            .enumerate()
            .filter(|(i, _)| self.chests_opened & (1 << i) == 0)
            .map(|(_, p)| p.cell())
    }

    /// Arena update: the turns go on (on the map or in an arena), not between the two.
    pub fn in_play(&self) -> bool {
        matches!(self.mode, ArenaMode::Map | ArenaMode::Arena)
    }

    pub fn require_in_play(&self) -> Result<()> {
        require!(self.in_play(), crate::errors::SessionError::ArenaTransition);
        Ok(())
    }

    /// The arena whose cells the units stand on, `None` on the map.
    pub fn ground(&self) -> Option<u8> {
        match self.mode {
            ArenaMode::Arena | ArenaMode::Won => Some(self.arena_pack),
            ArenaMode::Map | ArenaMode::Engaged => None,
        }
    }

    /// The packs still on the map, with their record.
    pub fn living_packs(&self) -> impl Iterator<Item = (u8, &PackLayout)> + '_ {
        self.packs
            .iter()
            .enumerate()
            .filter(move |(n, p)| p.is_used() && self.packs_alive & (1 << n) != 0)
            .map(|(n, p)| (n as u8, p))
    }

    /// On the map: the cells of the visible monsters of the living packs (they occupy them).
    /// Nothing in an arena.
    pub fn visible_monsters(&self) -> impl Iterator<Item = Cell> + '_ {
        let on_map = self.ground().is_none();
        self.living_packs()
            .filter(move |_| on_map)
            .map(|(_, p)| p.cell())
    }

    /// The living pack whose visible monster stands next to `c` (Chebyshev distance 1), lowest
    /// number first: stepping onto `c` engages it (docs/PARTY_AND_TURNS.md 4.1.3).
    pub fn pack_next_to(&self, c: Cell) -> Option<u8> {
        self.living_packs()
            .find(|(_, p)| deadroll_mapcheck::walk::adjacent(p.cell(), c))
            .map(|(n, _)| n)
    }

    /// The used chests.
    pub fn chest_list(&self) -> &[MapPoint] {
        &self.chests[..(self.chest_count as usize).min(CHESTS_MAX)]
    }

    /// Living players whose rewards are rolled, bit per slot.
    pub fn loot_eligible(&self) -> u8 {
        self.alive_mask & self.loot_mask & self.full_mask()
    }

    /// Bits for every joined slot.
    pub fn full_mask(&self) -> u8 {
        ((1u16 << self.core.player_count) - 1) as u8
    }

    /// The auras as the rules see them; borrow them with [`Self::aura_field`].
    pub fn party_auras(&self) -> PartyAuras {
        let mut out = auras_to_rules(&self.auras);
        for (p, w) in out.iter_mut().zip(&self.aura_weights) {
            for (a, w) in p.iter_mut().zip(w) {
                a.weights_pct = *w;
            }
        }
        out
    }

    /// Player `slot`'s auras with their weights.
    pub fn player_auras(&self, slot: u8) -> PlayerAuras {
        self.party_auras()[slot as usize]
    }

    /// Writes player `slot`'s auras and their weights.
    pub fn set_player_auras(&mut self, slot: u8, auras: &PlayerAuras) {
        for (i, a) in auras.iter().enumerate() {
            self.set_aura(slot, i, *a);
        }
    }

    /// Writes aura `index` of player `slot` and its weights.
    pub fn set_aura(&mut self, slot: u8, index: usize, a: Aura) {
        self.auras[slot as usize][index] = a.into();
        self.aura_weights[slot as usize][index] = a.weights_pct;
    }

    /// The party's auras over the living players' cells, and the monsters' (`Monsters::bearers`).
    pub fn aura_field<'a>(
        &'a self,
        auras: &'a PartyAuras,
        monsters: MonsterAuraField<'a>,
    ) -> AuraField<'a> {
        AuraField {
            auras,
            cells: &self.player_pos,
            alive: self.alive_mask,
            monsters,
        }
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

    #[test]
    fn aura_weights_travel_with_their_aura() {
        use deadroll_combat::ModKind;
        use deadroll_rules::aura::{Aura, PASSIVE_AURA, REACTIVE_AURA};
        let mut s = session(&[Cell { x: 0, y: 0 }, Cell { x: 1, y: 0 }]);
        let ward = Aura {
            kind: ModKind::Resistance,
            power_bp: 2_000,
            turns: 3,
            weights_pct: [0, 100, 0, 0, 0, 65],
            ..Aura::default()
        };
        s.set_aura(1, REACTIVE_AURA, ward);
        assert_eq!(s.player_auras(1)[REACTIVE_AURA], ward);
        assert_eq!(
            s.party_auras()[1][REACTIVE_AURA].weights_pct,
            ward.weights_pct
        );
        assert!(s.player_auras(1)[PASSIVE_AURA].is_empty());
        assert_eq!(s.player_auras(0)[REACTIVE_AURA].weights_pct, [0; 6]);
        s.set_player_auras(1, &Default::default());
        assert_eq!(s.aura_weights[1][REACTIVE_AURA], [0; 6]);
    }

    #[test]
    fn every_turn_needs_its_start() {
        let mut s = session(&[Cell { x: 0, y: 0 }]);
        s.turn_seq = 1;
        assert!(!s.turn_started());
        s.started_seq = 1;
        assert!(s.turn_started());
        s.turn_seq = 2;
        assert!(!s.turn_started());
    }

    fn with_packs() -> super::GameSession {
        use deadroll_types::{Facing, PackLayout};
        let mut s = session(&[Cell { x: 0, y: 0 }]);
        let pack = |x, y| PackLayout {
            visible: 0,
            x,
            y,
            facing: Facing::S,
            width: 8,
            height: 8,
        };
        s.packs[1] = pack(4, 4);
        s.packs[3] = pack(5, 5);
        s.packs_alive = 0b1010;
        s.mode = super::ArenaMode::Map;
        s
    }

    #[test]
    fn packs_stand_on_the_map_only_and_the_lowest_engages_first() {
        let mut s = with_packs();
        assert_eq!(s.ground(), None);
        assert_eq!(
            s.visible_monsters().collect::<Vec<_>>(),
            [Cell { x: 4, y: 4 }, Cell { x: 5, y: 5 }]
        );
        // (4, 5) is next to both: pack 1 first. (6, 6) only to pack 3. (4, 4) is pack 1's cell.
        assert_eq!(s.pack_next_to(Cell { x: 4, y: 5 }), Some(1));
        assert_eq!(s.pack_next_to(Cell { x: 6, y: 6 }), Some(3));
        assert_eq!(s.pack_next_to(Cell { x: 0, y: 0 }), None);
        // A dead pack is gone.
        s.packs_alive = 0b1000;
        assert_eq!(s.pack_next_to(Cell { x: 4, y: 5 }), Some(3));
        assert_eq!(s.pack_next_to(Cell { x: 3, y: 3 }), None);
        // In an arena the map's monsters are not there.
        s.mode = super::ArenaMode::Arena;
        s.arena_pack = 3;
        assert_eq!(s.ground(), Some(3));
        assert_eq!(s.visible_monsters().count(), 0);
    }

    #[test]
    fn turns_wait_between_the_map_and_an_arena() {
        use super::ArenaMode::*;
        let mut s = with_packs();
        for (mode, play) in [(Map, true), (Engaged, false), (Arena, true), (Won, false)] {
            s.mode = mode;
            assert_eq!(s.in_play(), play, "{mode:?}");
            assert_eq!(s.require_in_play().is_ok(), play);
        }
        s.mode = Won;
        s.arena_pack = 1;
        assert_eq!(s.ground(), Some(1), "a won arena is still the ground");
    }

    #[test]
    fn chests_stand_on_the_map_only() {
        let mut s = with_packs();
        s.chests[0] = deadroll_types::MapPoint {
            kind: deadroll_types::PointKind::Chest,
            x: 2,
            y: 2,
            data: 0,
        };
        s.chest_count = 1;
        assert!(s.closed_chest_at(Cell { x: 2, y: 2 }));
        s.mode = super::ArenaMode::Arena;
        assert!(!s.closed_chest_at(Cell { x: 2, y: 2 }));
        s.mode = super::ArenaMode::Map;
        s.chests_opened = 1;
        assert!(!s.closed_chest_at(Cell { x: 2, y: 2 }));
        assert_eq!(s.closed_chests().count(), 0);
    }
}
