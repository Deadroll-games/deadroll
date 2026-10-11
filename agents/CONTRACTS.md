# CONTRACTS: frozen interfaces between lanes

Written by lane L0 (lead) before parallel work starts. After L0 is merged, only the lead changes this file, in its own PR.
Field lists below are the **minimum** for P0. A lane may add fields at the end of its own accounts, never reorder or remove.

---

## 1. Program IDs

Filled at L0 from `anchor keys list` and committed to `Anchor.toml` (keypairs stay local, never committed).

| Program | Crate name | ID |
|---|---|---|
| registry | `deadroll_registry` | `<fill at L0>` |
| render | `deadroll_render_program` | `<fill at L0>` |
| character | `deadroll_character` | `<fill at L0>` |
| items | `deadroll_items` | `<fill at L0>` |
| map | `deadroll_map` | `<fill at L0>` |
| session | `deadroll_session` | `<fill at L0>` |
| city | `deadroll_city` | `<fill at L0>` |

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
```

Constants: `BP = 10_000`, `MAP_MAX = 32`, `PARTY_MAX = 4`, `MONSTERS_MAX = 24`, `SKILL_SLOTS_ACTIVE = 4`, `BAG_SLOTS = 20`.

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
MapLayout       { version, map, tiles: [u8; 1024], spawns: [Spawn; 24], spawn_count: u8, entrance: (u8,u8), exit: (u8,u8) }
Spawn           { bestiary_id: u8, x: u8, y: u8, level: u8, skills: u16, flags: u8, aspects: u8, pack: u8 }
GameSession     { version, map, dm, status, price_lamports: u64, players: [Pubkey; 4], player_count: u8,
                  turn_order: [u8; 28], turn_index: u8, round: u16, action_counter: u32,
                  pending_vrf: bool, last_seed: [u8; 32], bump }
Participation   { version, session, character, actor: Pubkey, slot: u8, x: u8, y: u8, hp: u32,
                  action_points: u8, states: [ActiveState; 6], ready: bool, bump }
MonsterState    { version, session, monsters: [Monster; 24], count: u8 }
```

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
join_session(character, actor)       player; pays price to Escrow; creates Participation, TempStash; CPI character.lock_for_session
set_ready / start_session            all ready -> roll initiative (VRF) -> Active
move(path)                           actor; spends action points; no rolls
act(skill_slot | base_attack, target) actor; requests VRF -> callback resolves hit via deadroll_combat
monster_act(monster, action, target) DM; same VRF path
end_turn                             actor or DM
on death in callback                 CPI character.mark_dead
on boss death                        status Victory; claim_rewards per player; DM claims escrow after audit
```

VRF callback instruction: `consume_randomness(randomness: [u8; 32])`, signed by the VRF program identity. It resolves the **pending action stored in `GameSession`** and emits `HitResolved` with the full breakdown.

## 7. Events (all programs)

`CharacterCreated, CharacterDied, MapFinalized, SessionOpened, PlayerJoined, TurnStarted, Moved, ActionRequested, HitResolved { rolls, packets, resonance, states, damage }, MonsterKilled, SessionEnded, RewardsGranted, RasterizerFinalized`.
