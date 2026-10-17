# programs/registry

`GameConfig` (trusted program ids, rasterizer pointer, NFT uri base) and the data tables the game
programs read without CPI (`docs/ARCHITECTURE.md` 4.2, 4.3). Owner lane: L4.

## Accounts

| Account | Seeds | Body |
|---|---|---|
| `GameConfig` | `["config"]` | `GameConfigData` from `crates/types` (frozen), nothing appended |
| `Table` (aspects, resonances, bestiary, tileset, prefabs, traits) | `["aspects"]`, `["resonances"]`, `["bestiary"]`, `["tileset"]`, `["prefabs"]`, `["traits"]` | 8-byte header + 4096-byte body, zero copy |

`Table` data: 8 discriminator, `version u8`, `kind u8`, `bump u8`, `pad u8`, `len u32 LE` (highest end offset
written), then the body at offset 16. Readers take records straight from the body at `16 + index * record_size`.

## Instructions

| Instruction | Who | What |
|---|---|---|
| `init_config(programs[7], uri_base[64])` | first caller becomes authority | creates `GameConfig`; every slot non-empty, slot 0 = this program |
| `set_programs(programs[7])`, `set_uri_base(uri_base)` | authority | replace the list / the uri prefix |
| `set_rasterizer()` | authority | **pulls** from render (no CPI): `blob` must be owned by `programs[Render]`, be its PDA `["rasterizer", id]`, kind 1, `finalized`; copies `id`, address, `sha256` (offsets in `programs/render/README.md`) |
| `set_palette()` | authority | same pull for `Palette` (kind 2), copies the address |
| `init_table(kind)` | authority | creates an empty table |
| `write_table(kind, offset, bytes)` | authority | whole, aligned, valid records only; tables stay editable |

`TableKind`: 0 Aspects, 1 Resonances, 2 Bestiary, 3 Tileset, 4 Prefabs, 5 Traits (append only).
TODO(post-demo): the same pull for `SpriteBook` (`GameConfigData` has no field for it).

## Record layouts (little-endian, validated by `write_table`, source: `src/state/records.rs`)

Index of a record = its position in the table. Reserved bytes are zero.

The aspect, trait and resonance records are the format of `crates/combat/src/tables.rs`: `write_table` validates them with its decoder, the one the session (its copy at `open_session`), the simulator and the website use.

**Aspects** (6 x 8 B): `[0] aspect id (= index, Aspect order Fire, Cold, Lightning, Blood, Blunt, Piercing)`, `[1] Family`, `[2] flags`, `[3] native state` (`StateKind`, `0xFF` = none), `[4] trait id` (index into Traits: 0 Fury, 1 Composure, 2 Swiftness, 3 Wasting, 4 Crushing, 5 Keenness), `[5] condition` of the pairs generated from this base, `[6..8]` reserved.

**Traits** (6 x 32 B, index = trait id; all zero = absent): `[0] present (1)`, `[1]` reserved, `[2..8] hit op` (acts whenever the tint triggers), `[8..14]`, `[14..20]` template ops (the trait on the base's native state: generated pairs, the second tint of a triple), `[20..32]` reserved.

**Resonances** (36 x 16 B, index = `base * 6 + tint`, same as `deadroll_combat::resonance_id`):
`[0] condition` (0 Always, 1 CanBurn, 2 CanBeChilled, 3 CanBeFrozen, 4 IsFrozen, 5 CanBeShocked, 6 CanBleed, 7 CanBeStunned, 8 HasArmor),
`[1] flags` (bit 0: this record overrides the generated mechanics), `[2..8] op 0`, `[8..14] op 1`, `[14..16]` reserved.
Op (6 B): `[0] kind` (`deadroll_combat::tables::OpKind`, 0..=20; bit 7 = only on a crit), `[1] arg` (a `StateKind`, `ModKind` or `Aspect` by kind; `0xFF` = the base's native state, or any state for a Multiplier), `[2..6] value u32` (a scale in bp of the `Balance` curve the kind names, or a count).

**Bestiary** (32 x 48 B, index = bestiary id: 0 wolf, 1 goblin, 2 goblin chieftain, 3 goblin warrior, 4 goblin shaman; an all-zero record is an empty slot):

| Offset | Field | Type |
|---|---|---|
| 0 | present (1) | u8 |
| 1 | role: 0 Swarm, 1 Fast, 2 Tank, 3 Loner, 4 Boss, 5 Helper | u8 |
| 2 | biome: 0 Forest, 1 Caves and Ruins | u8 |
| 3 | size in cells per side (1 or 2) | u8 |
| 4 | minimum map tier (1..=14) | u8 |
| 5 | speed = action points per turn (>= 1) | u8 |
| 6 | natural attack aspect | u8 `Aspect` |
| 7 | attack source | u8 `Source` |
| 8 | attack form | u8 `Form` |
| 9 | predisposition aspect, `0xFF` = none | u8 |
| 10 | resistance mask, bit i = `Aspect` i | u8 |
| 11 | immunity mask, bit i = `Aspect` i | u8 |
| 12 | trait bits (swarm, regeneration, leader, ...) | u16 |
| 14 | armor | u16 |
| 16 | evasion | u16 |
| 18 | accuracy | u16 |
| 20 | initiative | u16 |
| 22 | hp at level 0 | u32 |
| 26 | hp per level | u32 |
| 30 | attack damage min (<= max) | u16 |
| 32 | attack damage max | u16 |
| 34 | xp reward | u32 |
| 38 | state immunity mask, bit i = `StateKind` i | u8 |
| 39..48 | reserved | |

**Tileset** (7 x 8 B in `Tile` order): `[0] passable (0/1)`, `[1] move cost in action points (>= 1 if passable)`, `[2] blocks line of sight (0/1)`, `[3] flags`, `[4..8]` reserved.

**Prefabs**: opaque bytes, 4 KB, length-checked only (P1).

## Build and test

```
cargo test -p deadroll-registry
anchor build --tools-version v1.54 -p deadroll_registry
anchor deploy -p deadroll_registry --provider.cluster localnet
cd tests && ANCHOR_PROVIDER_URL=http://127.0.0.1:8899 ANCHOR_WALLET=<keypair json> \
  node --import tsx --test --test-concurrency=1 registry.test.ts
```

`init_config` is first-caller-wins: run it right after the first deploy.
