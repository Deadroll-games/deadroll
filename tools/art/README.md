# tools/art

CLI for sprite authors. Everything goes through `crates/render`, there is no second rasterizer.

```
cargo run -p art -- validate                       # every .dsl under assets/sprites, site_sprites, sprites_pending; bytes per sprite
cargo run -p art -- validate assets/sprites/tiles/tiles.dsl
cargo run -p art -- assets/sprites/monsters/monsters.dsl --scale 4 --out target/art      # one PNG per sprite
cargo run -p art -- assets/sprites/monsters/monsters.dsl --id 402 --out chief.png
cargo run -p art -- sheet characters --scale 4 --out target/art/characters.png  # full characters
cargo run -p art -- sheet contact --scale 2 --out target/art/all.png            # every sprite
cargo run -p art -- sheet matrix assets/sprites/items/items.dsl --id 100             # one item: materials x crafts
cargo run -p art -- sheet scene --scale 4                # tiles, trap, probe chest, warriors at one pixel scale
cargo run -p art -- sheet join old.png new.png --out cmp.png   # PNGs side by side
cargo run -p art -- pack [--first-book N]          # only assets/sprites/**: target/render-assets/palette.bin + book_<id>.bin (books N.., default 2, 64 KiB each)
cargo run -p art -- upscale old.dsl                # format 1 source (16..64 px) -> format 2, in place
```

Options: `--palette <file.pal>` (default `assets/palette/palette_2.pal`, 6-color ramps), `--material <name>`,
`--bg #rrggbb` (background under transparent pixels; sheets default to gray).

Preview colors: tiles use the palette materials `tile_wall`, `tile_floor`, `tile_water`, `tile_door`; items without their own material use the starter look (sword steel, robe robe, the rest leather).

The validator implements `crates/render/FORMAT.md` section 6.

Canvases are format 2 (`crates/render/FORMAT.md` 1.4): character layers 96x96, tiles 64x64, icons
32x32, monsters 96 or 128 square. Sprites are drawn pixel by pixel with `BITMAP` (FORMAT.md 1.2.1) or
with geometry; `copy <name>` reuses an earlier sprite, and `pack` stores identical sprites once. `upscale` doubles every coordinate (PIXEL becomes a 2x2 RECT, a
diagonal LINE two parallel lines, a SHADE its 2x2 block) and prints per sprite how many cells
differ from the old sprite enlarged x2. Previews are RGBA PNGs (sheets can have more than 256
colors); the on-chain encoder is the indexed one.

## Sprite folders

| Folder | Ids | Packed into books |
|---|---|---|
| `assets/sprites/bodies/` | 1-99 | yes |
| `assets/sprites/items/` | 100-399 (`100 + ItemBase`) | yes |
| `assets/sprites/monsters/` | 400-499 | yes |
| `assets/sprites/tiles/<biome>/`, `assets/sprites/points/` | 500-599, 800-999 | yes |
| `assets/sprites/resources/` | 600-799 (icons 32x32) | yes |
| `assets/sprites_pending/` | 1000+ (items without an `ItemBase` yet) | no |
| `assets/site_sprites/<category>/` | 1000+ (city, interiors, menu icons, benches, screens) | no, rendered to PNG in `app/assets/` |

Books are filled in id order, so moving a file between folders does not change the books. Ids and
names are unique across all three roots.

## Sculpted sources (`tools/art/sculpt`)

Monsters, tiles and points are generated: `node tools/art/sculpt/sculpt.js tools/art/sculpt/<model>.js > assets/sprites/<folder>/<file>.dsl`.
A model builds the sprite from primitives (ellipsoids, capsules, polygons, smooth masses) with light from the top left
and writes a BITMAP of explicit ramp colors. Edit the model, not the generated `.dsl`. Colors per sprite name are in
`MONSTER_LOOKS` and `TILE_LOOKS` (`src/preview.rs`) and must match the app (`app/src/chain/appearance.ts`).

## Tile ids by biome

| Ids | Content |
|---|---|
| 500, 501, 502, 503 | Caves: floor, wall (east and west neighbors), shallow water, door: the ids the current map uses |
| 507-521 | Caves: the other 15 wall shapes, id `507 + mask` (`506 + mask` above mask 10) |
| 522-537 | Caves: floors 1-3, planks 0-2, dirt 0-2, shallow water 1, deep water, stairs, rubble 0-1, chasm, void |
| 540-574 | Forest, fixed block (`sculpt/biome.js`): +0..15 walls by mask, +16..19 floor, +20..22 planks, +23..25 ground, +26 door, +27..28 shallow water, +29 deep, +30 stairs, +31..32 rubble, +33 chasm, +34 void |
| 800-834 | Crypt, the same block |
| 504, 505, 580-583 | Points: Caves entrance, exit portal (all biomes), chest closed and open, Forest and Crypt entrances |

Wall mask: bit 0 north, 1 east, 2 south, 3 west neighbor is a wall (or a door). Walls have no outline: a connected
side runs to the cell edge, an open side gets its own rim; with the south open the cell shows the wall's front face.
