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
