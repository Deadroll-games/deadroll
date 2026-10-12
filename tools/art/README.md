# tools/art

CLI for sprite authors. Everything goes through `crates/render`, there is no second rasterizer.

```
cargo run -p art -- validate                       # every assets/sprites/*.dsl
cargo run -p art -- validate assets/sprites/tiles.dsl
cargo run -p art -- assets/sprites/monsters.dsl --scale 8 --out target/art      # one PNG per sprite
cargo run -p art -- assets/sprites/monsters.dsl --id 402 --out chief.png
cargo run -p art -- sheet characters --scale 8 --out target/art/characters.png  # full characters
cargo run -p art -- sheet contact --scale 4 --out target/art/all.png            # every sprite
```

Options: `--palette <file.pal>` (default `assets/palette/palette_1.pal`), `--material <name>`,
`--bg #rrggbb` (background under transparent pixels; sheets default to gray).

The validator implements `crates/render/FORMAT.md` section 6.
