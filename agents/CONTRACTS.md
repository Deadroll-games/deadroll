# CONTRACTS: frozen interfaces between lanes

Written by lane L0 (lead) before parallel work starts. After L0 is merged, only the lead changes this file, in its own PR.
Field lists below are the **minimum** for P0. A lane may add fields at the end of its own accounts, never reorder or remove.

---

## 1. Program IDs

Filled at L0 from `anchor keys list` and committed to `Anchor.toml` (keypairs stay local, never committed).

| Program | Crate name | ID |
|---|---|---|
| registry | `deadroll_registry` | `dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG` |
| render | `deadroll_render_program` | `DgQCsXAe7Q7h8zdtToiTvk8XqNjjbXuD6xith2BmEiRB` |
| character | `deadroll_character` | `9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL` |
| items | `deadroll_items` | `48FtbrDegNrAUHFza9HdJzZAvUo3uUNqs1kNnZxFsqJS` |
| map | `deadroll_map` | `2YeyBfQ5xkRKD2kagxRS2yZYCQdWt5qidikaKfQhLBXH` |
| session | `deadroll_session` | `3gzfih2WaBgXcYZ57jWokmZM3FDE6QHX5y4R9EvKZoNJ` |
| city | `deadroll_city` | `2Cr5vB5awqDVx6XRcxtFFYQEG6Zy3aSAcKcqzh1xbVjS` |

Every program exposes `pub const ID` and a `cpi` feature so other programs can depend on it (`features = ["cpi"]`).

## 2. PDA seeds

| Account | Program | Seeds |
|---|---|---|
| `GameConfig` | registry | `["config"]` |
| `AspectTable` | registry | `["aspects"]` |
| `ResonanceTable` | registry | `["resonances"]` |
| `Bestiary` | registry | `["bestiary"]` |
| `Tileset` | registry | `["tileset"]` |
| `PrefabBook` | registry | `["prefabs"]` |
| `TraitTable` | registry | `["traits"]` (patch 4) |
| program signer | each program | `["authority"]` (used to sign CPI into other game programs) |
| `RasterizerCode` | render | `["rasterizer", version: u16 LE]` (created by client with SystemProgram, see `rasterizer` skill) |
| `Palette` | render | `["palette", version: u16 LE]` |
| `SpriteBook` | render | `["sprites", book_id: u16 LE]` |
| `PlayerProfile` | character | `["profile", owner]` |
| `Character` | character | `["character", owner, index: u8]` |
| `SkillSet` | character | `["skills", character]` |
| `Bag` | character | `["bag", character]` |
| `ItemState` | items | `["item", asset]` |
| `Loadout` | items | `["loadout", character]` (removed in patch 1, see section 8) |
| `Map` | map | `["map", dm, map_index: u32 LE]` |
| `MapLayout` | map | `["layout", map]` |
| `GameSession` | session | `["session", map]` |
| `Participation` | session | `["participation", session, character]` |
| `MonsterState` | session | `["monsters", session]` |
| `Escrow` | session | `["escrow", session]` |
| `TempStash` | session | `["stash", session, character]` |
| `City` | city | `["city"]` |

## 3. Shared types (`crates/types`)

All enums are `#[repr(u8)]`, Borsh-serializable, with `TryFrom<u8>`.

```
Aspect      Fire, Cold, Lightning, Blood, Blunt, Piercing
Family      Elemental, Physical, Mental, Sacral
Race        Human, ForestElf
Class       Warrior, Mage, Support
Source      Magical, Combat
Form        Strike, Projectile
TimeTag     Instant, DoT
StateKind   Burning, Chill, Freeze, Bleeding, Shock, Stun   (P0 uses Burning, Chill, Freeze, Bleeding)
Tile        Floor, Wall, Water, Door, Entrance, Exit, Trap
Tier        u8 in 1..=14 (1 is best)
SessionStatus  Lobby, Active, Victory, Wiped, Fled, Abandoned
MapStatus   Draft, Finalized, InSession, Finished
Stat        Strength, Dexterity, Intelligence, Endurance      (index into Character.stats)
ProgramIdx  Registry, Render, Character, Items, Map, Session, City   (index into GameConfig.programs)
EquipSlot   MainHand, OffHand, Helmet, Body, Pants, Boots, Gloves, Necklace, Cloak, Ring1, Ring2   (items lane)
ItemBase    Sword, Staff, Wand, LeatherArmor, Robe, LightLeatherArmor   (items lane, recipes append)
EquipSource Empty, Soulbound, Asset   (items lane)
```

Discriminants are sequential from 0 in the listed order and are part of the on-chain format: append only.
`Tier` is a validation type (`Tier::new(u8)`), not serialized; accounts and args carry a raw `u8`.

Constants: `BP = 10_000`, `MAP_MAX = 32`, `PARTY_MAX = 4`, `MONSTERS_MAX = 24`, `SKILL_SLOTS_ACTIVE = 4`, `BAG_SLOTS = 20`.
Added by the items lane: `SOULBOUND_MAX = 4`, `EQUIP_SLOTS = 11`, `EMPTY_ITEM = 0xFF`.
Added at L0: `MAP_CELLS = 1024`, `SPAWNS_MAX = 24`, `TURN_ORDER_MAX = 28`, `STATES_MAX = 6`, `LAYERS_MAX = 20`, `PROGRAMS_COUNT = 7`, `STATS_COUNT = 4`, `URI_BASE_LEN = 64`.
PDA seed prefixes from section 2 are constants in `deadroll_types::seeds` (`seeds::CHARACTER`, ...). Use them, never string literals.

Small structs (added at L0):
```
Cell          { x: u8, y: u8 }                                   (Anchor IDL has no tuples)
ActiveState   { kind: StateKind, turns: u8 (0 = empty slot), power: u32, source: u8 (turn-order slot) }
Monster       { bestiary_id: u8, level: u8, x: u8, y: u8, hp: u32, hp_max: u32, action_points: u8,
                alive: bool, pack: u8, flags: u8, states: [ActiveState; 6] }
```

Sprite render input (also used by `crates/render`):
```
RenderRequest { palette_version: u16, layers: [LayerRef; 20], flags: u32 }
LayerRef      { sprite_id: u16 (0 = empty), material_ramp: u8, accent_ramp: u8, element_ramp: u8 }
```

## 4. Minimum accounts for P0

```
GameConfig      { version, authority, programs: [Pubkey; 7], rasterizer_version: u16, rasterizer: Pubkey,
                  rasterizer_sha256: [u8; 32], palette: Pubkey, uri_base: [u8; 64], bump }
Character       { version, owner, index, nft: Option<Pubkey>, race, class, level: u16, xp: u32,
                  stats: [u16; 4], predisposition: Aspect, predisposition_tier: u8,
                  hp_max: u32, alive: bool, in_session: Option<Pubkey>, appearance: [LayerRef; 20],
                  state_version: u32, bump }
Map             { version, dm, index, width: u8, height: u8, status: Draft|Finalized|InSession|Finished,
                  layout_hash: [u8; 32], secrets_root: [u8; 32], bump }
MapLayout       { version, map, tiles: [u8; 1024], spawns: [Spawn; 24], spawn_count: u8, entrance: Cell, exit: Cell, bump }
Spawn           { bestiary_id: u8, x: u8, y: u8, level: u8, skills: u16, flags: u8, aspects: u8, pack: u8 }
GameSession     { version, map, dm, status, price_lamports: u64, players: [Pubkey; 4], player_count: u8,
                  turn_order: [u8; 28], turn_index: u8, round: u16, action_counter: u32,
                  pending_vrf: bool, last_seed: [u8; 32], bump }
Participation   { version, session, character, actor: Pubkey, slot: u8, x: u8, y: u8, hp: u32,
                  action_points: u8, states: [ActiveState; 6], ready: bool, bump }
MonsterState    { version, session, monsters: [Monster; 24], count: u8, bump }
Loadout         { version, character, owner, items: [ItemRecord; 4], item_count: u8,      (removed in patch 1, section 8)
                  equipped: [EquipRef; 11], bump }                     (items lane)
ItemRecord      { base: ItemBase, level: u8 }
EquipRef        { source: EquipSource, index: u8 (soulbound record), asset: Pubkey (item NFT, later) }
```

### 4.1 How programs use these layouts

`crates/types` holds each layout above as `<Name>Data` (`CharacterData`, `GameSessionData`, ...) with the fields in exactly this order, plus a `SIZE` constant (max borsh size, `Option` counted as `Some`). The owning program wraps it and appends its own fields after it:

```rust
use deadroll_types::CharacterData;

#[account]
#[derive(InitSpace)]
pub struct Character {
    pub core: CharacterData, // frozen contract, first after the discriminator
    // lane-specific fields, appended only
}
// space = 8 + Character::INIT_SPACE
```

Borsh adds no bytes for the wrapper, so `version` stays the first byte after the discriminator and owner fields keep fixed offsets for RPC `memcmp` filters. Programs depend on `deadroll-types` with `features = ["anchor"]` and forward `idl-build`, so the types appear in the IDL and the Codama sdk. Large accounts (`MapLayout`, `MonsterState`) should be `Box<Account<...>>` in instruction contexts.

## 5. CPI surface (only these, P0)

| Caller | Callee instruction | Signer check in callee |
|---|---|---|
| session | `character::lock_for_session(session)` | session `["authority"]` PDA |
| session | `character::grant_rewards(xp, resources)` | session PDA |
| session | `character::mark_dead()` (tombstone, freeze NFT, bump `state_version`) | session PDA |
| session | `character::unlock()` | session PDA |
| session | `map::set_status(InSession / Finished)` | session PDA |
| session | MagicBlock VRF `request_randomness` | per VRF SDK |
| session | `items::burn_bag_items()` | session PDA (P1) |
| items | `character::spend_resources()` | items PDA (P2) |
| city | `character::grant_achievement()` | city PDA (P2) |

## 6. Session flow (P0)

```
open_session(map, price)             DM; creates GameSession, MonsterState, Escrow; CPI map.set_status(InSession)
join_session(character, actor)       player; pays price to Escrow; creates Participation, TempStash; CPI character.lock_for_session;
                                     reads the items Loadout (last account) into Participation.equipment
set_ready / start_session            all ready -> roll initiative (VRF) -> Active
move(path)                           actor; spends action points; no rolls
act(skill_slot | base_attack, target) actor; requests VRF -> callback resolves hit via deadroll_combat;
                                     heal / buff skills: target = ally slot (own included), debuffs: monster
                                     index; optional trailing account target_participation = another ally
monster_act(monster, action, target) DM; same VRF path
end_turn                             actor or DM
on death in callback                 CPI character.mark_dead
on boss death                        status Victory; claim_rewards per player; DM claims escrow after audit
```

VRF callback instruction: `consume_randomness(randomness: [u8; 32])`, signed by the VRF program identity. It resolves the **pending action stored in `GameSession`** and emits `HitResolved` with the full breakdown.
Heals, buffs, debuffs and the effect of a damage skill emit `EffectResolved` (all of them roll on the same VRF value).
Accounts after the VRF identity: `session, monsters, participation, character, skill_set, session_authority, config, character_program, target_participation` - the last is optional (the program id when absent), set only for a heal or buff on another ally (`GameSession.pending_target`).

## 7. Events (all programs)

`CharacterCreated, CharacterDied, MapFinalized, SessionOpened, PlayerJoined, TurnStarted, Moved, ActionRequested, HitResolved { rolls, packets, resonance, states, damage }, EffectResolved, MonsterKilled, SessionEnded, RewardsGranted, RasterizerFinalized`.

## 8. Patch 1: gear in the character, map skills at Ready, death burns

Supersedes the `Loadout` lines above (sections 2, 4, 6) and adds to sections 4-6. The `crates/types`
items (`LoadoutData`, `seeds::LOADOUT`) stay, unused (this crate only grows).

```
Character (program struct, after the frozen core): ... nft_frozen: bool,
                gear: Gear { items: [ItemRecord; 4], item_count: u8, equipped: [EquipRef; 11] }
                CharacterData.version = 2 for characters created with gear
Participation (program struct, appended):  ... equipment: [u8; 11], skills: [u8; 6]
                skills = SkillSet indexes taken onto the map: 0-3 active, 4 reactive, 5 passive, 0xFF empty
```

CPI surface (section 5) after patch 1:

| Caller | Callee instruction | Accounts |
|---|---|---|
| session `set_ready(true)` | `character::lock_for_session(session, selection: [u8; 6])` | `character, session_authority, config, owner, skill_set` |
| session `set_ready(false)` | `character::unlock()` | `character, session_authority, config` |
| session (callback, `start_turn`) | `character::mark_dead()`: burns skills (names kept), bag, gear | `character, session_authority, config, skill_set (w), bag (w)` |

Character instructions added: `equip(item_index, slot)`, `unequip(slot)` (owner or session key,
`signer, character (w), session_token (optional)`). The items program has no instructions.

Session flow (section 6) after patch 1:

```
join_session(contribution, actor)    player; pays into Escrow; creates Participation, TempStash. No CPI,
                                     the character is not locked.
set_ready(ready, selection)          player main wallet, Lobby only. true: CPI character.lock_for_session
                                     (character and map skills checked, character locked: bag and gear
                                     frozen), copies HP, AP, initiative and Character.gear into
                                     Participation, writes Participation.skills. false: CPI character.unlock,
                                     clears the selection.
act(skill_slot, target)              skill_slot must be one of Participation.skills[0..4] (or 0xFF = weapon).
```

`consume_randomness` accounts after the VRF identity: `session, monsters, participation, character,
skill_set (w), bag (w), session_authority, config, character_program, target_participation`.
`act` and `monster_act` take `skill_set` and `bag` of the participation's character (seeds checked);
`start_turn` takes them as trailing optional accounts, required when over-time damage kills.

## 9. Patch 2: stats, trait tiers, spending stat points

Appended to section 8. `crates/types` is unchanged (the race trait tier stays
`CharacterData.predisposition_tier`).

```
Character (program struct, appended after gear):  ... gear: Gear, class_tier: u8
                class_tier = class trait tier, 1 best .. 14 (starts at PlayerTuning.class_start_tier)
                CharacterData.version = 3 for characters created with class_tier
Participation (program struct, appended):  ... skills: [u8; 6], class_tier: u8
                copied from Character.class_tier at set_ready(true), with the gear
```

Character instruction added: `spend_stat_points(points: [u16; 4])` (by `Stat`): the owner's main
wallet only (no session token), `owner (signer), character (w)`; a living character outside a
session; `0 < sum(points) <= unspent_points`. Writes the stats, `unspent_points`, `hp_max`, bumps
`state_version`. Event `StatPointsSpent { character, points, stats, unspent_points, hp_max }`.
Errors appended to `CharacterError`: `NoPointsSpent`, `NotEnoughStatPoints`.

Level-up numbers moved into `BalanceConfig.players` (`level_xp`, `stat_points_per_level`):
`grant_rewards` reads them there.

`crates/combat` (not a frozen contract, listed for the WASM layout): `ModFilter::SourceForm(Source,
Form)` appended as variant 5; `preview_hit` / `HitPreview` added. The website ships
`app/src/chain/wasm/combat.wasm` (ABI 2, `crates/combat/wasm/README.md`).

## 10. Patch 3: dice and states

`crates/types`: `StateKind::Penetration` appended (discriminant 6). `STATES_MAX` stays 6: the
state array sits in the middle of `ParticipationData` and `Monster`, so a seventh slot is not
added. A unit that already holds six states cannot take a seventh until one ends.

No account fields added. `Participation`, `GameSession` and `MonsterState` keep their layout, so
sessions opened before this upgrade still decode. The program logic, the hit event and the combat
WASM change, so the programs still need a devnet upgrade before the public site can use them.

Chance rolls (crit, resonance tint, state application, skill-effect rider) are a d100, `0..=100`,
success when `roll < chance`. Those chances in `BalanceConfig` are percents. Shares, multipliers
and counterweights stay in basis points.

`Attacker` gains `tempo_bonus_bp` at the end (Dexterity increase on Chill). `PacketResult` gains
`pen_attacker_bp`, `pen_state_bp`, `vuln_shock_bp` at the end. The session hit event mirrors them.
`StateKind` in the IDL gains `Penetration`.

Combat WASM ABI 3 (`crates/combat/wasm/README.md`).

## 11. Patch 4: resonances and tints

`crates/types`: seed `TRAITS = b"traits"` added (registry `["traits"]`). Nothing renamed or
removed.

Registry tables (`programs/registry/README.md`), validated with the decoder every reader uses
(`deadroll_combat::tables`):

```
TableKind::Traits = 5            ["traits"], 6 x 32 B: present, 0, hit op, template op x2, 12 x 0
Aspects record (8 B)             [aspect, family, flags, native state (0xFF none), trait id,
                                  condition of generated pairs, 0, 0]
Resonances record (16 B)         [condition, flags (bit 0: override), op 0, op 1, 0, 0]
Op (6 B)                         [kind | 0x80 crit only, arg, value u32 LE]; kinds 0..=20
                                  (deadroll_combat::tables::OpKind), arg 0xFF = the native state
Bestiary record                  byte 38: state immunity mask (bit = StateKind); 39..48 zero
```

`open_session` takes three more accounts at the end: `aspects`, `traits`, `resonances` (registry
table PDAs, address-checked, `InvalidRegistryTable` otherwise). It copies their bodies and each
spawn's bestiary defenses into `MonsterState`:

```
MonsterState (program struct, appended after mods):
  defense: [MonsterDefenseData { resist, immune_aspects, immune_states }; 24]
  res_aspects: [u8; 48], res_traits: [u8; 192], res_pairs: [u8; 576]
  burning_cells: [BurningCellData { x, y, until_round: u16, power: u32, source }; 16]
Participation (program struct, appended after class_tier):
  accuracy: AccuracyBuffData { aspect, turns, bonus: u32 }
```

`GameSession` is unchanged. `MonsterState` and `Participation` grow, so sessions opened before
this upgrade no longer load.

`consume_randomness` takes `layout` (the session's `MapLayout`, read-only) after
`target_participation`; `vrf::callback_metas` and every request (`act`, `monster_act`,
`start_session`) pass it.

Events: `HitResolved.result` gains `res_ops`, `jumps`, `jump_count`, `knockback`, `ignite_power`,
`target_mods`, `target_mod_count`, `accuracy_buff`, `landed`; `ResonanceData` gains
`second_tier`, `second_carrier`, `generated`. New `ResonanceSpread` (after a player's hit, when
a state jumped, the target was pushed, cells caught fire or the attacker got an accuracy buff)
and `CellBurned` (`move_unit` / `monster_move` into a burning cell). `TurnTicked` gains
`thaw_damage` at the end.

Combat WASM ABI 4: `set_tables`, `monster_sheet`; `TargetInput.defense` and
`AttackRequest.accuracy` appended; `HitPreview.tints` (`crates/combat/wasm/README.md`).

## 12. Patch 5: skills (range, timing, AP, reactive, passive, auras, line)

`crates/types`: `RangeTag { Melee, Ranged }` (discriminants 0, 1) and seed
`SKILL_EXTRAS = b"skill_extras"` added. Nothing renamed or removed.

`crates/skillcheck` (not frozen, listed for the IDL): `SkillExt { range: RangeTag, timing: TimeTag,
hp_below_pct: u8 }` (3 B); `SkillError` gains, at the end, `HealNotAvailable`,
`MissingThreshold`, `UnexpectedThreshold`, `ThresholdRange`, `PassiveDebuffNeedsAura`;
`Derived.ap_cost`. `SkillBlocks` and `SkillRecord` are unchanged.

Character program:

```
SkillExtras (new, zero-copy)  ["skill_extras", character], created by create_character
  version: u8, bump: u8, character: Pubkey,
  extras: [SkillExtra { range: u8, timing: u8, hp_below_pct: u8, ap_cost: u8,
                        description: [u8; 200] }; 16]          by SkillSet index
NewSkill (create_character arg)  { blocks, name, ext: SkillExt }   ext appended
create_character accounts        ... skill_extras (init) appended at the end
```

`SkillSet` keeps its layout. `SkillExtras` is not burned at death (the description stays with
the tombstone; the mechanics burn in `SkillSet`).

Character instruction added: `set_skill_description(skill_index: u8, description: [u8; 200])`,
`owner (signer), character, skill_set, skill_extras (w)`: the owner's main wallet, a living
character outside a session, `skill_index < SkillSet.count`, printable ASCII zero-padded and not
empty, written once (`DescriptionAlreadySet` otherwise). Event `SkillDescribed { character,
skill_index }`. Errors appended to `CharacterError`: `NoSuchSkill`, `InvalidDescription`,
`DescriptionAlreadySet`.

Session program, accounts (fields appended at the end):

```
Participation:  ... accuracy, map_skills: [MapSkillData; 6]
  MapSkillData { blocks, shares_bp: [u32; 3], power_bp, cooldown, ext: SkillExt, ap_cost }
  snapshot of the map skills at set_ready(true) (by Participation.skills slot)
GameSession:    ... pending_target, participations: [Pubkey; 4],
                reactives: [ReactiveData { skill_slot, skill: MapSkillData, cooldown_left }; 4],
                auras: [[AuraData; 2]; 4],
                pending_reactive: PendingReactiveData { kind, bearers, source, victim,
                                                        hp_before, hp_after, hp_max }
  AuraData { kind: ModKindData, aspect, power_bp: u32, radius, turns (255 = permanent, 0 = none),
             hostile, passive, self_only }
  auras[slot][0] = the passive skill's (set at set_ready), [1] = a reactive skill's (temporary)
  pending_reactive.kind: 0 none, 1 an enemy entered next to a bearer, 2 HP drop at start_turn
  pending_target: unused since patch 5 (always the default key)
```

`Participation` and `GameSession` grow, so sessions opened before this upgrade no longer load.

Instruction accounts:

| Instruction | Change |
|---|---|
| `set_ready` | `skill_extras` (read) appended |
| `join_session` | records `GameSession.participations[slot]` |
| `act` | `layout` appended (walls end a line); AP cost from the skill, range by `RangeTag` |
| `start_turn` | `payer` (signer, w) and the VRF accounts (`program_identity, oracle_queue, vrf_program, slot_hashes, system_program`) appended |
| `monster_move` | `signer` writable, the VRF accounts appended; the walk stops on the first cell next to a bearer of an `EnemyAdjacent` reactive skill and requests a reactive roll |
| `consume_randomness` | `target_participation` must be None; remaining accounts: `Participation (w), Character` of every other living player in slot order (`InvalidPartyAccounts` otherwise) |

`vrf::callback_metas` appends the same party accounts to every request (`act`, `monster_act`,
`monster_move`, `start_turn`); `start_session` passes none (the initiative roll returns before
the party is read, extra accounts are ignored). A reactive-only roll puts the first bearer's
Participation in `GameSession.pending`.

Errors appended to `SessionError`: `InvalidPartyAccounts`, `LineBlocked`,
`ReactiveAccountsMissing`. Events appended: `ReactiveTriggered { session, action_counter,
bearer, skill_slot, trigger, reaction: ReactionKind, target, cooldown_left }`,
`AuraChanged { session, bearer, aura_slot, aura: AuraData }`; `ReactionKind { Strike, Debuff,
Buff, Aura }`.

`BalanceConfig`: `PlayerTuning.act_ap` renamed `basic_attack_ap`, `strike_range` /
`projectile_range` renamed `melee_range` / `ranged_range`, `nearby_ally_cells` added;
`ItemBaseStats.basic_range`, `basic_form` added; `SupportBalance.passive_point_bp` added;
`SkillTuning` AP and HP-threshold numbers added. Balance JSON presets regenerated.

Combat WASM ABI 5: `AttackRequest.party`, `slot`, `target_at` appended; `unit_mods` and `line`
exports (`crates/combat/wasm/README.md`).

## 13. Patch 6: monsters from the registry (skills, kin, elite)

`crates/types` (added only): seed `MONSTER_SKILLS = b"monster_skills"`; `SPAWN_ELITE: u8 = 1`
(bit of `Spawn.flags` and `Monster.flags`); `MONSTER_SLOTS = 7`; `SESSION_SKILLS_MAX = 32`.

`crates/skillcheck` (not frozen): module `monster` (record codec), `validate_monster_skill`,
`check_monster_selection`; `SkillError` gains, at the end, `HealCategory`, `MonsterSkillUnknown`,
`MonsterSkillCount`.

Registry:

```
TableKind::MonsterSkills = 6     ["monster_skills"], 128 x 32 B
  [0] present, [1..13] SkillBlocks (borsh), [13..16] SkillExt, [16] flags (bit 0 kin only),
  [17..32] zero; write_table runs deadroll_skillcheck::validate_monster_skill (Heal allowed,
  never passive or an aura)
Bestiary record                  [39] pool_start, [40] pool_len (<= 16, a run of MonsterSkills),
                                 [41] kin (0xFF none; names in crates/rules registry.rs KINS),
                                 [42..48] zero
```

The p0 monster table is gone: every monster number (accuracy, evasion, armor, damage, XP, size,
range, pool, kin) comes from the bestiary record.

Map: `finalize_map` accounts `bestiary`, `monster_skills` (registry tables, addresses checked,
`InvalidRegistryTable`) appended. `Spawn.skills` is the chosen pool bits: exactly
`min(slots, pool)` per category (slots by rank in `BalanceConfig.monsters.slots`: regular
`[3, 0, 1]`, elite `[4, 0, 1]`, boss `[5, 1, 1]`). Errors appended to `MapError`:
`TierBelowTemplate`, `MonsterSkillUnknown`, `MonsterSkillCount`, `EliteBoss`, `TooManyElites`,
`TooManySessionSkills`, `InvalidRegistryTable`.

Session program, accounts (fields appended at the end):

```
MonsterState:  ... burning_cells, records: [[u8; 48]; 24], skill_table: [[u8; 16]; 32],
               skill_count: u8, skills: [[u8; 7]; 24], skill_cooldowns: [[u8; 7]; 24],
               auras: [[MonsterAuraData; 2]; 24]
  records[i]      the spawn's bestiary record, copied at open_session
  skill_table     each chosen registry skill once (its record bytes 1..17)
  skills[i][slot] skill_table index or 0xFF; slots 0..5 active, 5 reactive, 6 passive
  MonsterAuraData { aura: AuraData, kin_only: bool }; auras[i][0] passive (from open_session,
                  turns 255), [1] the reactive skill's (temporary)
  cooldowns       unused since patch 6 (skill_cooldowns replaces it)
GameSession:   ... pending_reactive, pending_monster_reactive: PendingMonsterReactiveData
                 { kind, bearers: u32 (monster bits), source, victim, hp_before, hp_after, hp_max },
               pending_deaths: u8
  pending_deaths  players down in a VRF callback whose death is not final yet, one bit per slot
                  (the queue, finalized in slot order)
```

`MonsterState` and `GameSession` grow, so sessions opened before this upgrade no longer load.
`MonsterState` costs about 2.6 KB more rent per session.

| Instruction | Change |
|---|---|
| `open_session` | `monster_skills` (registry table) appended; elite HP `+ elite_hp_bp` |
| `monster_act(monster, action, target)` | `action` 0 = natural attack, `1 + slot` = an active skill (`InvalidAction` above 7). Enemy skills: `target` = player slot, `participation` = its. Heal and buff: `target` = monster index (itself allowed, the same kin for a kin-only skill), `participation` = the lowest living player's. `layout` appended. Sets the slot's cooldown, emits `SkillUsed { unit: 4 + monster }` |
| `move_unit` | `signer` writable, the VRF accounts appended (a monster's `EnemyAdjacent` reactive) |
| `consume_randomness` | account list unchanged; it no longer calls `character.mark_dead`: a player at 0 HP is down (`alive_mask` cleared, `PlayerDied`) and queued in `pending_deaths`. `character`, `skill_set`, `bag`, `session_authority`, `config`, `character_program` stay in the list but are not written |
| `start_turn` | remaining accounts: a writable `Character, SkillSet, Bag` triple per `pending_deaths` bit, in slot order (`InvalidDeathAccounts` otherwise; none when the queue is empty). It finalizes them first (`mark_dead` each, `DeathFinalized`); when the turn already started it returns after that, ending the turn of a current player who went down |
| `claim_rewards` | the same remaining accounts; the queue is finalized first |
| `claim_escrow` | `session` writable, `character_program` appended; the same remaining accounts, finalized first |
| `act`, `monster_act`, `move_unit`, `monster_move`, `end_turn`, `flee` | `DeathsPending` while `pending_deaths != 0` |

`ActionRequested.skill_slot` / `EffectResolved.skill_slot` of a monster: the action (0 natural
attack, `1 + slot`) and the monster skill slot respectively. A monster's reactive-only roll puts
the named player's Participation in `GameSession.pending`.

Errors appended to `SessionError`: `TooManyMonsterSkills`, `MonsterSkillMissing`, `NotSameKin`,
`InvalidMonsterSkills`, `DeathsPending`, `InvalidDeathAccounts`. Events appended:
`MonsterReactiveTriggered { session, action_counter, bearer, skill_slot, trigger, reaction, target,
cooldown_left }`, `MonsterAuraChanged { session, bearer, aura_slot, aura, kin_only }`,
`DeathFinalized { session, character, slot }`. `PlayerDied` now marks the fall; the tombstone
(`CharacterDied` from the character program) comes with `DeathFinalized`. A death from over-time
damage in `start_turn` is final in the same instruction. The callback's party accounts stay pairs
(patch 5).

`BalanceConfig.monsters` (`MonsterScaling`): `elite_hp_bp`, `slots: [[u8; 3]; 3]` (rank x
category), `ai_heal_below_pct` added.

Combat WASM ABI 6: monsters by record bytes (`RecordInput`); exports `monster_skill`,
`spawn_skills`, `preview_monster_attack`, `monster_plan`; `PartyInput.bearers`
(`crates/combat/wasm/README.md`).

## 14. Patch 7: render format 2, sprite books in GameConfig, finalize writes GameConfig

Additions only. `crates/types` is unchanged.

`GameConfig` (registry): one registry field after the core (section 4.1), account size `8 + 420 + 128`:
```
GameConfig { core: GameConfigData, sprite_books: [Pubkey; 4] }   lookup order, Pubkey::default() = empty slot
```
A `GameConfig` created before patch 7 (`8 + 420` bytes) is grown once by `registry::resize_config()`
(authority; rent top-up; new bytes zero). Until then no instruction loads it as `GameConfig`; programs
that read only `GameConfigData` at offset 8 are not affected.

New registry instructions:

| Instruction | Who | Accounts | What |
|---|---|---|---|
| `resize_config()` | `GameConfig.authority` | `authority (w, signer), config (w), system_program` | one-time migration to the patch 7 size |
| `set_sprite_books()` | `GameConfig.authority` | `authority (signer), config (w)`, then 1..=4 finalized `SpriteBook`s as remaining accounts | `sprite_books` = the books in order, other slots cleared; `SpriteBooksSet` |
| `set_rasterizer_from_render()` | render `["authority"]` PDA (CPI only) | `render_authority (signer), config (w), blob` | the same pull as `set_rasterizer` |

New CPI (section 5):

| Caller | Callee instruction | Signer check in callee |
|---|---|---|
| render `finalize_rasterizer` | `registry::set_rasterizer_from_render()` | render `["authority"]` PDA from `GameConfig.programs[Render]` |

`render::finalize_rasterizer` accounts: `authority (signer, = GameConfig.authority), rasterizer (w),
config (w), render_authority, registry_program`. Finalize hashes, checks, freezes and then points
`GameConfig` at the new rasterizer (`docs/STORAGE_AND_DELIVERY.md` 3.2).

Events: registry `SpriteBooksSet { sprite_books: [Pubkey; 4] }`, `ConfigResized { old_len: u32, new_len: u32 }`.
Errors (registry, appended): `UntrustedCaller, WrongConfigAccount, ConfigAlreadyResized, SpriteBookCount, DuplicateSpriteBook`.

Sprites (`crates/render/FORMAT.md`): sprite format 2, canvases 32/64/96/128 (character layers and
tiles 64x64, icons 32x32, monsters 64/96/128). Rasterizer version 2 and sprite book 2 hold format 2;
version 1 and book 1 stay for format 1 images. Sprite ids, `LayerRef` and `RenderRequest` are unchanged.
The rasterizer interface is `render(sprites, palette, state, asset, mode)` (`ART_DESCRIPTION.md` 2.2):
RAW_RGBA is the canvas 1:1, PNG is an indexed PNG of 512 px on the longer side.
