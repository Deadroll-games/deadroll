//! Events (CONTRACTS.md, section 7). `PlayerReady`, `InitiativeRolled`, `TurnTicked`,
//! `TurnSkipped`, `ModsTicked`, `EffectResolved`, `SkillUsed`, `PlayerDied`, `FleeVoted`, `RewardsRolled`, `ChestOpenRequested`, `ChestOpened`, `CacheSettled`, `ContributionRefunded`, `CacheClosed`, `EscrowClaimed` and `SessionKeyChanged`
//! are session-local.
use anchor_lang::prelude::*;
use deadroll_character::state::{MAP_SKILLS, SKILLS_MAX};
use deadroll_rules::spread::SPREAD_MODS_MAX;

use deadroll_types::{
    ActiveState, CacheSlot, Cell, RewardSource, SessionStatus, StateKind, ARENA_MONSTERS_MAX,
    PARTY_MAX, STATES_MAX, TURN_ORDER_MAX,
};

use deadroll_skillcheck::{Effect, Trigger};

use crate::{
    hit_event::{Applied, EffectRollData, HitItemsData, HitResultData},
    state::{AccuracyBuffData, AuraData, ResistModData, TimedModData, STORED_MODS},
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
    /// Patch 11: bit per reward cache entry that burned (a settled cache left without "Done", or
    /// an earlier party's past its DM deadline).
    pub cache_burned: u8,
}

#[event]
pub struct PlayerReady {
    pub session: Pubkey,
    pub slot: u8,
    pub ready: bool,
    /// The skills taken onto the map (`Participation.skills`), empty when not ready.
    pub skills: [u8; MAP_SKILLS],
    /// HP with the gear, copied at ready.
    pub hp: u32,
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
    /// a monster's by skill slot in the first `MONSTER_SLOTS` entries (patch 6).
    pub cooldowns: [u8; SKILLS_MAX],
    /// Patch 4: the part of `dot_damage` from a Freeze that thawed (Burning Frost).
    pub thaw_damage: u32,
}

/// Buffs and debuffs at the start of a unit's turn (`start_turn`, right after `TurnTicked`).
#[event]
pub struct ModsTicked {
    pub session: Pubkey,
    /// Turn-order id.
    pub unit: u8,
    pub turn_seq: u32,
    /// `deadroll_combat::support::mod_bit` of every buff or debuff that ended now.
    pub expired: u16,
    /// The unit's buffs and debuffs after the tick (`turns`: own turn starts left).
    pub mods: [TimedModData; STORED_MODS],
    /// Action points of this turn: speed with Haste and Slow.
    pub action_points: u8,
    /// Patch 9: the unit's Resistance after the tick.
    pub resist_mod: ResistModData,
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
    /// What the players' worn items put into this hit (weapon base, armor, defense, attack).
    pub items: HitItemsData,
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
    pub mods: [TimedModData; STORED_MODS],
    /// Patch 9: the target's Resistance after.
    pub resist_mod: ResistModData,
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

/// A player reached 0 HP: out of the battle at once (no turn, no target). Since patch 6 the
/// tombstone (`character.mark_dead`) follows in `start_turn` or a claim, `DeathFinalized`; a death
/// from over-time damage in `start_turn` is final in the same instruction.
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
pub struct RewardsRolled {
    pub session: Pubkey,
    /// Patch 11: a body, the boss table, a chest (docs/PARTY_AND_TURNS.md 9.3).
    pub source: RewardSource,
    /// The killed monster, or the chest.
    pub index: u8,
    /// Registry reward table id (`NO_TABLE` = none: experience only).
    pub table: u8,
    pub level: u8,
    /// By player slot: experience into the cache.
    pub xp: [u32; PARTY_MAX],
    /// By player slot: the reward and the boss guarantee's extra one (empty = none).
    pub rewards: [[CacheSlot; 2]; PARTY_MAX],
    /// Bit per player slot whose cache was full: what did not fit is lost.
    pub lost: u8,
}

/// Patch 11: a player opens a visible chest; the roll is pending.
#[event]
pub struct ChestOpenRequested {
    pub session: Pubkey,
    pub chest: u8,
    pub slot: u8,
    pub action: u32,
    pub action_points: u8,
}

/// Patch 11: a chest is open (it no longer occupies its cell); `RewardsRolled` has what it gave.
#[event]
pub struct ChestOpened {
    pub session: Pubkey,
    pub chest: u8,
    pub slot: u8,
}

/// Patch 11: a player's cache after the party: theirs to lay out (`granted`), or burned (dead)
/// or abandoned (fled). `xp` went to the character.
#[event]
pub struct CacheSettled {
    pub session: Pubkey,
    pub character: Pubkey,
    pub slot: u8,
    pub xp: u32,
    pub granted: bool,
}

/// Patch 11: a player took their contribution back after the DM's deadline passed.
#[event]
pub struct ContributionRefunded {
    pub session: Pubkey,
    pub player: Pubkey,
    pub slot: u8,
    pub amount: u64,
}

/// Patch 11: the owner released the character from its ended party.
#[event]
pub struct PartyLeft {
    pub session: Pubkey,
    pub character: Pubkey,
}

/// Patch 11: "Done" in the bag screen: the cache is closed, `applied` = the bag layout written
/// (false for a dead character).
#[event]
pub struct CacheClosed {
    pub character: Pubkey,
    pub session: Pubkey,
    pub applied: bool,
}

#[event]
pub struct EscrowClaimed {
    pub session: Pubkey,
    pub dm: Pubkey,
    pub amount: u64,
}

/// One monster a resonance passed a state on to (`deadroll_rules::spread`).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpreadHitData {
    /// Monster index.
    pub monster: u8,
    pub kind: StateKind,
    pub applied: Applied,
    /// The monster's states after.
    pub states: [ActiveState; STATES_MAX],
}

/// One monster a splash hit (resonance pass).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SplashHitData {
    /// Monster index.
    pub monster: u8,
    pub damage: u32,
    /// HP after.
    pub hp: u32,
    pub died: bool,
}

/// What a player's hit did beyond its target (docs/DAMAGE_TYPES_AND_MIXES.md 6.4), right after
/// its `HitResolved`: states that jumped to other monsters, a knockback, burning cells, and the
/// accuracy buff the attacker got. Only written when something happened.
#[event]
pub struct ResonanceSpread {
    pub session: Pubkey,
    pub action_counter: u32,
    /// Turn-order ids.
    pub attacker: u8,
    pub target: u8,
    pub hits: [SpreadHitData; 8],
    pub hit_count: u8,
    /// The target was pushed from `moved_from` to `moved_to`.
    pub moved: bool,
    pub moved_from: Cell,
    pub moved_to: Cell,
    /// Cells set on fire (`ignited[..ignited_count]`): Burning of `ignite_power` on whoever
    /// steps in, through round `until_round`.
    /// Patch B: 12, the ring around a 2x2 target (`deadroll_rules::spread::IGNITED_MAX`).
    pub ignited: [Cell; 12],
    pub ignited_count: u8,
    pub ignite_power: u32,
    pub until_round: u16,
    /// The attacker's accuracy buff after the hit.
    pub accuracy: AccuracyBuffData,
    /// The modifiers each of `hits` got (`hit_mods[i]` for `hits[i]`, `turns == 0` = none): the
    /// nova's first, then the jump modifiers (triples: up to four).
    pub hit_mods: [[TimedModData; SPREAD_MODS_MAX]; 8],
    /// Resonance pass: monsters a splash hit (`splashes[..splash_count]`).
    pub splashes: [SplashHitData; 8],
    pub splash_count: u8,
}

/// A unit stepped into a burning cell and caught fire (`move_unit`, `monster_move`).
#[event]
pub struct CellBurned {
    pub session: Pubkey,
    /// Turn-order id.
    pub unit: u8,
    pub x: u8,
    pub y: u8,
    pub applied: Applied,
    /// The unit's states after.
    pub states: [ActiveState; STATES_MAX],
}

/// What a triggered reactive skill did (`deadroll_rules::reactive::Reaction`).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactionKind {
    /// A counterattack: its `HitResolved` follows (one per unit of a line).
    Strike,
    /// A debuff on the enemy: its `EffectResolved` follows.
    Debuff,
    /// A buff on the bearer or an ally: its `EffectResolved` follows.
    Buff,
    /// A temporary aura around the bearer: its `EffectResolved` (the strength roll) and
    /// `AuraChanged` follow.
    Aura,
}

/// A reactive skill triggered by itself (docs/SKILL_BUILDER.md, section 3.2), on the dice of
/// the action that caused it (`action_counter`, after that action's own rolls).
#[event]
pub struct ReactiveTriggered {
    pub session: Pubkey,
    pub action_counter: u32,
    /// Player slot of the bearer.
    pub bearer: u8,
    /// The reactive skill's `SkillSet` index.
    pub skill_slot: u8,
    pub trigger: Trigger,
    pub reaction: ReactionKind,
    /// Turn-order id the reaction goes to: the enemy, the buffed player, or the bearer for an
    /// aura.
    pub target: u8,
    /// Own turn starts until it can trigger again.
    pub cooldown_left: u8,
}

/// An aura started, or ended (`aura.turns == 0`): docs/SKILL_BUILDER.md, section 3.5. Passive
/// auras start at `set_ready(true)` and end at `set_ready(false)` or the bearer's death.
#[event]
pub struct AuraChanged {
    pub session: Pubkey,
    /// Player slot of the bearer.
    pub bearer: u8,
    /// `0` the passive skill's aura, `1` the reactive skill's.
    pub aura_slot: u8,
    pub aura: AuraData,
}

/// Patch 6: a monster's reactive skill triggered by itself (docs/SKILL_BUILDER.md 3.2,
/// docs/BESTIARY.md 3.2), on the dice of the action that caused it, after the players' reactions.
#[event]
pub struct MonsterReactiveTriggered {
    pub session: Pubkey,
    pub action_counter: u32,
    /// Monster index of the bearer.
    pub bearer: u8,
    /// The reactive skill's monster slot (`deadroll_rules::monster::REACTIVE_SLOT`).
    pub skill_slot: u8,
    pub trigger: Trigger,
    pub reaction: ReactionKind,
    /// Turn-order id the reaction goes to: the player, the buffed monster, or the bearer for an
    /// aura.
    pub target: u8,
    pub cooldown_left: u8,
}

/// Patch 6: a monster's aura started, or ended (`aura.turns == 0`). A passive one starts at
/// `open_session` and ends with the bearer's death.
#[event]
pub struct MonsterAuraChanged {
    pub session: Pubkey,
    /// Monster index of the bearer.
    pub bearer: u8,
    /// `0` the passive skill's aura, `1` the reactive skill's.
    pub aura_slot: u8,
    pub aura: AuraData,
    /// On allies of the bearer's kin only.
    pub kin_only: bool,
}

/// Patch 6: the death of a player who went down in a VRF callback is final: `character.mark_dead`
/// ran (the tombstone; skills, bag and soulbound items burn). `PlayerDied` came at the fall.
#[event]
pub struct DeathFinalized {
    pub session: Pubkey,
    pub character: Pubkey,
    pub slot: u8,
}

/// Patch 10: another temporary key is bound to the party (docs/GAME_COMMISSIONS.md, 8).
#[event]
pub struct SessionKeyChanged {
    pub session: Pubkey,
    /// The player's character; `None` for the DM's key.
    pub character: Option<Pubkey>,
    /// `Participation.session_key` or `GameSession.dm_actor` now; default = none.
    pub session_key: Pubkey,
}

// Arena update (docs/PARTY_AND_TURNS.md 4.1).
/// Player `slot` stepped onto (`x`, `y`) next to pack `pack`'s visible monster: `enter_arena` next.
#[event]
pub struct PackEngaged {
    pub session: Pubkey,
    pub pack: u8,
    pub slot: u8,
    pub x: u8,
    pub y: u8,
}

/// The party entered pack `pack`'s arena: `party_first` from the pack's back. The living
/// players stand on `players` (by slot), the pack's monsters are the `MonsterState` slots,
/// `spawns` their layout spawns.
#[event]
pub struct ArenaEntered {
    pub session: Pubkey,
    pub pack: u8,
    pub party_first: bool,
    pub width: u8,
    pub height: u8,
    pub players: [Cell; PARTY_MAX],
    pub monsters: u8,
    pub spawns: [u8; ARENA_MONSTERS_MAX],
}

/// The last monster of pack `pack` died: `leave_arena` next.
#[event]
pub struct ArenaWon {
    pub session: Pubkey,
    pub pack: u8,
}

/// The party left pack `pack`'s arena for the map, each living player with the pack's
/// experience `xp` (by slot) in its cache; the pack is gone from the map.
#[event]
pub struct ArenaLeft {
    pub session: Pubkey,
    pub pack: u8,
    pub xp: [u32; PARTY_MAX],
}
