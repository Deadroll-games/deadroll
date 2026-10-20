# MAP_DESCRIPTION: Deadroll maps

Version 0.1. A document about maps: sizes, layers, tiles and prefabs, secrets, visibility, builder rules for the DM, on-chain storage and rendering.

Related documents:

- `ART_DESCRIPTION.md`: sprite geometry, rasterizer, palette. Tiles and prefabs are drawn by the same rules.
- `STORAGE_AND_DELIVERY.md`: where accounts live and how clients load them.
- `GAME_COMMISSIONS.md`: session keys, the rule "game turns do not create accounts".

---

## 1. Principles

1. **A map stores the meaning of its cells, not a picture.** One tile type per cell, a list of objects, points, and the Merkle root of the secrets. The picture is assembled by the rasterizer.
2. **Only ready-made parts.** The DM builds a map from a limited set of tiles and prefabs (mountains, rocks, trees, bushes...), drawn in advance and stored on chain. Custom images cannot be uploaded into a map.
3. **The program checks the rules.** Limits, passability, and the path from entrance to exit are checked in the program when the map is created. The builder on the website uses the same code via WASM.
4. **The public part is open, secrets are under a Merkle root.** Terrain, objects, and monsters are visible. Traps and hidden stashes are stored only as the tree root and are revealed cell by cell with a proof.
5. **The map is a template, the party is state.** `Map` is created by the DM once and used in one party. Everything live (positions, explored and revealed cells, monster HP) lives in `GameSession`.

---

## 2. Map sizes

### 2.1 D&D reference points

- Cell = 5 feet. A character's normal speed of 30 feet = **6 cells per turn**.
- Combat maps for VTT are usually from 10×10 to 50×50 cells, the Roll20 default page is 25×25.
- Published maps of single locations are often around 30×20 cells, large whole dungeons reach ~100×100.

### 2.2 Why not a 500-cell path

At 6 cells per turn, a path of 500 cells is 80+ turns of walking alone, without fights. For one party on a stream this is too long. A typical dungeon for one session: the path from the entrance to the boss is 60-150 cells, that is 10-25 turns of movement plus combats.

### 2.3 Deadroll sizes

| Size        | Cells  | Used for                                  |
| ----------- | ------ | ----------------------------------------- |
| **S** 16×16 | 256    | one or two rooms, a short quest, **MVP**  |
| **M** 24×24 | 576    | a small dungeon for one session           |
| **L** 32×32 | 1024   | a large dungeon, a long session           |

Width and height may differ (for example, 32×16 for a corridor), but each side is from 8 to 32. The storage format allows up to 64 per side, in case larger maps are needed later.

The brief mentions 10×10 for the MVP: that is one room, which a character crosses in 2 turns. 16×16 shows exploration better, and costs ~256 bytes in storage.

---

## 3. Map layers

| Layer    | What                                              | Visibility  | Where stored                                         |
| -------- | ------------------------------------------------- | ----------- | ---------------------------------------------------- |
| Terrain  | floor, walls, water, doors, stairs                | public      | `Map.cells`, 1 byte per cell                         |
| Objects  | trees, rocks, bushes, mountains, columns, barrels | public      | `Map.objects`, list of prefabs                       |
| Points   | entrance, exit, visible chests                    | public      | `Map.points`                                         |
| Monsters | placement from the Bestiary                       | public      | `Map.spawns`, live state in `GameSession`            |
| Secrets  | traps, hidden stashes, secret doors               | **hidden**  | only `Map.secrets_root` + declared counts            |

---

## 4. Terrain

### 4.1 Cell byte

```
bit 7-3: tile type (0-31)
bit 2-0: variant (0-7)
```

### 4.2 Tile types (starter set)

| id    | Tile                              | Passable | Blocks vision                | Step cost                |
| ----- | --------------------------------- | -------- | ---------------------------- | ------------------------ |
| 0     | void (outside the dungeon)        | no       | yes                          |                          |
| 1     | stone floor                       | yes      | no                           | 1                        |
| 2     | wooden floor                      | yes      | no                           | 1                        |
| 3     | dirt, grass                       | yes      | no                           | 1                        |
| 4     | wall                              | no       | yes                          |                          |
| 5     | door                              | yes      | yes (closed) / no (open)     | 1                        |
| 6     | shallow water                     | yes      | no                           | 2 (difficult terrain)    |
| 7     | deep water                        | no       | no                           |                          |
| 8     | stairs down / exit                | yes      | no                           | 1                        |
| 9     | rubble                            | yes      | no                           | 2                        |
| 10    | chasm                             | no       | no                           |                          |
| 11-31 | reserved                          |          |                              |                          |

Tile properties (passability, vision, cost) live in the on-chain `Tileset` table, not in the map.

### 4.3 What is computed and not stored

- **Wall autotiling.** Which piece of wall to draw (corner, end, straight, T-junction) is determined from the 4 neighbors: a 4-bit mask → 16 wall sprite variants. The DM just places "wall".
- **Floor details.** Cracks, pebbles, and moss are chosen by a PRNG seeded with `SHA-256(map address ‖ cell number)`. The floor does not look like repeating tiles.
- **Transitions.** The edge of water and the edge of grass are drawn from the neighbors, the same way as walls.

The variant (bits 2-0) is what the DM chooses deliberately: a floor with a drain grate, a wall with a torch, a cracked wall.

---

## 5. Tileset

- A tile is drawn on a **64×64** canvas, with the same pixel size as characters and monsters, so that a character stands on a cell 1:1 (a character layer is 96×96 and stands on the bottom and the center of its cell).
- The same geometry commands, slots, shading, and palette as in `ART_DESCRIPTION.md`.
- **Seamlessness:** the edges of floor tiles must not give away the seam. The outline of floor tiles is disabled (`no_outline`), and shapes at the edges either end flat or continue symmetrically. Seams are 1 px along the edge (2 px where two tiles meet).
- **Walls** are drawn in 16 autotiling variants. For the MVP, 6 is acceptable (straight horizontal, straight vertical, 4 corners), the rest map to the nearest one.
- Walls in the 3/4 view have a visible "front" face at the bottom, so that the map reads the same way as the characters.

---

## 6. Prefabs

### 6.1 The `PrefabBook` account

One per game. Each prefab:

| Field           | Example                                                         |
| --------------- | --------------------------------------------------------------- |
| `id`            | 17                                                              |
| `category`      | tree / rock / bush / mountain / column / furniture / decor      |
| `footprint`     | 1×1, 2×2, 3×2 (base cells)                                      |
| `passable`      | mask over the footprint cells: passable or not                  |
| `blocks_vision` | mask over the footprint cells: blocks vision or not             |
| `move_cost`     | step cost over passable cells (bush = 2)                        |
| `sprite_id`     | geometry sprite                                                 |
| `variants`      | number of variants (pine, spruce, dead tree)                    |

### 6.2 Prefab canvas

- Width = `footprint.w × 32`.
- Height = `footprint.h × 32` + up to **32 px of headroom on top**: the tree crown, the mountain peak, and the top of a column extend onto the cell above the footprint. This gives the 3/4 view.
- Example: a 1×1 tree is drawn on a 32×64 canvas, a 3×2 mountain on 96×96.

### 6.3 Starter set

| Category  | Prefabs                  | Footprint | Passable        | Vision       |
| --------- | ------------------------ | --------- | --------------- | ------------ |
| Tree      | pine, deciduous, dead    | 1×1       | no              | blocks       |
| Bush      | common, thorny           | 1×1       | yes, cost 2     | does not block |
| Rock      | small                    | 1×1       | yes, cost 2     | does not block |
| Rock      | boulder                  | 2×2       | no              | blocks       |
| Mountain  | cliff                    | 3×2       | no              | blocks       |
| Column    | stone, ruined            | 1×1       | no              | blocks       |
| Furniture | table, barrel, crate     | 1×1       | no              | does not block |
| Decor     | bones, pool of blood, candle | 1×1   | yes             | does not block |

### 6.4 Storage in the map

`Map.objects`: a list of `(prefab_id u8, x u8, y u8, variant u8)` = 4 bytes per object. `(x, y)` is the top-left cell of the footprint.

Variety without storage: the rasterizer slightly varies a prefab by PRNG (seed = map address + object index): foliage tint within the ramp, mirroring, small details. 30 identical pines do not look like copies.

---

## 7. Points and monsters

### 7.1 Points

`Map.points`: a list of `(type u8, x u8, y u8, data u8)`.

| Type          | Required                | `data`          |
| ------------- | ----------------------- | --------------- |
| entrance      | exactly 1               |                 |
| exit          | exactly 1               |                 |
| visible chest | per map tier limits     | loot table id   |

### 7.2 Monsters

`Map.spawns`: a list of `(bestiary_id u8, x u8, y u8, level u8, skills u16, flags u8, aspects u8, pack u8)`.

- `bestiary_id`: a template from the Bestiary (`BESTIARY.md`).
- `level`: the monster's level, within the map tier and not below the template's minimum tier.
- `skills`: a bitmask of the skills the DM chose from the template's pool; the number chosen is exactly min(slots, skills of that category in the pool) (`BESTIARY.md`, section 3.2).
- `flags`: the elite version (`BESTIARY.md`, section 5) and other flags.
- `aspects`: the chosen elemental aspects for templates where the DM chooses them (Wood Fairy, Wood Fairy King).
- `pack`: the pack number. Monsters with the same number enter combat together (`BESTIARY.md`, section 2.1).
- A boss is a spawn with a template marked as a boss in the Bestiary; its pack is the boss pack.

Checks at `finalize_map`:

- each pack has at most one monster with the "Loner" role;
- pack size and number of packs are within map tier limits;
- monsters of one pack stand close together (within the pack radius).

When a party is opened, spawns are copied into `GameSession.monsters` with full HP.

---

## 8. Secrets

### 8.1 What can be a secret

| Type         | Contents                                      |
| ------------ | --------------------------------------------- |
| no secret    |                                               |
| trap         | trap type, detection difficulty, damage       |
| hidden stash | loot table id                                 |
| secret door  | turns a wall into a door when revealed        |

### 8.2 Merkle tree

- **Every cell of the map has a leaf**, including empty ones. This way the DM can prove both "there is a trap here" and "there is nothing here", but cannot lie in either direction.
- Leaf: `SHA-256(map address ‖ cell number u16 ‖ contents ‖ salt 16 bytes)`. Each cell has its own salt, otherwise empty cells could be guessed by brute force.
- The number of leaves is padded with empty leaves to a power of two.
- Depth: 8 for an S map, 10 for L. Proof: 8-10 hashes = 256-320 bytes.
- `Map` stores only the root (32 bytes). The tree and salts are known only to the DM client. **The DM must save the map's secrets file**: without it, cells cannot be revealed.

### 8.3 Declared counts

The program does not see the secrets, so when creating a map the DM publicly declares **how many** secrets of each type are on the map. The counts are checked against map tier limits (section 10). During reveals the program counts the secrets found and rejects a reveal beyond the declared count.

Players see: "the map has 3 traps and 1 hidden stash", but do not know where.

### 8.4 When a cell is revealed

| Trigger                                                          | What is revealed                                    |
| ---------------------------------------------------------------- | --------------------------------------------------- |
| a character stepped on the cell                                  | that cell; a trap triggers at the moment of reveal  |
| a successful detection skill (`search`, a roll against difficulty) | all cells in the skill's area                     |
| interaction with an object (search a barrel, examine a wall)     | that cell                                           |

### 8.5 DM obligations

- The trigger records in `GameSession` an **obligation** to reveal a list of cells.
- The DM client sends the reveals automatically: contents, salt, proof. About 3 cells fit in one transaction.
- Until the obligations are fulfilled, **the party's next turn does not go through**. The DM cannot "forget" to reveal a trap.
- What is revealed is recorded in `GameSession` (section 11) and becomes visible to the party.
- Penalties for delaying a reveal: in phase 1, via the DM stake.

---

## 9. Visibility

- **Vision radius** is 3 cells by default, changed by race, skills, and light.
- **Line of sight:** a cell is visible if the Bresenham line from the character to it does not pass through cells that block vision (walls, trees, boulders, closed doors).
- **Explored:** cells that have ever been in the vision of anyone in the party are marked in `GameSession.explored` and are shown muted from then on.

**Important:** terrain, objects, and monsters lie on chain in the open, anyone can read them. The vision radius for the public layers is an **interface** mechanic, not cryptography. Only the secrets under the Merkle root are truly hidden. A real fog of war for terrain (with its own commitment) is planned for phase 3.

---

## 10. Builder rules

Checked by the program in the `create_map` instruction. The builder on the website calls the same code from the `deadroll-mapcheck` crate via WASM and shows errors before the transaction is sent.

### 10.1 Structure

- [ ] Each side is from 8 to 32
- [ ] Exactly one entrance and one exit, both on passable cells
- [ ] **There is a path from the entrance to the exit** (breadth-first search over passable cells, taking objects into account)
- [ ] Every visible chest and every spawn is reachable from the entrance
- [ ] Monsters, chests, the entrance, and the exit stand on passable cells without objects
- [ ] Objects do not overlap and do not extend beyond the map
- [ ] Only prefabs from `PrefabBook`, variant within `variants`
- [ ] Only tiles from `Tileset`

### 10.2 Limits by map tier

The numbers are starter values, tuned at balancing. Object limits depend on map size.

| Parameter                  | Map tier 1 (levels 1-2)  | Map tier 2 (levels 3-4)  |
| -------------------------- | ------------------------ | ------------------------ |
| Monsters (without boss)    | up to 4                  | up to 8                  |
| Bosses                     | 1                        | 1                        |
| Visible chests             | 1                        | 2                        |
| Hidden stashes (declared)  | 1                        | 2                        |
| Traps (declared)           | 1-2                      | up to 4                  |
| Trees, boulders, mountains | up to 1 per 8 map cells  | up to 1 per 8 map cells  |
| Mountains                  | up to 2                  | up to 3                  |
| Decor                      | up to 1 per 16 map cells | up to 1 per 16 map cells |

### 10.3 Compute

Breadth-first search over 1024 cells fits within the compute limit of a single transaction, but an L map is better created in two steps: `init_map` (allocate the account, write the grid in chunks) and `finalize_map` (rule check; after it the map is immutable).

---

## 11. Party state on the map

The part of `GameSession` that relates to the map:

| Field                                         | Size for L (32×32)   |
| --------------------------------------------- | -------------------- |
| character positions                           | 2 bytes × up to 4    |
| `monsters`: template, HP, position, skills    | ~8 bytes × up to 8   |
| `explored`: bitmask                           | 128 bytes            |
| `revealed`: bitmask of revealed cells         | 128 bytes            |
| revealed secrets: (cell, type, data)          | ~4 bytes × up to 8   |
| opened doors: bitmask over doors              | a few bytes          |
| DM obligations: list of cells                 | up to 9 × 2 bytes    |
| whose turn, turn number                       | a few bytes          |

In total, on the order of 400-500 bytes for an L map. The account is created in full size when the party is opened, turns only update it.

---

## 12. Map rendering

The rasterizer (`ART_DESCRIPTION.md`) gets the function `render_map(map, session, viewer, mode)`.

Rendering order:

1. Terrain: tiles with autotiling and floor details.
2. Flat objects (decor, puddles) and markers of revealed secrets.
3. **Objects, characters, and monsters, sorted by bottom cell (y, then x).** This way a character behind a tree is partly covered by the crown, and a character in front of a tree covers the trunk.
4. Effects (rune glow, particles).
5. Darkening: cells outside the `viewer`'s vision darken, unexplored ones are almost black.

Image size: 64 px per cell. An S map = 1024×1024, L = 2048×2048. Units reach up above their cell. For the NFT card of a map (phase 3), a reduced version or nearest-neighbor upscaling is used, by the same rules as for characters.

---

## 13. `Map` account format

```
Header:
  u8   version
  u8   width, height
  u8   tier
  u16  tileset_version
  u16  prefabbook_version
  32   dm (author)
  u8   status (draft / final)

Data:
  [u8; w*h]          cells
  u8 + 4n            objects
  u8 + 4n            points
  u8 + 4n            spawns
  [u8; N_TYPES]      declared secret counts
  32                 secrets_root
```

---

## 14. Storage budget

| What                                     | Size      | Deposit (~6960 lamports per byte) |
| ---------------------------------------- | --------- | --------------------------------- |
| S map (16×16)                            | ~450 bytes | ~0.003 SOL                       |
| M map (24×24)                            | ~800 bytes | ~0.006 SOL                       |
| L map (32×32)                            | ~1.3 KB   | ~0.009 SOL                        |
| `Tileset` (32 tiles + 16 wall variants)  | ~3-4 KB   | ~0.03 SOL                         |
| `PrefabBook` (30 prefabs)                | ~2-3 KB   | ~0.02 SOL                         |
| The `GameSession` part for an L map      | ~500 bytes | ~0.004 SOL                       |

---

## 15. Who does what

| Role         | What it does                                                                                           |
| ------------ | ------------------------------------------------------------------------------------------------------ |
| AI artist    | draws tiles and prefabs by the rules of `ART_DESCRIPTION.md` + sections 5-6 of this document          |
| Game team    | uploads `Tileset` and `PrefabBook`, sets the map tier limits                                           |
| DM           | assembles the map in the builder from ready-made parts, places monsters and secrets, keeps the secrets file |
| Program      | checks the rules, stores the map, verifies reveals                                                     |
| Rasterizer   | draws the map from the data                                                                            |

---

## 16. Implementation checklist

- [ ] `Tileset` table with tile properties
- [ ] `PrefabBook` with the starter set of prefabs
- [ ] The `deadroll-mapcheck` crate: all checks of section 10, compiles into both the program and WASM
- [ ] Instructions `init_map`, `write_map_chunk`, `finalize_map`
- [ ] DM client: salt generation, Merkle tree, export and import of the secrets file
- [ ] Reveal instruction with proof verification and counting against the declared counts
- [ ] DM obligations block the next turn
- [ ] `render_map` with autotiling, y-sorting, and darkening
- [ ] Test: the DM cannot reveal a cell with contents different from the committed ones
- [ ] Test: the DM cannot prove "empty" for a cell with a trap
- [ ] Test: a map without a path from the entrance to the exit fails `finalize_map`
