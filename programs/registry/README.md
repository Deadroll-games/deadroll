# programs/registry

`GameConfig` (trusted program ids, rasterizer pointer, NFT uri base) and the data tables the game
programs read without CPI (`docs/ARCHITECTURE.md` 4.2, 4.3). Owner lane: L4.

## Accounts

| Account | Seeds | Body |
|---|---|---|
| `GameConfig` | `["config"]` | `GameConfigData` from `crates/types` (frozen), then `sprite_books: [Pubkey; 4]` (patch 7, offset 428, lookup order, `Pubkey::default()` = empty); `8 + 420 + 128` bytes |
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
| `set_sprite_books()` | authority | 1..=4 `SpriteBook`s (kind 3) as remaining accounts, each pulled like above, distinct; stored in order, other slots cleared |
| `set_rasterizer_from_render()` | render `["authority"]` PDA (CPI from `render::finalize_rasterizer`) | the `set_rasterizer` pull; `STORAGE_AND_DELIVERY.md` 3.2 |
| `resize_config()` | authority | grows a pre-patch-7 `GameConfig` (`8 + 420`) to the current size, tops up rent; refused once done |
| `init_table(kind)` | authority | creates an empty table |
| `write_table(kind, offset, bytes)` | authority | whole, aligned, valid records only; tables stay editable |

`TableKind`: 0 Aspects, 1 Resonances, 2 Bestiary, 3 Tileset, 4 Prefabs, 5 Traits, 6 MonsterSkills, 7 Rewards (append only).

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
| 39 | skill pool start, index into MonsterSkills | u8 |
| 40 | skill pool length (<= 16, the bits of `Spawn.skills`) | u8 |
| 41 | kin, `0xFF` = none (0 Wolf, 1 Goblin, 2 Bear, 3 Treant, 4 Fairy, 5 Troll; names in `crates/rules/src/registry.rs`) | u8 |
| 42 | resisted tags (patch 9), bit 0 Magical, 1 Combat (power sources), 2 Strike, 3 Projectile (forms) | u8 |
| 43 | immune tags, the same bits | u8 |
| 44 | body loot table (patch 11), a Rewards id, `0xFF` = none | u8 |
| 45..48 | reserved | |

The registry is the only source of monster numbers: the session copies the record at `open_session`, the simulator and the website decode the same bytes (`deadroll_rules::MonsterTemplate::from_record`).

**MonsterSkills** (128 x 32 B, index = skill id, names in `assets/registry/monster_skills.json`; an all-zero record is an empty slot): `[0] present (1)`, `[1..13] SkillBlocks` (borsh, as a character skill), `[13..16] SkillExt` (range, timing, HP threshold), `[16] flags` (bit 0: a heal or buff reaches allies of the bearer's kin only), `[17..32]` reserved. `write_table` runs `deadroll_skillcheck::validate_monster_skill`: the player builder rules without class limits, Heal allowed as an active or reactive skill on one unit (never passive or an aura).

**Tileset** (7 x 8 B in `Tile` order): `[0] passable (0/1)`, `[1] move cost in action points (>= 1 if passable)`, `[2] blocks line of sight (0/1)`, `[3] flags`, `[4..8]` reserved.

**Prefabs**: opaque bytes, 4 KB, length-checked only (P1).

**Rewards** (patch 11, 32 x 108 B, index = table id, names in `assets/registry/reward_tables.json`; an all-zero record is an empty id): `[0] present (1)`, `[1] level_base (>= 1)`, `[2] level_max (base..base+3)`, `[3]` reserved, `[4..6] cost` in budget points (u16), `[6..8]` reserved, then 4 levels of 25 B: `[count 1..=6 for a used level, 0 otherwise]`, 6 entries `(kind, id u16, qty >= 1)` (`RewardKind`: 0 Resource, 1 Recipe, 2 Consumable, 3 Blank, 4 Reagent), unused entries zero. Level `level_base + i` is block `i`. `write_table` runs `deadroll_rules::loot::LootTable::from_record`, the decoder the session uses.

## Build and test

```
cargo test -p deadroll-registry
anchor build --tools-version v1.54 -p deadroll_registry
anchor deploy -p deadroll_registry --provider.cluster localnet
cd tests && ANCHOR_PROVIDER_URL=http://127.0.0.1:8899 ANCHOR_WALLET=<keypair json> \
  node --import tsx --test --test-concurrency=1 registry.test.ts
```

`init_config` is first-caller-wins: run it right after the first deploy.
