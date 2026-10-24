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

TypeScript reads the ids from `@deadroll/sdk` (`PROGRAMS`, generated from the IDL addresses); an sdk test keeps them equal to `Anchor.toml`.

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
| `RewardTable` | registry | `["rewards"]` (patch 11, section 20) |
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
| `TempStash` | session | `["stash", session, character]` (retired in patch 11, see section 20) |
| `RewardCache` | session | `["cache", character]` (patch 11, section 20) |
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
| session | `character::grant_rewards(xp, resources)` (retired in patch 11, section 20) | session PDA |
| session | `character::grant_xp(xp)` (patch 11) | session PDA |
| session | `character::apply_bag(cache, layout)` (patch 11) | session PDA |
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

## 15. Patch 7c: BITMAP sprites, palette format 2, 96 px characters

Additions only. `crates/types` is unchanged (`RenderRequest` stays 106 bytes, the WASM ABI is the same).

`RenderRequest.flags` (`crates/render/FORMAT.md` 3.4), new bits in the existing `u32`:

| Bit | Name | Meaning |
|---|---|---|
| 0 | `GRAY` | dead character, gray ramps (unchanged) |
| 1 | `CURSE` | every ramp blended half way to the palette curse ramp, curse outline |
| 2 | `COLD` | cold outline (`CURSE` wins when both are set) |

Bits 3..31 stay reserved (error 9).

Palette body format 2 (`FORMAT.md` 3.2): header `format = 2, palette_version, outline rgb,
dark_ramp, curse_ramp, cold_ramp, ramp_count`, ramps of 6 colors (18 bytes), materials of 3 bytes
(`primary, secondary, flags`, bit 0 = dither). Palette 2 keeps every ramp and material index of
palette 1 and only appends, so `Character.appearance` of existing characters draws unchanged in the
new style. Palette 1 (format 1) stays for rasterizer version 1.

Sprites: new command `BITMAP` (opcode 0xB, run-length coded pixels, each a slot and an optional ramp
color). Character layers are 96x96, tiles 64x64, monsters 96 or 128. Sprite ids are unchanged. `art pack`
writes `book_<id>.bin` books of at most 64 KiB (ids 2, 3, ..., at most 4); identical sprites share
bytes inside a book. Deploy scripts upload every book and pass all of them to `set_sprite_books` in
id order. The app takes the palette version from the palette `GameConfig.palette` points at.

Character layers (sprite format 2, before it reaches devnet): the body of a race is one sprite and
the stacking order changed. `Character.appearance[i]` is the layer of type `i`
(`crates/render/FORMAT.md` 1.4):

| Index | Layer | Index | Layer |
|---|---|---|---|
| 0 | shadow | 8 | hair |
| 1 | back_item | 9 | offhand (shield, in front of the body and the hair) |
| 2 | body (one sprite per race) | 10 | helmet |
| 3 | glove_back | 11 | mainhand |
| 4 | pants | 12 | fist_front (over the weapon) |
| 5 | boots | 13 | glove_front |
| 6 | armor_body | 14 | overlay_element |
| 7 | necklace | 15..19 | not used |

Body sprites: 1 shadow, 10 human body, 11 human hair, 12 elf body, 13 elf hair, 14 front fist. The
old body part ids (2-7, 20-25) and the wound overlay are gone: a character created before this
change fails to render (error 7) until its appearance is rebuilt.

Character image (no class outfits): `Character.appearance` holds only the shadow (index 0), the body
(2), the hair (8) and the front fist (12). Clients draw a character as that appearance dressed in
what `Character.gear` has equipped: item sprite `100 + ItemBase` (Sword 100, Staff 101, Wand 102,
LeatherArmor 103, Robe 104, LightLeatherArmor 105) in the layer of its `EquipSlot` (MainHand ->
11 mainhand, Body -> 6 armor_body; the other slots map to their layers of the table above, rings and
gloves draw nothing yet). Equip and unequip change the image. The class kit sprites (helmet,
shield, cape, pants, boots, hoods, bags, amulet, mace; ids 100-106, 200-205, 300-305) are gone.

## 16. Patch 9: resistances

Combat (`deadroll_combat`), appended:

```
ModKind:   6 Resistance (after Vulnerability); MODS_MAX = 7, each kind in the slot `kind as usize`
TimedMod:  ... source, weights_pct: [u8; 6]       share of power_bp per Aspect, percent (Resistance only)
Defender:  ... states, tag_resist_bp: [u32; 4], immune_tags: u8
           tags (TAGS = 4): bit 0 Magical, 1 Combat (Source), 2 Strike, 3 Projectile (Form)
Effect:    10 Resistance (deadroll_skillcheck, a buff like Protection)
```

Aspect resistance cuts only the damage of its packet; the chance of a state (native, resonance,
a skill's effect) meets only the state resistance. A Stun or Freeze effect on a target immune to
that state rolls nothing (`EffectRoll.outcome = Immune`). Chill buildup never freezes a target
immune to Freeze.

Bestiary record: byte 42 resisted tags, byte 43 immune tags (bits as above), 44..48 zero.
`bestiary.json`: `tag_resistance`, `tag_immunity` (names Magical, Combat, Strike, Projectile).

Session program, accounts (fields appended at the end; `mods` keeps its 6 slots, the kinds before
Resistance by `ModKind`):

```
Participation: ... map_skills, resist_mod: ResistModData
MonsterState:  ... auras, resist_mods: [ResistModData; 24], aura_weights: [[[u8; 6]; 2]; 24]
GameSession:   ... pending_deaths, aura_weights: [[[u8; 6]; 2]; 4]
  ResistModData { data: TimedModData, weights_pct: [u8; 6] }   Borsh-identical to a combat TimedMod
  aura_weights[i][slot]  the weights of auras[i][slot] (a Resistance aura)
ModKindData: 6 Resistance
```

Events: `ModsTicked` and `EffectResolved` append `resist_mod: ResistModData` (their `mods` are
the 6 stored slots); `HitResultData.target_mods` is `[ResistModData; 3]`.

## 17. Patch 9b: aspect resistance against its state, resistance to each state

Combat (`deadroll_combat`), appended:

```
Balance:    ... burning_cell_rounds, aspect_state_resist_share_bp: u32   (default 2_500)
StateRoll:  ... applied, resist_bp: u32       the counterweight the chance met, 0 when not rolled
EffectRoll: ... rolls_used, resist_bp: u32    the target's counterweight of an effect chance
Defender::state_counter(kind, tables, balance) = state_resist_bp[kind]
    + aspect_state_resist_share_bp x resist_bp[the aspect whose native state is kind] (capped)
```

Every chance of a state (native, resonance, a skill's Stun or Freeze effect) meets
`state_counter`; Freeze is no aspect's native state. A modifier effect meets the strongest state
resistance.

Rules: `ItemBaseStats.state_kind_resist_bp: [u32; 7]` (by `StateKind`, on top of
`state_resist_bp`); `hero::state_resists` gives `Defender.state_resist_bp`.

Session events: `StateRollData` and `EffectRollData` append `resist_bp: u32`. No account changes.

## 18. Patch 10: a session key for one party (MagicBlock Session Keys)

`crates/types` `ParticipationData`: two fields take the names of docs/GAME_COMMISSIONS.md 5. Same
position and type, so the bytes and every open party stay as they are; only the IDL, the sdk and
the Rust and TS field names change:

```
ParticipationData { version, game_session (was session), character, session_key (was actor), slot, ... }
```

`join_session(contribution, actor)` keeps its signature (section 8), but `actor` must be
`Pubkey::default()` (`SessionKeyAtReady` otherwise): joining binds no key, `Participation.session_key`
stays default until Ready (docs/PARTY_AND_TURNS.md 2.1). Ready is one main wallet transaction:
`session_keys::create_session_v2` (token + prepayment), `set_session_key(key)`,
`set_ready(true, selection)`; when the key bound to the participation is still live, only
`set_ready(true)`. `set_ready(false)` leaves the key bound. `open_session(price, dm_actor)` takes the
DM's key after its `create_session_v2` in the same transaction.

Session program, instruction added:

```
set_session_key(session_key: Pubkey)   main wallet only; accounts signer, session (w), participation (w, optional)
    with participation: its player; writes Participation.session_key
    without:            the DM; writes GameSession.dm_actor
    Lobby or Active only (PartyEnded otherwise); Pubkey::default() unbinds. Creates no account.
```

Event `SessionKeyChanged { session, character: Option<Pubkey> (None = DM), session_key }`.
Errors appended to `SessionError`: `InvalidToken`, `PartyEnded`, `SessionKeyAtReady`. Appended to `CharacterError`:
`InvalidToken`. The old `SessionTokenMissing`, `InvalidSessionToken`, `SessionTokenExpired` stay,
unused.

Game instructions (`act`, `move_unit`, `end_turn`, `flee`, `claim_rewards`, `monster_act`,
`monster_move`) and `character::equip` / `unequip`: `session_token` is
`Option<Account<SessionTokenV2>>` (same position), checked by the `session-keys` macros
(`#[derive(Session)]`, `#[session_auth_or]`; the character program implements `SessionV2` with the
session program as the target). A signer that is not the main wallet must also be the key bound to
the party: `Participation.session_key`, or `GameSession.dm_actor` for monster actions.
Without a token a session key fails with `Unauthorized`; a token of another wallet, key or program,
or an expired one, with `InvalidToken`; a live token for a key the party does not have, with
`Unauthorized`.

`deadroll_rules::BalanceConfig.fees: FeeTuning { prepay_lamports, low_balance_lamports,
max_key_lamports, key_validity_secs }` (appended; defaults 0.01 SOL, 0.002 SOL, 0.05 SOL, 3 h), read
by the website from `balance.json`. No program reads it.

## 19. Patch 14: skill blocks move to crates/types

`crates/types`: new module `skill` (not glob-exported at the crate root) with `Category`,
`Purpose`, `Target`, `Effect`, `Trigger`, `SkillBlocks`, `SkillExt` and `SKILL_ASPECTS_MAX`,
moved from `crates/skillcheck` with the same derives, discriminants and field order. Nothing
renamed or removed: `deadroll_skillcheck` re-exports them (`pub use deadroll_types::skill::*`),
so every `deadroll_skillcheck::SkillBlocks` path is unchanged. `crates/skillcheck` loses its
`anchor` and `idl-build` features (no Anchor outside `types`, docs/ARCHITECTURE.md 5).

Same borsh bytes and `InitSpace` (pinned in `crates/types/tests/roundtrip.rs`); IDLs unchanged.

## 20. Patch 11: rewards, visible chests, the cache and the bag

Docs: docs/PARTY_AND_TURNS.md 2 (10), 3.2, 5.1, 7-9, 10.2; docs/MAP_DESCRIPTION.md 7.1, 10;
docs/BESTIARY.md 9; docs/ITEMS_AND_CRAFT.md 2, 6.1. The decisions of patch 11 fill the gaps.
Every layout below changed: maps, parties and bags made before the patch are not read after the
upgrade (agreed, no migration).

`crates/types`, added:

```
consts  CHESTS_MAX = 4, NO_TABLE = 0xFF, CACHE_SLOTS = 32, CACHE_BYTES = CACHE_SLOTS * 5,
        BAG_RESOURCE_SLOTS = 40, BAG_CONSUMABLE_SLOTS = 5
enums   PointKind { Chest }, RewardKind { Resource, Recipe, Consumable, Blank, Reagent },
        RewardSource { Body, Chest, Boss, Stash }, CacheStatus { Empty, Open, Claimed }
structs MapPoint { kind, x, y, data }                 data: a chest's reward table id
        CacheSlot { kind, id: u16, qty: u16 }         qty 0 = empty; read_all / write_all on raw bytes
        BagSlot { kind, id: u16, qty: u8 }            qty 0 = empty (kind and id zero too)
        BagLayout { resources: [BagSlot; 40], consumables: [BagSlot; 5] }
MapLayoutData appended: chests: [MapPoint; CHESTS_MAX], chest_count: u8, boss_table: u8
seeds   REWARDS = "rewards" (registry table), CACHE = "cache" (session RewardCache)
```

Registry: `TableKind::Rewards` (appended), records of `deadroll_rules::loot::record` (108 bytes:
present, level_base, level_max, cost u16, up to 4 levels of up to 6 `(kind, id u16, qty)`
entries), 32 at most. Bestiary record byte 44 (from the reserve) = the body's reward table
(`NO_TABLE` = none).

`deadroll_rules::BalanceConfig.loot: LootTuning` (appended): chests per tier, budget by tier and
minimum players, allowed tables per tier (u64 mask), `level_step_bp`, `chest_open_ap`,
the limits of what a DM sets for a map (`chest_radius_min/max` 1..=10, `settle_timeout_min/max_secs`
600..=36_000), over-tier experience and loot, stack limits.

Map program:

```
Map appended: min_players: u8, chest_radius: u8, settle_timeout_secs: u32
MapSettings { min_players, chest_radius, settle_timeout_secs }
create_map(index, width, height, tier, settings: MapSettings)   settings appended (the radius
                                                     within LootTuning limits; the time to settle
                                                     at most 10 h, its 10 min minimum is open_session's)
write_chests(chests: Vec<MapPoint>, boss_table)      DM, Draft only (CHESTS_MAX at most)
finalize_map                                         account rewards (registry table) appended;
                                                     chest and budget rules (MapCheckError appended)
```

Character program:

```
Bag: resources: [u32; 8] removed; layout: BagLayout appended at the end
grant_xp(xp)                       session PDA; living character (it may be in a later party by
                                   now); replaces grant_rewards
apply_bag(cache: [CacheSlot; 32], layout: BagLayout)
                                   session PDA; living, not in a party; check_bag_layout
grant_rewards                      retired (removed)
events  XpGranted { character, xp, level, total_xp }, BagApplied { character }; RewardsGranted retired
errors  appended: BagDirtySlot, BagWrongSection, BagStackTooBig, BagMoreThanOwned
```

Session program:

```
GameSession: xp_pool removed; appended tier, min_players, chests, chest_count, boss_table,
             chests_opened, pending_chest, xp_bp: [u16; 4], loot_mask, unpaid_kills: u32, ended_at: i64,
             chest_radius: u8, settle_timeout_secs: u32 (the map's, copied at open_session)
Participation appended: cache_xp: u32, cache: [u8; CACHE_BYTES]   (raw CacheSlots, last in the account)
MonsterState appended: loot_ids: [u8; 12], loot_tables: [[u8; 108]; 12]
RewardCache ["cache", character]: version, character, entries: [CacheEntry; CACHE_PARTIES = 3], bump
CacheEntry  { session, status: CacheStatus, xp: u32, slots: [CacheSlot; 32] }   one per party
TempStash   retired (removed)

open_session        account rewards (registry table) appended: copies the bodies', chests' and
                    boss's tables; tier, minimum players, chests, chest radius and time to
                    settle from the map; SettleTimeoutTooShort below 10 min (not in mock-vrf)
join_session        account stash replaced by reward_cache (init_if_needed); remaining accounts:
                    the GameSession of every Open entry. A Claimed entry burns; an Open one burns
                    if its party ended and its deadline passed, else it waits; this party takes a
                    free entry (TooManyPendingParties). PlayerJoined appended cache_burned: u8 mask
leave_party         owner main wallet: a living character locked to an ended party is unlocked
                    (CPI unlock); its rewards keep waiting in the Participation
set_ready(true)     xp_bp and loot_mask from the character's level and the map tier
start_session       NotEnoughPlayers below the map's minimum
open_chest(chest)   owner or session key, own turn, next to the chest, AP, no living monster
                    within the map's chest_radius of the chest or of any living player;
                    VRF request, the callback rolls the chest
consume_randomness  the chest roll; the kills pay out at the end (experience and body rewards
                    into each living player's cache, the boss table with its guarantee), one by one
                    while sol_remaining_compute_units > PAYOUT_RESERVE_CU of the oracle's 256k;
                    the rest stay in unpaid_kills for the next callback or the settlement
settle_party        DM main wallet, before ended_at + settle_timeout_secs: deaths first, then a
                    (Participation, RewardCache, Character) triple per player: the party's entry
                    Claimed (or emptied), grant_xp for the living, unlock only if still locked to
                    this party, escrow to the DM, map Finished
                    (replaces claim_rewards and claim_escrow, both removed)
claim_after_timeout after the deadline, the player's main wallet: its own triple first, then the
                    deaths; the same settlement for its character, its contribution back; the
                    last claim finishes the map
close_cache(session, layout) owner main wallet, that party's entry Claimed: apply_bag (living,
                    not locked), the entry empties; the account closes once every entry is empty
events  RewardsRolled, ChestOpenRequested, ChestOpened, CacheSettled, ContributionRefunded,
        CacheClosed, PartyLeft; RewardsClaimed retired
errors  appended: TooManyLootTables, InvalidRewardTables, NotEnoughPlayers, InvalidChest,
        ChestOpened, ChestNotAdjacent, MonsterNearChest, ChestInTheWay, SettleDeadlinePassed,
        SettleDeadlineNotPassed, InvalidSettleAccounts, DmMainWalletOnly, CacheNotClaimed,
        InvalidRewardCache, NotCacheOwner, SettleTimeoutTooShort, TooManyPendingParties,
        InvalidCacheSessions, NotLockedHere
map errors appended: ChestRadiusOutOfRange, SettleTimeoutOutOfRange
```

A closed chest occupies its cell for movement and knockback; an open one does not. Roll indexes of
the reward events start at `LOOT_ROLL_BASE` in the paying action's VRF stream, after the combat
rolls (rule 10). The DM's deadline is `ended_at + GameSession.settle_timeout_secs`; a `mock-vrf`
session (localnet only) accepts maps with seconds, so the tests reach the claims after it.
