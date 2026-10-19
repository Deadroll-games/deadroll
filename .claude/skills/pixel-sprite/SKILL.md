---
name: pixel-sprite
description: Use when drawing characters, monsters, items or tiles as sprite DSL files in assets/sprites. Sprites are geometry (rects, pixels, ellipses) with role slots, auto-shaded and outlined by crates/render, never hand-painted bitmaps.
---

# Pixel sprites from geometry

**Source of truth: `crates/render/FORMAT.md`** (text DSL, exact rasterization rules, binary format,
validator rules, crafted look). Design background: `docs/ART_DESCRIPTION.md` (its sizes are still
the 32 px ones; format 2 doubled them, the numbers below are the current ones). If this skill and
FORMAT.md ever disagree, FORMAT.md wins.
You write sprites as text in `assets/sprites/<name>.dsl`. `tools/art` compiles and renders them,
`art pack` builds the on-chain sprite book 2. Shading and outline are done by the rasterizer: you
only place shapes and slots.

## Workflow

1. Take your id range (below) and the current bodies in `assets/sprites/bodies.dsl` as the pose.
2. Write the `.dsl` file. One file holds several sprites. Touch only your own file.
3. Look at it:
   - `cargo run -p art -- assets/sprites/<name>.dsl --scale 8 --out target/art/<name>` (one PNG per sprite);
   - `cargo run -p art -- sheet characters` (bare bodies and every kit on both bodies, alive and dead);
   - `cargo run -p art -- sheet matrix assets/sprites/<name>.dsl` (each item: 3 materials,
     2 enchantments, 3 runes, empty and filled sockets, a debug slot view, on the body).
   All previews go to `target/`, never to `/tmp` or the disk root.
4. `cargo run -p art -- validate assets/sprites/<name>.dsl` must pass.
5. Fix until the checklist passes.

## File syntax

```
# comment
sprite <id> <name>        # id unique in the repo, name [a-z0-9_]{1,32}
size <w> <h>              # 32 64 96 128 (per layer type, below)
layer <type>              # names below
flags no_outline          # optional
<commands, one per line>
```
Keywords and command names are case-sensitive. Numbers are decimal. Tokens are separated by spaces.

## Commands

| Command | Meaning |
|---|---|
| `RECT x y w h S` | fills `x <= X < x+w`, `y <= Y < y+h` |
| `PIXEL x y S` | one pixel |
| `LINE x0 y0 x1 y1 S` | 1 px line, both ends included |
| `ELLIPSE x y w h S` | filled ellipse inside the **bounding rect** `x y w h` (not center and radius) |
| `RING x y w h S` | 1 px ellipse outline inside the bounding rect |
| `POLY S x1 y1 x2 y2 x3 y3 ...` | filled polygon, 3..8 vertices; vertices are pixel **corners**; slot comes **first** |
| `MIRROR_X a` | mirror everything drawn so far about a vertical axis, `a` doubled: `a = 64` between px 31 and 32, `a = 63` through px 31 |
| `MIRROR_Y a` | same, horizontal axis |
| `SHADE x y S shade` | one pixel with a fixed shade, 0 = darkest |
| `SOCKET x y` | marks a **4x4** socket square at `(x, y)`, draws nothing |

`S` in the table is the slot letter. Later commands overwrite earlier ones; slot `_` erases.

## Limits (validator, `FORMAT.md` 6)

- at most **48 commands**, at most **8 `SHADE`**, at most **4 `SOCKET`** per sprite;
- every shape 1 px from the canvas edge (the outline needs it), except `no_outline` layers;
- line slopes only 0, 90, 45 degrees, 1:2, 2:1;
- body layers use only `K H Y D R O _`, other layers `P S A E U O _`;
- socket squares 4x4, entirely inside `P` (not on `U`, `E`, `A`), not overlapping;
- rune zone `U` inside the `P` mask (every 4-neighbor is `P` or `U`), **at least 12 px** on a weapon;
- a `portrait` with `U` keeps **6 px** from every edge (three rune glow bands of 2 px);
- minimum detail **2 px** (exceptions: eye pupils and highlights, bowstrings, chains, `SHADE`).

If a shape does not fit in 48 commands it is too detailed: simplify, merge rects, use `MIRROR_X`.

## Slots, not colors

| Letter (items) | Letter (body) | Role | Shades |
|---|---|---|---|
| `_` | `_` | erase | |
| `O` | `O` | explicit outline pixel (visor slit, finger gap, seam) | 1 |
| `P` | `K` | primary material / skin | 4 |
| `S` | `H` | secondary material (handle, leather, straps) / hair, fur | 3 |
| `A` | `Y` | accent (trim, buckles, studs) / eyes | 3 |
| `E` | `D` | element core (staff head, blade center) / dark details (pupils, mouth, brows, nostrils) | 4 |
| `U` | `R` | rune zone (strip along the blade, inside `P`) / racial features | 4 |

Colors come from the palette at render time. Slot `D` always uses the palette's dark ramp.

## What to draw and what not to

- **Silhouette first** (`ART_DESCRIPTION.md` 12): the sprite must read when filled with one color.
  Details come after and never break the outline of the shape.
- Draw only **permanent details of the shape**: folds, trim, straps, buckles, seams, soles, teeth,
  claws, fur tufts, stone texture. Do **not** draw materials, enchantments, runes or gems: crafting
  gives them (`ART_DESCRIPTION.md` 1.6, 8). You only mark where they go:
  - `U`: rune zone, a strip at least 2 px wide along the main plate or blade (12+ px);
  - `E`: element core, where an enchantment glows (staff or wand head, blade center);
  - `SOCKET`: on a weapon at the guard and pommel (two-handed: also the blade near the guard);
    on a shield or armor at the center and corners of the main plate.
- Inside one slot, use `O` (1 px lines) for seams and finger gaps, `SHADE` for highlights
  (an eye highlight, a buckle glint), and a second slot border as a facet (`P` next to `S`).
- Faces at 64 px (head about 16x16): eye white = `Y` (the lightest shade via `SHADE ... Y 2`),
  pupil 2x2 or 1x2 in `D`, a 1 px highlight with `SHADE`, brows in `D` or `H`, nose as a 2 px step
  in `K` with a `SHADE`, mouth 4x1..2 in `D`. Keep both eyes on the same rows across races.
- Tiles: texture from shapes with meaning (bricks and mortar, planks and nails, wave crests),
  never random noise.

## Layer types and sizes

Character layers, bottom to top (the name is what you write after `layer`), all **64x64**:
`shadow`, `back_item`, `arm_back`, `glove_back`, `offhand`, `legs`, `pants`, `boots`, `torso`,
`armor_body`, `necklace`, `head`, `hair`, `helmet`, `arm_front`, `glove_front`, `mainhand`,
`overlay_wounds`, `overlay_element`.
Other: `tile` **64x64**, `monster` **64, 96 or 128** square (bigger ones stand bottom-center on
their map cell), `icon` **32x32**, `portrait` any allowed size (64x64; long items 32x96 or 64x128).
Must have `flags no_outline`: `shadow`, `necklace`, `overlay_wounds`, `overlay_element`, floor tiles.

## Sprite id ranges

| Ids | Content |
|---|---|
| 1-99 | bodies: races, shadow, arms, heads, hair |
| 100-199 | warrior kit |
| 200-299 | mage kit |
| 300-399 | support kit |
| 400-499 | monsters, tombstone |
| 500-599 | tiles |

0 is the empty layer. Never reuse an id or a name that already exists in `assets/sprites/`, and
never change the id or layer of an existing sprite (characters store them on-chain).

## Canvas, pose and anchors (64x64)

- 3/4 view facing right. Height about 44-48 px, **feet on y = 61**, 12 px headroom for helmets
  and raised weapons, body 20-24 px wide, head 16-18 px.
- Anchors, shared by every body and every kit: equipment must sit on both human and elf without
  shifting.

| Anchor | Point | Used by |
|---|---|---|
| `HAND_FRONT` | (42, 38) | main weapon grip, front glove |
| `HAND_BACK` | (20, 38) | shield, second weapon, back glove |
| `HEAD_CENTER` | (30, 20) | helmets, hoods |
| `NECK` | (30, 30) | necklace, pendant around y = 32-34 |
| `BACK` | (24, 30) | cloak, bag, weapon on the back |
| `WAIST` | (30, 44) | belt, top of the pants |
| `FEET` | (30, 61) | boots, shadow |

- Weapon length: one-handed 20-28 px, two-handed and staves 32-44 px. A weapon sticks out of the
  body silhouette and must not cover the face.
- Tiles: one 64x64 sprite per map cell.

## Auto shading (done by the rasterizer, light from the top left)

A neighbor is "own" if it has the same slot in the same sprite (`U` counts as `P` on items).
```
L,T,R,B = left/top/right/bottom neighbor is NOT own; n = shades of the slot
if (L||T) && !(R||B): shade = (L&&T) ? n-1 : min(2,n-1)
else if (R||B) && !(L||T): shade = 0
else: shade = 1
```
A border between two slots works as a facet: blade `P` next to hilt `S` gets a light and a dark edge.

## Outline (done by the rasterizer)

Every empty pixel with a filled 4-neighbor in the same sprite becomes outline `#120c16`, drawn over
lower layers and under the sprite's own fill.

## Crafted look (done by the rasterizer, `FORMAT.md` 8)

So you know what your marks turn into in the matrix:
- empty socket: the 4x4 square in material shade 0 with a 1 px frame in shade 1 around it;
  filled: the square in the gem ramp, lightest pixel top left;
- runes: the `U` zone split into segments, each pixel carved (shade 0) or glowing;
- each rune adds a 2 px glow band around the `P`/`U`/`E` mask: rune 1 at 1-2 px, rune 2 at
  3-4 px, rune 3 at 5-6 px; an enchantment recolors `P` and `U` and fills `E`.

## Example (64x64, valid DSL)

```
sprite 199 example_sword
size 64 64
layer mainhand
RECT 44 10 4 24 P
RECT 44 8 2 2 P
RECT 40 34 12 2 A
RECT 44 36 4 4 S
RECT 45 12 2 12 U
SOCKET 44 26
SHADE 44 10 P 3
```

The current bodies and kits in `assets/sprites/` are the reference: read `bodies.dsl` before
drawing anything worn.

## Ramps used in the demo palette

steel `#3a4250 #5c6676 #8a94a4 #c8d0dc`, skin `#7a4a32 #a86c4a #d09268 #ecbc92`,
robe `#1e1638 #2e2257 #43337f #5c48a8`, green `#14321e #1f4d2c #2f6e3e #4a9455`,
leather `#3a2416 #5e3c24 #86583a #a8764e`, gold `#5e3e0a #8a5e10 #d9a832 #f6e08a`,
cold `#1d3f63 #2a5f8a #4f8fc0 #cfe8ff`, fire `#5a1208 #a8300c #e8741c #ffd860`.
Full list: `assets/palette/palette_1.pal` (frozen on-chain as palette 1).

## Checklist

- [ ] `cargo run -p art -- validate <file>` passes
- [ ] Silhouette reads when filled with one color
- [ ] Weapon sticks out of the body silhouette and does not cover the face
- [ ] No detail thinner than 2 px except pupils, highlights, bowstrings, chains, seams in `O`
- [ ] Same anchors as the bodies; worn items checked on human and elf in `sheet characters`
- [ ] `U`, `E` and `SOCKET` only where crafting can put something; checked in `sheet matrix`
- [ ] Ids and layers unchanged for existing sprites, new ids inside your range
- [ ] Dead character: grayness comes from the `GRAY` render flag, no extra sprite
