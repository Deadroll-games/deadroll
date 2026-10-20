# tools/art

CLI for sprite authors. Everything goes through `crates/render`, there is no second rasterizer.

```
cargo run -p art -- validate                       # every assets/sprites/*.dsl, bytes per sprite
cargo run -p art -- validate assets/sprites/tiles.dsl
cargo run -p art -- assets/sprites/monsters.dsl --scale 4 --out target/art      # one PNG per sprite
cargo run -p art -- assets/sprites/monsters.dsl --id 402 --out chief.png
cargo run -p art -- sheet characters --scale 4 --out target/art/characters.png  # full characters
cargo run -p art -- sheet contact --scale 2 --out target/art/all.png            # every sprite
cargo run -p art -- sheet matrix assets/sprites/warrior_kit.dsl --id 106       # one item: materials x crafts
cargo run -p art -- sheet scene --scale 4                # tiles, trap, probe chest, warriors at one pixel scale
cargo run -p art -- sheet join old.png new.png --out cmp.png   # PNGs side by side
cargo run -p art -- pack                           # target/render-assets/palette.bin + book_<id>.bin (books 2.., 64 KiB each)
cargo run -p art -- upscale old.dsl                # format 1 source (16..64 px) -> format 2, in place
```

Options: `--palette <file.pal>` (default `assets/palette/palette_2.pal`, 6-color ramps), `--material <name>`,
`--bg #rrggbb` (background under transparent pixels; sheets default to gray).

Preview colors: tiles use the palette materials `tile_wall`, `tile_floor`, `tile_water`, `tile_door`; the support kit uses `leather`.

The validator implements `crates/render/FORMAT.md` section 6.

Canvases are format 2 (`crates/render/FORMAT.md` 1.4): character layers 96x96, tiles 64x64, icons
32x32, monsters 96 or 128 square. Sprites are drawn pixel by pixel with `BITMAP` (FORMAT.md 1.2.1) or
with geometry; `copy <name>` reuses an earlier sprite, and `pack` stores identical sprites once. `upscale` doubles every coordinate (PIXEL becomes a 2x2 RECT, a
diagonal LINE two parallel lines, a SHADE its 2x2 block) and prints per sprite how many cells
differ from the old sprite enlarged x2. Previews are RGBA PNGs (sheets can have more than 256
colors); the on-chain encoder is the indexed one.
