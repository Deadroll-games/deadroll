---
name: pixel-sprite
description: Use when drawing characters, monsters, items or tiles as sprite DSL files in assets/sprites. Sprites are geometry (rects, pixels, ellipses) with role slots, auto-shaded and outlined by crates/render, never hand-painted bitmaps.
---

# Pixel sprites from geometry

**Source of truth: `crates/render/FORMAT.md`** (text DSL, exact rasterization rules, binary format).
Design background: `docs/ART_DESCRIPTION.md`. If this skill and FORMAT.md ever disagree, FORMAT.md wins.
You write sprites as text in `assets/sprites/<name>.dsl`. `tools/art` compiles and renders them,
`tools/scripts/upload-sprites.ts` packs them into the on-chain `SpriteBook`. Shading and outline are
done by the rasterizer: you only place shapes and slots.

## Workflow

1. Take your id range (below) and the reference layers at the end as the base pose.
2. Write the `.dsl` file. One file may hold several sprites.
3. `cargo run -p art -- assets/sprites/<name>.dsl --scale 8 --out <scratch>/<name>.png`, look at the PNG.
4. `cargo run -p art -- validate assets/sprites/<name>.dsl` must pass.
5. Fix until the checklist passes. Touch only your own `.dsl` file.

## File syntax

```
# comment
sprite <id> <name>        # id unique in the repo, name [a-z0-9_]{1,32}
size <w> <h>              # 16 32 48 64
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
| `MIRROR_X a` | mirror everything drawn so far about a vertical axis, `a` doubled: `a = 32` between px 15 and 16, `a = 31` through px 15 |
| `MIRROR_Y a` | same, horizontal axis |
| `SHADE x y S shade` | one pixel with a fixed shade, 0 = darkest (max 4 per sprite) |
| `SOCKET x y` | marks a 2x2 socket square at `(x, y)`, draws nothing (max 4) |

`S` in the table is the slot letter. Later commands overwrite earlier ones; slot `_` erases.
Keep every shape at least 1 px from the canvas edge (the outline needs it), except `no_outline` layers.
At most 24 commands per sprite. Line slopes only 0, 90, 45 degrees, 1:2, 2:1.

## Slots, not colors

| Letter (items) | Letter (body) | Role | Shades |
|---|---|---|---|
| `_` | `_` | erase | |
| `O` | `O` | explicit outline pixel (visor slit, finger gap) | 1 |
| `P` | `K` | primary material / skin | 4 |
| `S` | `H` | secondary material (handle, leather) / hair, fur | 3 |
| `A` | `Y` | accent (trim, gems) / eyes | 3 |
| `E` | `D` | element core (staff head, blade center) / dark details (mouth, brows, scars) | 4 |
| `U` | `R` | rune zone (1 px strip along the blade, inside `P`) / racial features | 4 |

Body layers use only `K H Y D R O _`. Colors come from the palette at render time.

## Layer types and sizes

Character layers, bottom to top (the name is what you write after `layer`):
`shadow`, `back_item`, `arm_back`, `glove_back`, `offhand`, `legs`, `pants`, `boots`, `torso`,
`armor_body`, `necklace`, `head`, `hair`, `helmet`, `arm_front`, `glove_front`, `mainhand`,
`overlay_wounds`, `overlay_element`. All 32x32.
Other: `tile` (32x32), `monster` (32, 48 or 64 square), `icon` (16x16), `portrait`.
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

0 is the empty layer. Never reuse an id or a name that already exists in `assets/sprites/`.

## Canvas and pose

- Character and monster layers: 3/4 view facing right, feet on y = 30.
- Anchors: main hand (21,19), off hand (10,19), head center (15,10), neck (15,15), back (12,15),
  waist (15,22), feet (15,30).
- Tiles: one 32x32 sprite per map cell.

## Auto shading (done by the rasterizer, light from the top left)

A neighbor is "own" if it has the same slot in the same sprite (`U` counts as `P`).
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

## Reference layers (human warrior, 32x32, valid DSL)

```
sprite 1 shadow
size 32 32
layer shadow
flags no_outline
RECT 10 31 12 1 P

sprite 2 human_arm_back
size 32 32
layer arm_back
RECT 9 16 2 3 K
RECT 9 19 3 2 K

sprite 3 human_legs
size 32 32
layer legs
RECT 12 23 3 8 K
RECT 16 23 3 8 K

sprite 4 human_torso
size 32 32
layer torso
RECT 12 15 7 8 K
RECT 14 14 3 1 K

sprite 5 human_head
size 32 32
layer head
RECT 12 6 7 8 K
PIXEL 14 10 Y
PIXEL 17 10 Y
PIXEL 16 12 D
PIXEL 17 12 D

sprite 6 human_hair
size 32 32
layer hair
RECT 11 7 2 6 H
RECT 12 5 7 2 H

sprite 7 human_arm_front
size 32 32
layer arm_front
RECT 19 16 2 3 K
RECT 20 19 3 2 K

sprite 100 warrior_cape
size 32 32
layer back_item
RECT 10 16 6 11 P
RECT 9 19 1 8 P

sprite 101 warrior_shield
size 32 32
layer offhand
RECT 4 15 5 1 P
RECT 3 16 7 6 P
RECT 4 22 5 1 P
RECT 5 23 3 1 P
PIXEL 6 24 P
RECT 5 18 3 3 A

sprite 102 warrior_pants
size 32 32
layer pants
RECT 12 23 7 2 P
RECT 12 25 3 3 P
RECT 16 25 3 3 P

sprite 103 warrior_boots
size 32 32
layer boots
RECT 12 28 3 3 P
RECT 16 28 3 3 P
RECT 19 29 1 2 P
RECT 12 28 3 1 S
RECT 16 28 3 1 S

sprite 104 warrior_armor
size 32 32
layer armor_body
RECT 11 15 9 7 P
RECT 18 14 3 3 P
RECT 10 14 2 3 P
RECT 11 22 9 1 S
PIXEL 15 22 A

sprite 105 warrior_helmet
size 32 32
layer helmet
RECT 13 3 5 1 P
RECT 12 4 7 1 P
RECT 11 5 9 3 P
RECT 11 8 9 1 A
RECT 11 9 2 3 P

sprite 106 warrior_sword
size 32 32
layer mainhand
RECT 22 5 2 12 P
PIXEL 22 4 P
RECT 20 17 6 1 A
```

## Ramps used in the demo palette

steel `#3a4250 #5c6676 #8a94a4 #c8d0dc`, skin `#7a4a32 #a86c4a #d09268 #ecbc92`,
robe `#1e1638 #2e2257 #43337f #5c48a8`, green `#14321e #1f4d2c #2f6e3e #4a9455`,
leather `#3a2416 #5e3c24 #86583a #a8764e`, gold `#8a5e10 #d9a832 #f6e08a`,
cold `#1d3f63 #2a5f8a #4f8fc0 #cfe8ff`, fire `#5a1208 #a8300c #e8741c #ffd860`.
Slot `D` always uses the palette's dark ramp.

## Checklist

- [ ] `cargo run -p art -- validate <file>` passes
- [ ] Silhouette reads when filled with one color
- [ ] Weapon sticks out of the body silhouette
- [ ] No detail thinner than 2 px except eyes, bowstrings, chains, `SHADE` highlights
- [ ] Same anchors as the reference body
- [ ] Ids inside your range, no duplicates
- [ ] Dead character: tombstone sprite (400-499); grayness comes from the `GRAY` render flag
