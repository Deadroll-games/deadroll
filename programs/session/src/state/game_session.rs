use anchor_lang::prelude::*;
use deadroll_mapcheck::secrets::{Board, Leaves};
use deadroll_rules::{
    aura::{Aura, AuraField, MonsterAuraField, PartyAuras, PlayerAuras, AURAS_PER_PLAYER},
    exit::Vote,
};
use deadroll_types::{
    Aspect, Cell, FoundSecret, GameSessionData, MapPoint, PackLayout, SecretKind, Spawn,
    ARENA_MONSTERS_MAX, CHESTS_MAX, MONSTERS_MAX, NO_SPAWN, OBLIGATIONS_MAX, PACKS_MAX, PARTY_MAX,
    SECRETS_MAX, SECRET_LEAVES_MAX, SECRET_TYPES, SPAWNS_MAX, TILE_KINDS_MAX,
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
    pub unit_time: [u32; ARENA_UNITS],
    /// Each player's Dexterity, set at `set_ready(true)`: it wins a tie on the timeline.
    pub player_dex: [u16; PARTY_MAX],
    // Patch B: Retreat and Escape from an arena (docs/PARTY_AND_TURNS.md 7.5). `flee_mask` holds
    // the votes for `exit_kind`.
    /// The proposal on the table (`deadroll_rules::exit::ExitKind` as `u8`, `NO_EXIT` = none).
    pub exit_kind: u8,
    /// The vote passed: bit per arena slot whose last turn is still to come. While it is not
    /// empty only those monsters move; when it empties the party leaves.
    pub last_moves: u8,
    /// Retreats this party made (`ArenaTuning::retreats_max`).
    pub retreats: u8,
    /// Bit per pack number the party retreated from: on guard, it moves first from any side.
    pub packs_guarded: u8,
    /// Bit per pack number whose next fight starts with the retreat buff (cleared when it starts).
    pub packs_buffed: u8,
    /// Bit per layout spawn killed in an arena the party retreated from: it stays dead.
    /// Patch C: per revealed spawn (`spawns`).
    pub dead_spawns: u32,
    // Patch C: secrets under the Merkle root (docs/MAP_DESCRIPTION.md 8, 11, 17.5).
    /// The map's `secrets_root`, its leaf count (`deadroll_mapcheck::secrets::Leaves`) and the
    /// public part: each pack's threat, the declared counts (copied at `open_session`).
    pub secrets_root: [u8; 32],
    pub leaf_count: u16,
    pub declared: [u8; SECRET_TYPES],
    pub threat: [u8; PACKS_MAX],
    /// Bit per leaf the DM revealed.
    pub revealed: [u8; REVEALED_BYTES],
    /// Secrets of each type revealed (`SecretKind::counted`), never above `declared`.
    pub found_counts: [u8; SECRET_TYPES],
    /// The secrets revealed, in order (`found_count` used): what the party sees.
    pub found: [FoundSecret; SECRETS_MAX],
    pub found_count: u8,
    /// Bit per pack number whose composition the DM revealed.
    pub packs_revealed: u8,
    /// The DM's obligations (MAP 8.5): leaves to reveal before any other action
    /// (`obligation_count` used), and the player whose step or look made them.
    pub obligations: [u16; OBLIGATIONS_MAX],
    pub obligation_count: u8,
    pub obliged_by: u8,
    /// The revealed packs' monsters, pack by pack in reveal order (`spawn_count` used): the
    /// spawns `enter_arena` loads and `dead_spawns` counts.
    pub spawns: [Spawn; SPAWNS_MAX],
    pub spawn_count: u8,
    /// The map's totals over the revealed packs (`deadroll_mapcheck::roster::RosterTotals`):
    /// monsters, elites, bosses, packs.
    pub roster_totals: [u8; 4],
    /// The chests of the map; the hidden stashes found follow them in `chests`.
    pub visible_chests: u8,
}

/// Patch C: bytes of `GameSession.revealed`, a bit per leaf.
pub const REVEALED_BYTES: usize = SECRET_LEAVES_MAX.div_ceil(8);

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
    /// Patch B: the party retreated and the monsters took their last turns: `leave_arena` brings
    /// the party back, the pack stays on the map (docs/PARTY_AND_TURNS.md 7.5).
    Retreated,
}

/// Turn-order ids of an arena: every player slot, then every arena slot. A named length: `InitSpace`
/// multiplies an array length expression without parentheses.
pub const ARENA_UNITS: usize = PARTY_MAX + ARENA_MONSTERS_MAX;

/// Patch B: the bit of layout spawn `spawn` in `GameSession.dead_spawns`.
pub const fn spawn_bit(spawn: u8) -> u32 {
    match 1u32.checked_shl(spawn as u32) {
        Some(b) => b,
        None => 0,
    }
}

const _: () = assert!(deadroll_types::SPAWNS_MAX <= 32);

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
            ArenaMode::Arena | ArenaMode::Won | ArenaMode::Retreated => Some(self.arena_pack),
            ArenaMode::Map | ArenaMode::Engaged => None,
        }
    }

    /// Patch B: the Retreat or Escape proposal on the table.
    pub fn exit_vote(&self) -> Vote {
        Vote {
            kind: self.exit_kind,
            mask: self.flee_mask,
        }
    }

    pub fn set_exit_vote(&mut self, v: Vote) {
        self.exit_kind = v.kind;
        self.flee_mask = v.mask;
    }

    /// The vote passed and the arena's monsters are taking their last turns: no player acts.
    pub fn leaving(&self) -> bool {
        self.last_moves != 0
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
    /// Nothing in an arena. Patch C: an ambush pack shows none.
    pub fn visible_monsters(&self) -> impl Iterator<Item = Cell> + '_ {
        let on_map = self.ground().is_none();
        self.living_packs()
            .filter(move |(_, p)| on_map && p.visible != NO_SPAWN)
            .map(|(_, p)| p.cell())
    }

    /// The living pack whose visible monster stands next to `c` (Chebyshev distance 1), lowest
    /// number first: stepping onto `c` engages it (docs/PARTY_AND_TURNS.md 4.1.3). Patch C: an
    /// ambush pack only by its trap.
    pub fn pack_next_to(&self, c: Cell) -> Option<u8> {
        self.living_packs()
            .find(|(_, p)| p.visible != NO_SPAWN && deadroll_mapcheck::walk::adjacent(p.cell(), c))
            .map(|(n, _)| n)
    }

    /// Patch C: the map's size, also while the party is in an arena.
    pub fn map_dims(&self) -> (u8, u8) {
        match self.ground() {
            None => (self.width, self.height),
            Some(_) => (self.map_width, self.map_height),
        }
    }

    /// Patch C: the map's leaf numbering.
    pub fn leaves(&self) -> Leaves {
        let (w, h) = self.map_dims();
        Leaves::new(w, h, &self.packs)
    }

    /// Patch C: the leaf of cell `c` of the ground the units stand on.
    pub fn leaf_here(&self, c: Cell) -> Option<u16> {
        let board = match self.ground() {
            None => Board::Map(c),
            Some(pack) => Board::Arena(pack, c),
        };
        self.leaves().of(board)
    }

    pub fn is_revealed(&self, leaf: u16) -> bool {
        self.revealed
            .get(leaf as usize / 8)
            .is_some_and(|b| b & (1 << (leaf % 8)) != 0)
    }

    pub fn set_revealed(&mut self, leaf: u16) {
        if let Some(b) = self.revealed.get_mut(leaf as usize / 8) {
            *b |= 1 << (leaf % 8);
        }
    }

    /// Patch C: the obligations in use.
    pub fn obligation_list(&self) -> &[u16] {
        &self.obligations[..(self.obligation_count as usize).min(OBLIGATIONS_MAX)]
    }

    /// Patch C: the DM owes no reveal (docs/MAP_DESCRIPTION.md 8.5: until the obligations are
    /// fulfilled, the party's next turn does not go through).
    pub fn require_no_reveals(&self) -> Result<()> {
        require!(
            self.obligation_count == 0,
            crate::errors::SessionError::RevealsPending
        );
        Ok(())
    }

    /// Patch C: the DM must reveal `leaf` (once; a revealed leaf needs nothing). `false` when
    /// the list is full.
    pub fn oblige(&mut self, leaf: u16) -> bool {
        if self.is_revealed(leaf) || self.obligation_list().contains(&leaf) {
            return true;
        }
        let n = self.obligation_count as usize;
        if n >= OBLIGATIONS_MAX {
            return false;
        }
        self.obligations[n] = leaf;
        self.obligation_count += 1;
        true
    }

    /// Patch C: takes `leaf` off the obligations; `false` if it was not on them.
    pub fn fulfil(&mut self, leaf: u16) -> bool {
        let list = self.obligation_list();
        let Some(i) = list.iter().position(|&l| l == leaf) else {
            return false;
        };
        let n = list.len();
        self.obligations.copy_within(i + 1..n, i);
        self.obligations[n - 1] = 0;
        self.obligation_count -= 1;
        true
    }

    /// Patch C: the secrets found so far.
    pub fn found_list(&self) -> &[FoundSecret] {
        &self.found[..(self.found_count as usize).min(SECRETS_MAX)]
    }

    /// Patch C: a secret door found on cell `c` of the ground the units stand on: a door now.
    pub fn door_found_at(&self, c: Cell) -> bool {
        self.found_count != 0
            && self.leaf_here(c).is_some_and(|leaf| {
                self.found_list()
                    .iter()
                    .any(|f| f.leaf == leaf && f.secret.kind == SecretKind::Door)
            })
    }

    /// Patch C: the revealed spawns in use.
    pub fn spawn_list(&self) -> &[Spawn] {
        &self.spawns[..(self.spawn_count as usize).min(SPAWNS_MAX)]
    }

    /// Patch C: the used packs, bit per pack number.
    pub fn used_packs(&self) -> u8 {
        self.packs
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_used())
            .fold(0, |m, (n, _)| m | 1 << n)
    }

    /// Patch C: the DM's final audit is done (docs/PARTY_AND_TURNS.md 3.2): every declared secret
    /// and every pack revealed, the map's one boss among them.
    pub fn audit_done(&self) -> bool {
        self.found_counts == self.declared
            && self.packs_revealed == self.used_packs()
            && self.roster_totals[2] == deadroll_mapcheck::limits::BOSSES
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
    fn the_account_size_is_its_borsh_size() {
        use anchor_lang::Space;
        let s = session(&[Cell { x: 0, y: 0 }]);
        assert_eq!(
            anchor_lang::prelude::borsh::to_vec(&s).unwrap().len(),
            super::GameSession::INIT_SPACE
        );
    }

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

    /// Patch C: an ambush pack shows no monster and engages only by its trap.
    #[test]
    fn an_ambush_pack_is_not_on_the_map() {
        let mut s = with_packs();
        s.packs[3].visible = deadroll_types::NO_SPAWN;
        assert_eq!(
            s.visible_monsters().collect::<Vec<_>>(),
            [Cell { x: 4, y: 4 }]
        );
        assert_eq!(s.pack_next_to(Cell { x: 6, y: 6 }), None);
        assert_eq!(s.used_packs(), 0b1010);
    }

    /// Patch C: the obligations, the revealed leaves, a found door, the audit.
    #[test]
    fn obligations_block_until_revealed() {
        use deadroll_types::{CellSecret, FoundSecret, OBLIGATIONS_MAX};
        let mut s = with_packs();
        assert!(s.require_no_reveals().is_ok());
        let (w, h) = s.map_dims();
        assert_eq!((w, h), (s.width, s.height));
        let leaf = s.leaf_here(Cell { x: 1, y: 1 }).unwrap();
        assert_eq!(leaf as usize, deadroll_types::PACK_LEAVES + w as usize + 1);
        assert!(s.oblige(leaf));
        assert!(s.oblige(leaf), "once");
        assert_eq!(s.obligation_list(), &[leaf]);
        assert!(s.require_no_reveals().is_err());
        assert!(s.fulfil(leaf));
        assert!(!s.fulfil(leaf));
        assert!(s.require_no_reveals().is_ok());
        // A revealed leaf needs nothing; the list holds `OBLIGATIONS_MAX`.
        s.set_revealed(leaf);
        assert!(s.is_revealed(leaf) && !s.is_revealed(leaf + 1));
        assert!(s.oblige(leaf));
        assert_eq!(s.obligation_count, 0);
        for l in 0..OBLIGATIONS_MAX as u16 {
            assert!(s.oblige(100 + l));
        }
        assert!(!s.oblige(500));
        // The middle one goes, the order stays.
        assert!(s.fulfil(101));
        assert_eq!(s.obligation_list()[..2], [100, 102]);

        // A secret door found on the ground the party stands on.
        let door = Cell { x: 2, y: 3 };
        assert!(!s.door_found_at(door));
        s.found[0] = FoundSecret {
            leaf: s.leaf_here(door).unwrap(),
            secret: CellSecret::door(),
        };
        s.found_count = 1;
        assert!(s.door_found_at(door));
        assert!(!s.door_found_at(Cell { x: 3, y: 3 }));

        // The audit: every declared secret, every pack, one boss.
        s.declared = [0, 0, 0, 1];
        s.found_counts = [0, 0, 0, 1];
        s.packs_revealed = 0b0010;
        assert!(!s.audit_done());
        s.packs_revealed = 0b1010;
        assert!(!s.audit_done(), "no boss among them");
        s.roster_totals[2] = 1;
        assert!(s.audit_done());
        s.declared[0] = 1;
        assert!(!s.audit_done());
    }
}
