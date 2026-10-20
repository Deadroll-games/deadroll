---
name: pixel-sprite
description: Use when drawing characters, monsters, items or tiles as sprite DSL files in assets/sprites. Sprites are drawn pixel by pixel (BITMAP) or from geometry; every pixel stores a role slot, never a color, and crates/render colors, shades and outlines it.
---

# Pixel sprites with role slots

**Source of truth: `crates/render/FORMAT.md`** (text DSL, exact rasterization rules, binary format,
validator rules, crafted look). Design background: `docs/ART_DESCRIPTION.md` (its sizes are still
the 32 px ones; the numbers below are the current ones: characters 96 px, tiles 64 px). If this skill and
FORMAT.md ever disagree, FORMAT.md wins.
You write sprites as text in `assets/sprites/<name>.dsl`. `tools/art` compiles and renders them,
`art pack` builds the on-chain sprite book 2. Each pixel keeps its slot, so materials, enchantments, runes,
sockets, death and curse work on it. Draw the look pixel by pixel with `BITMAP` and explicit
ramp colors where you want control; let the rasterizer shade (2.8), outline (colored, 2.9),
dither (2.10) and drop overlap shadows (2.11) where you do not.

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
| `BITMAP x y w h` + `h` rows | pixel by pixel, one character per pixel (legend below) |

After `layer`, `copy <name>` takes the commands of an earlier sprite of the same file (an elf
body is `copy human_body` plus its ear); identical sprites are stored once in a book.

`BITMAP` legend: `.` keep what is under, `_` erase, `O` outline pixel, a slot letter
(`P S A E U` / `K H Y D R`) auto shaded, or a slot with an explicit ramp color 0-5:
`0-5` P/K, `a-f` S/H, `g-l` A/Y, `m-r` E/D, `s-x` U/R. Color 0 is the outline shade of the
ramp, 1-4 the body of the form, 5 the highlight. Rows have exactly `w` characters.

`S` in the table is the slot letter. Later commands overwrite earlier ones; slot `_` erases.

## Limits (validator, `FORMAT.md` 6)

- at most **48 commands** (a `BITMAP` counts as one), at most **8 `SHADE`**, at most **4 `SOCKET`** per sprite;
- every shape 1 px from the canvas edge (the outline needs it), except `no_outline` layers;
- line slopes only 0, 90, 45 degrees, 1:2, 2:1;
- body layers use only `K H Y D R O _`, other layers `P S A E U O _`;
- socket squares 4x4, entirely inside `P` (not on `U`, `E`, `A`), not overlapping;
- rune zone `U` inside the `P` mask (every 4-neighbor is `P` or `U`), **at least 12 px** on a weapon;
- a `portrait` with `U` keeps **6 px** from every edge (three rune glow bands of 2 px);
- minimum detail **2 px** for geometry (exceptions: eye pupils and highlights, bowstrings, chains, `SHADE`);
  a `BITMAP` is drawn pixel by pixel, 1 px details are fine;
- `art validate` prints the bytes of every sprite: keep an eye on the book size (64 KiB per book).

## Slots, not colors

| Letter (items) | Letter (body) | Role | Auto levels |
|---|---|---|---|
| `_` | `_` | erase | |
| `O` | `O` | explicit outline pixel (visor slit, finger gap, seam, dark joints) | 1 |
| `P` | `K` | primary material / skin | 4 |
| `S` | `H` | secondary material (handle, leather, straps) / hair, fur | 3 |
| `A` | `Y` | accent (trim, buckles, studs) / eyes | 3 |
| `E` | `D` | element core (staff head, blade center) / dark details (pupils, mouth, brows, nostrils) | 4 |
| `U` | `R` | rune zone (strip along the blade, inside `P`) / racial features | 4 |

Every ramp has 6 colors (palette 2); explicit shades and `SHADE` take 0-5.

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
- Faces at 96 px (head about 13x14, `bodies.dsl`): eye white `l`/`k`, iris `j`, brows in hair
  `b`, nose a 1 px step out of the face edge with a lit top, mouth 2 px of `o`. Keep both eyes on
  the same rows across races.
- Thin lines inside a slot (grout, mortar, plate seams) are lit by auto shading: draw them as `O`
  or with explicit dark colors.
- Tiles: texture from shapes with meaning (bricks and mortar, planks and nails, wave crests),
  never random noise.

## Layer types and sizes

Character layers, bottom to top (the name is what you write after `layer`), all **96x96**:
`shadow`, `back_item`, `body`, `glove_back`, `pants`, `boots`, `armor_body`, `necklace`, `hair`,
`offhand`, `helmet`, `mainhand`, `fist_front`, `glove_front`, `overlay_element`.
The body of a race is ONE sprite (head, torso, arms, legs, briefs): separate body parts would get
outline seams at the shoulder, elbow, wrist and knee. `fist_front` is the front fist alone, pixel for
pixel the fist of the body, drawn over the weapon so the fingers wrap the grip (no outline). The
shield (`offhand`) is held in front of the torso on the back arm, over the armor, its near rim in
front; long hair falls behind it. No wound overlays and no torn items: only worn items change the image, plus death gray.
Other: `tile` **64x64** (one map cell), `monster` **96 or 128** square, `icon` **32x32**, `portrait`
any allowed size (64x64; long items 32x96 or 64x128). Characters and monsters stand bottom-center
on their cell at the tile's pixel size and overlap the row above.
Must have `flags no_outline`: `shadow`, `necklace`, `fist_front`, `overlay_element`, floor tiles.

## Sprite id ranges

| Ids | Content |
|---|---|
| 1-99 | shadow, race bodies, hair, the front fist |
| 100-199 | warrior kit |
| 200-299 | mage kit |
| 300-399 | support kit |
| 400-499 | monsters, tombstone |
| 500-599 | tiles |

0 is the empty layer. Never reuse an id or a name that already exists in `assets/sprites/`, and
never change the id or layer of an existing sprite (characters store them on-chain).

## Canvas, pose and anchors (96x96)

- 3/4 view facing right. Figure about 70-74 px from the hair to **feet on y = 91**, head about
  13x14, shoulders about 26 px wide, normal proportions.
- Every race has the same silhouette and anchors below the neck (an elf body is `copy` of the
  human one); races differ in head, ears, hair, face and skin ramp. Any item fits any race.
- Anatomy: shoulders slope into the neck (no deltoid ball), the waist narrows into the hips and the
  briefs sit on the hips, limbs taper smoothly (no bumps at elbows and knees, no horizontal stripes),
  hands slightly elongated, a little wider than the wrist, with a thumb.

| Anchor | Point | Used by |
|---|---|---|
| `HAND_FRONT` | (63, 62) | main weapon grip, front glove |
| `HAND_BACK` | (34, 63) | shield, second weapon, back glove |
| `HEAD_CENTER` | (48, 27) | helmets, hoods |
| `NECK` | (48, 35) | necklace, pendant around y = 38-40 |
| `BACK` | (44, 42) | cloak, bag, weapon on the back |
| `WAIST` | (48, 57) | belt, top of the pants |
| `FEET` | (48, 91) | boots, shadow |

- Weapon length: one-handed 26-34 px, two-handed and staves 44-60 px. A weapon sticks out of the
  body silhouette and must not cover the face.
- Tiles: one 64x64 sprite per map cell, same pixel size as the characters.

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

## Example (geometry, a 64x64 legacy canvas, valid DSL)

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

The current bodies and the warrior kit in `assets/sprites/` are the reference (BITMAP at 96 px):
read `bodies.dsl` before drawing anything worn. `art sheet scene` shows tiles, the probe chest
(`tools/art/probe/`) and warriors at one pixel scale.

## Palette

`assets/palette/palette_2.pal`: 6-color ramps, shadows cold, lights warm, color 0 is the ramp's
outline shade. Materials of cloth, leather, stone and plants carry `dither`. Indices of palette 1
never move: new ramps and materials are appended. Palette 1 (`palette_1.pal`, 4-color ramps) is
frozen for rasterizer 1.

## Checklist

- [ ] `cargo run -p art -- validate <file>` passes
- [ ] Silhouette reads when filled with one color
- [ ] Weapon sticks out of the body silhouette and does not cover the face
- [ ] No detail thinner than 2 px except pupils, highlights, bowstrings, chains, seams in `O`
- [ ] Same anchors as the bodies; worn items checked on human and elf in `sheet characters`
- [ ] `U`, `E` and `SOCKET` only where crafting can put something; checked in `sheet matrix`
- [ ] Ids and layers unchanged for existing sprites, new ids inside your range
- [ ] Dead character: grayness comes from the `GRAY` render flag, no extra sprite
