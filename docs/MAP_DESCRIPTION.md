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
4. **The public part is open, secrets are under a Merkle root.** Terrain, objects, and the one monster each pack shows are visible. Traps, hidden stashes, secret doors and the composition of packs are stored only as the tree root and are revealed one by one with a proof (sections 8, 17.5).
5. **The map is a template, the party is state.** `Map` is created by the DM once and used in one party. Everything live (positions, explored and revealed cells, monster HP) lives in `GameSession`. **Planned (section 17.7):** the map is one-use and closes by itself after the rewards are issued; its deposit returns to the DM.

---

## 2. Map sizes

### 2.1 D&D reference points

- Cell = 5 feet. A character's normal speed of 30 feet = **6 cells per turn**.
- Combat maps for VTT are usually from 10×10 to 50×50 cells, the Roll20 default page is 25×25.
- Published maps of single locations are often around 30×20 cells, large whole dungeons reach ~100×100.

### 2.2 Why not a 500-cell path

At 6 cells per turn, a path of 500 cells is 80+ turns of walking alone, without fights. For one party on a stream this is too long. A typical dungeon for one session: the path from the entrance to the boss is 60-150 cells, that is 10-25 turns of movement plus combats.

### 2.3 Deadroll sizes

The DM picks a **size class**; the class follows from the map's area (section 17.6):

| Class       | Most cells | About square | Used for                               |
| ----------- | ---------- | ------------ | -------------------------------------- |
| **Small**   | 1024       | 32×32        | one or two rooms, a short quest        |
| **Medium**  | 5184       | 72×72        | a dungeon for one session              |
| **Big**     | 10201      | 101×101      | a large dungeon, a long session        |
| **Dungeon** | 20449      | 143×143      | a whole dungeon                        |

Width and height may differ (for example, 128×8 is a Small corridor), but each side is from 8 to 143 and the area within the largest class. A map's class is the first one whose cells hold its area.

The brief mentions 10×10 for the MVP: that is one room, which a character crosses in 2 turns. 16×16 shows exploration better.

---

## 3. Map layers

| Layer    | What                                              | Visibility  | Where stored                                         |
| -------- | ------------------------------------------------- | ----------- | ---------------------------------------------------- |
| Terrain  | floor, walls, water, doors, stairs                | public      | `Map.cells`, 1 byte per cell                         |
| Objects  | trees, rocks, bushes, mountains, columns, barrels | public      | `Map.objects`, list of prefabs                       |
| Points   | entrance, exit, visible chests                    | public      | `Map.points`                                         |
| Monsters | placement from the Bestiary                       | public (planned: one per pack, section 17.2) | `Map.spawns`, live state in `GameSession` |
| Secrets  | traps, hidden stashes, secret doors               | **hidden**  | only `Map.secrets_root` + declared counts            |

---

## 4. Terrain

### 4.1 Cell byte

```
bit 7-3: tile type (0-31)
bit 2-0: variant (0-7)
```

The variant is the DM's pick and only chooses the picture of the tile in the map's biome (section 4.4). No rule reads it. A variant the biome has no picture for is drawn as variant 0.

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
| 11-31 | reserved (ids are not used)       |          |                              |                          |

Tile properties (passability, vision, cost) live in the on-chain `Tileset` table, not in the map. A `Tileset` record is 8 bytes: passable, step cost, blocks vision, flags (bit 0 = door), 4 reserved bytes. The record index is the tile type id. The variant (bits 2-0 of the cell byte) is not in the `Tileset`: it is picture only (section 4.1).

### 4.3 What is computed and not stored

- **Wall autotiling.** Which piece of wall to draw (corner, end, straight, T-junction) is determined from the 4 neighbors: a 4-bit mask, bit 0 north, bit 1 east, bit 2 south, bit 3 west. Walls and doors join, the map edge and void do not. The mask picks one of the biome's **16 wall shapes**. The DM just places "wall". Autotiling is computed by the site, the map stores only "wall".
- **Transitions.** The edge of water and the edge of grass are drawn from the neighbors, the same way as walls.

The variant (bits 2-0) is what the DM chooses deliberately: a floor with a drain grate, a wall with a torch, a cracked wall. There is no random choice of details.

In the builder the brush is a **tile type plus a variant**, shown with pictures from the chain rasterizer. A stroke overwrites the whole cell (type and variant).

### 4.4 Biome

The **biome** is the art set the map is drawn with. The DM picks it at `create_map`; it is stored in `Map.biome` (u8).

| id | Biome           |
| -- | --------------- |
| 0  | Caves and Ruins |
| 1  | Forest          |
| 2  | Crypt           |

- Picture only: no rule reads the biome, and it does not limit which monsters a map may hold (`BESTIARY.md`, section 11).
- The battle grid, the builder, the entrance picture and the loading screen follow the biome.
- A map created before the field (`Map` version 1) reads as Caves and Ruins.
- **Templates.** The builder offers one 16×16 template map per biome (`assets/registry/maps/`). They live on the site only and are never uploaded: the DM loads one, edits it and publishes it through the usual `create_map` ... `finalize_map`.

---

## 5. Tileset

- A tile is drawn on a **64×64** canvas, with the same pixel size as characters and monsters, so that a character stands on a cell 1:1 (a character layer is 96×96 and stands on the bottom and the center of its cell).
- The same geometry commands, slots, shading, and palette as in `ART_DESCRIPTION.md`.
- **Seamlessness:** the edges of floor tiles must not give away the seam. The outline of floor tiles is disabled (`no_outline`), and shapes at the edges either end flat or continue symmetrically. Seams are 1 px along the edge (2 px where two tiles meet).
- **Walls** are drawn in 16 autotiling shapes per biome, one for each value of the mask (section 4.3).
- Tiles of each biome have their own sprites and variants (sprite ids: `ART_DESCRIPTION.md`, section 6.5).
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

| Type (`PointKind`) | Required                | `data`          |
| ------------------ | ----------------------- | --------------- |
| chest (0)          | per map tier limits     | loot table id   |
| entrance (1)       | exactly 1               |                 |
| exit (2)           | exactly 1               |                 |

Points are stored in the order the DM wrote them. The number of a chest in a party is its place among the chests (in point order).

Besides the points, the map holds the settings the DM gives it at creation:

- the **reward table of the boss** (at least 2 levels, `PARTY_AND_TURNS.md`, section 9.4);
- the **minimum number of players** of the map: the party does not start with fewer, and the reward budget is checked for it (section 10.2);
- the **chest radius** (1 to 10 cells): a chest cannot be opened while a living monster is within this distance of the chest or of any living player (`PARTY_AND_TURNS.md`, section 9.3.1);
- the **issue deadline** (10 minutes to 10 hours) for the rewards after the party ends (`PARTY_AND_TURNS.md`, section 2, step 10).

A closed chest occupies its cell; an open chest is passable. "A chest is reachable" means that a cell adjacent to it is reachable.

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
| ambush trap  | the hidden pack it starts a fight with (section 17.4) |
| hidden stash | loot table id                                 |
| secret door  | turns a wall into a door when revealed        |

A trap deals a fixed damage the DM sets, within the map tier's limit (`BalanceConfig`); it comes off HP as it is, without dice and resistances. A trap and an ambush trap lie on a cell that can be entered and is not the entrance, the exit, a chest or a pack's cell. An ambush trap and a hidden stash lie on the map, not in an arena. A secret door lies in a wall. A found hidden stash opens like a chest; its table counts in the map's reward budget.

### 8.2 Merkle tree

- **Every cell of the map and of every arena has a leaf**, including empty ones. This way the DM can prove both "there is a trap here" and "there is nothing here", but cannot lie in either direction. Each pack's composition has a leaf too.
- The leaves, in order: one per pack number (8), the map's cells row by row, then each used pack's arena cells row by row.
- Cell leaf: `SHA-256(0x00 ‖ map address ‖ leaf number u16 ‖ contents 4 bytes ‖ salt 16 bytes)`. Pack leaf: `SHA-256(0x02 ‖ map address ‖ leaf number u16 ‖ composition ‖ salt 16 bytes)`. Node: `SHA-256(0x01 ‖ left ‖ right)`. Each leaf has its own salt, otherwise empty cells could be guessed by brute force.
- The number of leaves is padded with empty hashes to a power of two; a padding index is never a leaf.
- Depth: up to 15 (a 143×143 Dungeon with 40 arenas of 12×12). Proof: up to 15 hashes = 480 bytes. A map has one pack leaf per pack number its size class stores (8 on a Small map).
- `Map` stores only the root (32 bytes). There is no secrets file: the secrets lie on chain, encrypted (section 17.5).

### 8.3 Declared counts

The program does not see the secrets, so when creating a map the DM publicly declares **how many** secrets of each type are on the map. The counts are checked against map tier limits (section 10). During reveals the program counts the secrets found and rejects a reveal beyond the declared count.

Players see: "the map has 3 traps and 1 hidden stash", but do not know where.

### 8.4 When a cell is revealed

| Trigger                                                          | What is revealed                                    |
| ---------------------------------------------------------------- | --------------------------------------------------- |
| a character stepped on the cell                                  | that cell; a trap triggers at the moment of reveal  |
| a successful detection skill (`search`, a roll against difficulty) | all cells in the skill's area (**later**: there is no detection skill yet) |
| interaction with an object (search a barrel, examine a wall)     | that cell: "Examine" a cell next to the character, for action points |
| the party engages a pack                                         | the pack's composition (section 17.2)               |

### 8.5 DM obligations

- The trigger records in `GameSession` an **obligation** to reveal a list of cells.
- The DM client sends the reveals automatically with the DM's session key: contents, salt, proof. One or two cells fit in one transaction.
- Until the obligations are fulfilled, **the party's next turn does not go through**. The DM cannot "forget" to reveal a trap.
- What is revealed is recorded in `GameSession` (section 11) and becomes visible to the party.
- Penalties for delaying a reveal: in phase 1, via the DM stake.

---

## 9. Visibility

- **Vision radius** is 3 cells by default, changed by race, skills, and light.
- **Line of sight:** a cell is visible if the Bresenham line from the character to it does not pass through cells that block vision (walls, trees, boulders, closed doors).
- **Explored:** cells that have ever been in the vision of anyone in the party are marked in `GameSession.explored` and are shown muted from then on.

**Important:** terrain, objects, and monsters lie on chain in the open, anyone can read them. The vision radius for the public layers is an **interface** mechanic, not cryptography. Only the secrets under the Merkle root are truly hidden. Terrain stays open: the fog of war covers only secrets and, after the arena update, the composition of packs (section 17.5).

---

## 10. Builder rules

Checked by the program in the `finalize_map` instruction (not in `create_map`). The builder on the website calls the same code from the `deadroll-mapcheck` crate via WASM and shows errors before the transaction is sent.

### 10.1 Structure

- [ ] Each side is from 8 to 143, the area within the map's size class (section 17.6)
- [ ] Exactly one entrance and one exit, both on passable cells
- [ ] **There is a path from the entrance to the exit** (reachability over the 8 neighbours of passable cells, without cutting corners, taking objects into account)
- [ ] Every visible chest and every spawn is reachable from the entrance
- [ ] Monsters, chests, the entrance, and the exit stand on passable cells without objects
- [ ] Objects do not overlap and do not extend beyond the map
- [ ] Only prefabs from `PrefabBook`, variant within `variants`
- [ ] Only tiles from `Tileset`

### 10.2 Limits by map tier

The numbers are starter values, tuned at balancing. Object limits depend on map size. The reward budget is counted for the map's **minimum number of players**. The table gives a Small map's limits; a bigger size class multiplies them (section 17.6).

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

A map is written in several steps: `create_map` (allocate the accounts; a layout past 10 KB grows with `grow_layout` in the same transaction), `write_layout` (the grid in chunks), `write_spawns`, `write_points`, `write_arena`, `write_secrets`, and `finalize_map` (rule check; after it the map is immutable).

The reachability check of a big map does not fit one transaction: it runs in steps (`check_step`), each within the compute limit, and keeps where it stopped in the layout. `finalize_map` does the last part and the rules; a Small map needs no step. Any write to the layout starts the check over. The website counts the steps a map needs with the same code and sends them before `finalize_map`, in the same wallet confirmation.

---

## 11. Party state on the map

The part of `GameSession` that relates to the map:

| Field                                         | Size                 |
| --------------------------------------------- | -------------------- |
| character positions                           | 2 bytes × up to 4    |
| `monsters`: template, HP, position, skills    | ~8 bytes × up to 8   |
| `explored`: bitmask                           | 128 bytes            |
| `revealed`: bitmask over every leaf           | ~3.3 KB (`SessionBoard`) |
| revealed secrets: (leaf, contents)            | 6 bytes × up to 96 (`SessionBoard`) |
| DM obligations: list of leaves                | up to 16 × 2 bytes   |
| revealed packs' monsters                      | 9 bytes × up to 80 (`SessionBoard`) |
| whose turn, turn number                       | a few bytes          |

The accounts are created in full size, for the largest map, when the party is opened; turns only update them. The revealed leaves, the found secrets and the revealed monsters live in a separate account of the party, `SessionBoard`, so that `GameSession` stays small.

---

## 12. Map rendering

The rasterizer (`ART_DESCRIPTION.md`) gets the function `render_map(map, session, viewer, mode)`.

**Now:** there is no `render_map` yet. The site draws the map itself from tile, monster and character pictures made by the chain rasterizer, by the map's biome and each cell's variant, with wall autotiling (section 4.3). Monster sprites face left and the site mirrors them toward their target (`ART_DESCRIPTION.md`, section 6.1).

Rendering order:

1. Terrain: tiles of the map's biome, by variant, with autotiling.
2. Flat objects (decor, puddles) and markers of revealed secrets.
3. **Objects, characters, and monsters, sorted by bottom cell (y, then x).** This way a character behind a tree is partly covered by the crown, and a character in front of a tree covers the trunk.
4. Effects (rune glow, particles).
5. Darkening: cells outside the `viewer`'s vision darken, unexplored ones are almost black.

Image size: 64 px per cell. A 16×16 map = 1024×1024, 32×32 = 2048×2048; past that the site draws only the part of the map on screen, with the whole map on a parchment minimap beside it. Units reach up above their cell. For the NFT card of a map (phase 3), a reduced version or nearest-neighbor upscaling is used, by the same rules as for characters.

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

This is the target format. In the current build the cells, spawns and points live in a separate `MapLayout` account (`["layout", map]`), and the `Map` account holds the header, the tier, the map settings of section 7.1 (minimum players, chest radius, issue deadline), the layout hash and `secrets_root`.

`MapLayout` is at **version 5**: as big as its size class and sides need. After the header (width, height, size class): each pack's visible monster, the points, the packs, their threats, the declared secret counts, the arenas, then the cells (`width` per row), the DM's encrypted copy of the secrets and the reachability check's scratch. A class stores 8, 16, 24 or 40 pack numbers and 4, 8, 12 or 16 visible chests. The layout hash covers everything before the encrypted copy. A version 4 map cannot open a party.

`Map` is at **version 2** (121 bytes): `biome: u8` (section 4.4) is appended at the end. A version 1 map (120 bytes) reads with the missing byte as zero (Caves and Ruins), so a finalized old map still opens a party; a version 1 draft cannot be continued.

---

## 14. Storage budget

| What                                     | Size      | Deposit (~6960 lamports per byte) |
| ---------------------------------------- | --------- | --------------------------------- |
| Small map (32×32), with the `Map`        | ~3.8 KB   | ~0.028 SOL                        |
| Medium map (72×72)                       | ~11.9 KB  | ~0.084 SOL                        |
| Big map (101×101)                        | ~19.9 KB  | ~0.140 SOL                        |
| Dungeon map (143×143)                    | ~37.6 KB  | ~0.263 SOL                        |
| `Tileset` (32 tiles + 16 wall variants)  | ~3-4 KB   | ~0.03 SOL                         |
| `PrefabBook` (30 prefabs)                | ~2-3 KB   | ~0.02 SOL                         |
| A party's `SessionBoard`                 | ~4.6 KB   | ~0.033 SOL                        |

A map is played once: after the rewards its deposits go back to the DM (section 17.7).

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

- [x] `Tileset` table with tile properties (a registry table)
- [x] `PrefabBook` with the starter set of prefabs (a registry table)
- [x] The `deadroll-mapcheck` crate: all checks of section 10, compiles into both the program and WASM
- [x] Instructions `create_map`, `write_layout`, `write_spawns`, `write_points`, `finalize_map`
- [x] DM client: salts, Merkle tree, the secrets encrypted on chain (section 17.5)
- [x] Reveal instruction with proof verification and counting against the declared counts
- [x] DM obligations block the next turn
- [x] Biome (`Map.biome`), variant brush and biome templates in the builder
- [x] Wall autotiling and drawing by biome and variant on the site
- [ ] `render_map` with autotiling, y-sorting, and darkening in the rasterizer
- [x] Test: the DM cannot reveal a cell with contents different from the committed ones
- [x] Test: the DM cannot prove "empty" for a cell with a trap
- [x] Test: a map without a path from the entrance to the exit fails `finalize_map`

---

## 17. Planned: map and arena

**Planned, not implemented.** This section is the design of the arena update. Until the patch, sections 1-16 describe how maps work now. Turns, combat and outcomes of the update are in `PARTY_AND_TURNS.md`, section 4.1.

### 17.1 Two modes

- **Map** (outside combat): the party walks the map in turns in a circle, as now. Monsters stand still; the DM moves monsters only in combat.
- **Arena** (combat): each pack has its own small arena. One fight is one pack; the map waits while the arena is played.

### 17.2 Packs on the map

- On the map only **one monster of each pack** is visible. The DM picks which one.
- The site shows the pack's **threat level**. The DM declares the threat openly when building the map; the program checks it when the pack is revealed, and a mismatch rejects the reveal.
- The threat is the sum of the pack's monster levels: an elite's level counts 2 times, a boss's 3 times (`BalanceConfig`).
- A pack with no monster on the map is an **ambush pack**: only its ambush trap starts its fight (section 17.4).
- A pack has a **facing** (the direction it looks), set by the DM. It decides who moves first in the arena (`PARTY_AND_TURNS.md`, section 4.1).

### 17.3 Arenas

- The DM draws an **arena for every pack** in the builder, with the same tiles, objects and biome rules as the map.
- The DM picks the arena size within limits from `BalanceConfig`.
- An arena may hold its own traps and secrets (section 17.5).
- A boss may occupy 2×2 cells in the arena.

### 17.4 Ambush trap

A trap to which the DM links a **hidden pack** with its arena. When it triggers, the whole party goes into that arena and the monsters move first. The party comes in at the edge the pack's facing names. Each ambush pack has exactly one ambush trap.

### 17.5 Fog of war and secrets

- Hidden: the **composition of packs** and the **secrets** (traps, hidden stashes, secret doors), under the Merkle root (section 8). Terrain stays open.
- Until found, a secret looks like an ordinary floor or wall. After it is found it is highlighted.
- **There is no secrets file.** The secrets lie on chain, encrypted with a key derived from a signature of the DM's wallet. The DM's client decrypts them and sends the reveals by itself, with the DM's session key, without separate signatures (like the players' moves, `GAME_COMMISSIONS.md`, section 7).
- The key: the wallet signs a fixed message (an Ed25519 signature is the same every time); HKDF-SHA256 of the signature with the map address gives the AES-256-GCM key of the encrypted copy and the key that derives each leaf's salt. Nothing is stored: any device with the wallet opens the secrets.
- **The DM must be online** during the party.
- The DM's final audit before payment stays (`PARTY_AND_TURNS.md`, section 3.2).
- **Later:** fog of war through a MagicBlock Private Ephemeral Rollup (TEE).

### 17.6 Map sizes

The DM picks the size; sizes are set by area (section 2.3).

| Size        | Most cells | Side, about | Limits    |
| ----------- | ---------- | ----------- | --------- |
| **Small**   | 1024       | 32×32       | ×1 (base) |
| **Medium**  | 5184       | 72×72       | ×3        |
| **Big**     | 10201      | 101×101     | ×5        |
| **Dungeon** | 20449      | 143×143     | ×8        |

The Small limits are the base (section 10.2). A bigger size multiplies the packs, the regular monsters, the elites, the visible chests, the reward budget, the traps with the ambush traps, and the declared ambush traps, hidden stashes and secret doors. Each of them has its own multiplier in `BalanceConfig` (Ivan's decision: any one can be switched off with ×1 at every size). The multipliers raise the maximums only: the minimum number of traps stays the Small one. The boss stays one per map. A limit never goes past what the size class stores.

### 17.7 One-use map

A map is played once. When the rewards are issued (the DM's settlement, or the last player's claim after the DM's deadline) it closes by itself with its layout, and both deposits return to the DM. A party that never started does not play the map.

### 17.8 Later

- A boss calling allies during combat: a reactive skill with a limit of uses, for some bosses, in a separate skills update (`BESTIARY.md`, section 6).
