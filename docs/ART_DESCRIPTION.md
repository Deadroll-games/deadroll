# ART_DESCRIPTION: Deadroll art rendering

Version 0.5. This document covers **how the art of the game is drawn**: sprite geometry, the rasterizer, shading, layers, effects, the output image and the work of the AI artist.

Where the data lives, how the rasterizer is loaded and how the image reaches wallets is described in `STORAGE_AND_DELIVERY.md`.

Everything described here defines the on-chain data format. Changing the standard after the first sprites ship is expensive, so the rules are stated strictly.

---

## 1. Core principles

1. **The source of truth is the role data.** Every sprite is a list of commands: geometry (`RECT`, `LINE`, `ELLIPSE`...) or a `BITMAP` of pixels. A pixel stores a role slot, never a color. The commands are stored, not the image.
2. **Commands define shape and role, not color.** The AI artist places shapes and specifies a role slot (primary material, hilt, accent). Shadows, highlights, outline and colors are computed by the rasterizer.
3. **Rendering is deterministic.** One universal function turns data into pixels. The same data always gives the same image, on any device.
4. **The image is assembled from layers.** A character is a stack of layers (body, armor, weapon...). A change of state (a new helmet, sharpening) changes a few bytes, and the image reassembles itself.
5. **Effects are computed from shape, not from coordinates.** Glow, frost and sparks work the same way on a sword, a bow, a shield and a staff.
6. **The AI draws only base shapes.** "Two-handed sword", "round shield", "staff". What a specific item becomes (onyx or steel, darkness or cold enchantment, runes, sockets) is decided by **crafting**: the player adds materials, the program writes the result into the item state, and the rasterizer draws it dynamically.

---

## 2. Rasterizer

### 2.1 Requirements

- **Rust, `no_std` + `alloc`, integer arithmetic only.** No `f32`/`f64`: floating point can give different results on different platforms.
- **Pure function:** the input is data bytes and the asset address, the output is pixels. No time, no external randomness, only a seeded PRNG (section 10).
- **No external crates for PNG and compression.** The PNG encoder is our own and minimal (section 13), so that the binary stays small.
- **One function for the whole game.** Differences between a sword, a bow and a character live in the data, not in the code.
- **Versioning.** Rendering rules are frozen in a version. Any change of the rules is a new rasterizer version, not a silent fix.
- **Build:** crate `deadroll-render`, compiled to WASM, `wasm-opt -Oz`. Size target 30-60 KB.

### 2.2 Interface

```rust
render(
    sprites: &[u8],      // sprites of the needed layers (format 4.5)
    palette: &[u8],      // master palette and material table
    state: &[u8],        // character or item state
    asset: [u8; 32],     // asset address, PRNG seed
    mode: OutputMode,    // RAW_RGBA | PNG
) -> Vec<u8>
```

- `RAW_RGBA`: canvas pixels as is, for a canvas on the website.
- `PNG`: a finished indexed PNG, enlarged by nearest neighbor by the integer scale `512 / max(w, h)` (64 gives x8, 96 gives x5, 128 gives x4, 32 gives x16).

### 2.3 Pipeline

```
1. For each layer:
   a. run the geometry commands in order → slot grid
   b. auto-shading → index grid (indices 0-15)
   c. outline → outline pixels around the layer
2. Stack the layers in a fixed order (section 6.4)
3. Substitute palette ramps by materials and elements → colors
4. Apply procedural effects (glow, frost, sparks...)
5. Output: RAW_RGBA or PNG (section 13)
```

---

## 3. Canvases and sizes

| What                                                 | Canvas          |
| ---------------------------------------------------- | --------------- |
| Character layer (body, armor, weapon in hand...)     | 96×96           |
| Tile                                                 | 64×64           |
| Monster                                              | 96×96 or 128×128 |
| Item icon in the inventory                           | 32×32           |
| Item portrait for the NFT card: shield, helmet, amulet | 64×64         |
| Portrait of a long item: sword, staff, bow, spear    | 32×96 or 64×128 |

Width and height are stored in the sprite header. Allowed values for a side: 32, 64, 96, 128.

Characters and monsters stand on the bottom and the center of their cell and overlap the row above it.

An item has three sprites: **in hand** (a simplified silhouette inside the character canvas), **portrait** (detailed, on its own canvas) and **icon** 32×32. Materials, elements and effects apply to all of them the same way.

Format 2 changed the canvas sizes (section 3). Sprites of format 1 are drawn only by the rasterizer of version 1.

---

## 4. Geometry: the command language

### 4.1 Coordinate system

- Integer coordinates, origin `(0, 0)` in the top left corner, `x` to the right, `y` down.
- Pixel `(x, y)` is the square from `x` to `x+1`, its center is at `(x+0.5, y+0.5)`.
- Internally the rasterizer counts in **doubled coordinates** (pixel center = `2x+1`) so that everything is an integer.

### 4.2 Slots

Every command carries a **slot** (4 bits). A slot means the role of the pixel, not a color.

| Slot  | Items, armor, clothing                    | Body                                             |
| ----- | ----------------------------------------- | ------------------------------------------------ |
| 0 `_` | **erase** (makes pixels transparent)      | erase                                            |
| 1 `O` | explicit outline pixel (exception, 4.6)   | explicit outline                                 |
| 2     | `P` primary material                      | `K` skin                                         |
| 3     | `S` secondary material                    | `H` hair, fur                                    |
| 4     | `A` accent                                | `Y` eyes                                         |
| 5     | `E` element / effect core                 | `D` dark details: mouth, brows, scars            |
| 6     | `U` rune zone (section 9.4)               | `R` racial features: horns, scales, tattoos      |

For overlays (blood, frost, soot) a single slot `P` = the overlay color is used.

### 4.3 Commands

| Opcode | Command    | Arguments          | What it does                                                                                                  |
| ------ | ---------- | ------------------ | ------------------------------------------------------------------------------------------------------------- |
| 0x1    | `RECT`     | x y w h            | fills the pixels `[x, x+w) × [y, y+h)`                                                                        |
| 0x2    | `PIXEL`    | x y                | a single pixel                                                                                                |
| 0x3    | `LINE`     | x0 y0 x1 y1        | a line 1 px thick, both ends included                                                                         |
| 0x4    | `ELLIPSE`  | x y w h            | a filled ellipse inscribed in the rectangle                                                                   |
| 0x5    | `RING`     | x y w h            | a 1 px thick ellipse outline (arcs, rings, bows)                                                              |
| 0x6    | `POLY`     | n, then n pairs x y | a filled polygon, 3-8 vertices                                                                               |
| 0x7    | `MIRROR_X` | axis ×2            | mirrors everything drawn in the layer about a vertical axis                                                   |
| 0x8    | `MIRROR_Y` | axis ×2            | mirrors about a horizontal axis                                                                               |
| 0x9    | `SHADE`    | x y shade          | a pixel with an explicit shade, bypassing auto-shading                                                        |
| 0xA    | `SOCKET`   | x y                | a socket point: draws no pixels, marks a place where crafting can create a socket (up to 4 per sprite, section 9.4) |
| 0xB    | `BITMAP`   | x y w h, then pixels | a block of pixels, each one a slot, an explicit color or a skip (section 4.4)                              |

**BITMAP text syntax.** The command is followed by `h` rows of `w` characters:

- `.` skip, `_` erase, `O` outline;
- a slot letter is auto shaded;
- `0-5` (P/K), `a-f` (S/H), `g-l` (A/Y), `m-r` (E/D), `s-x` (U/R) are that slot with an explicit ramp color 0-5.

`copy <name>` after `layer` reuses the commands of an earlier sprite.

### 4.4 Exact rasterization rules

These rules are mandatory for implementations: they are what makes all clients draw the same thing.

- **Order:** commands run strictly one after another. A later command overwrites an earlier one (painter's algorithm). Slot 0 erases.
- **RECT:** pixels with `x ≤ X < x+w` and `y ≤ Y < y+h`.
- **LINE:** Bresenham's algorithm from `(x0,y0)` to `(x1,y1)`, both ends included. Then **pixel-perfect cleanup**: if points `i-1` and `i+1` are diagonal neighbors and point `i` touches both orthogonally, point `i` is removed. This way diagonals do not get "steps" 2 px thick.
- **ELLIPSE:** in doubled coordinates the center is `cx = 2x + w`, `cy = 2y + h`. Pixel `(X, Y)` is filled if `(2X+1-cx)² · h² + (2Y+1-cy)² · w² ≤ w² · h²`. Works for even and odd sizes.
- **RING:** the pixels that fall inside the `ELLIPSE` with the same arguments and have at least one 4-neighbor outside it.
- **POLY:** a pixel is filled if its center is inside the polygon by the even-odd rule, checked in doubled coordinates. A center exactly on an edge is filled if the edge is a left or a top edge (top-left rule).
- **MIRROR_X a:** for every non-empty pixel `(X, Y)` of the layer, the same pixel is written at `(a - 1 - X, Y)`. The axis is in doubled coordinates: on a 64 px canvas `a = 64` mirrors between pixels 31 and 32, `a = 63` through the center of pixel 31; on a 96 px canvas `a = 96` mirrors between pixels 47 and 48. The source does not change.
- **MIRROR_Y:** the same vertically.
- **SHADE:** the pixel gets the slot from the nibble and an explicit ramp color 0-5. Auto-shading does not touch it.
- **BITMAP:** pixel `i` of the block goes to `(x + i mod w, y + i div w)`. Each pixel is a slot, an explicit color or a skip. The pixels are packed with RLE. One `BITMAP` is one command. An explicit color is not auto shaded (like `SHADE`).

### 4.5 Sprite binary format

```
Header (6 bytes):
  u8  version
  u8  width
  u8  height
  u8  layer_type      (body, armor, weapon, overlay, icon, portrait...)
  u8  flags           (bit0: no_outline)
  u8  cmd_count

Command:
  u8  opcode (high 4 bits) | slot (low 4 bits)
  arguments, one u8 each
```

Command sizes: `RECT` 5 bytes, `PIXEL` 3, `LINE` 5, `ELLIPSE`/`RING` 5, `POLY` 2 + 2n, `MIRROR` 2, `SHADE` 4, `SOCKET` 3.

`BITMAP` binary layout: `x y w h`, then run-length coded pixels. A run byte is `len << 6 | value`: `len` 0-2 is a run of 1-3 pixels, `len` 3 reads one more byte for a run of 4-259. `value` is `slot << 3 | code`: slot 7 is a skip, code 0 is auto shading, code `1 + c` is ramp color `c`. A run that goes past `w*h` is error 6.

A typical geometry sprite: 6-15 commands, **20-80 bytes**. A 64×64 bitmap at 4 bits per pixel takes 2048 bytes before RLE.

A sprite book may point several ids at the same bytes. 1 px details are allowed inside a `BITMAP`.

### 4.6 Exceptions

| Exception                       | Where allowed                                                                       |
| ------------------------------- | ----------------------------------------------------------------------------------- |
| Details 1 px thick              | bowstring, necklace chain, strands of hair, eyes, highlights (`SHADE`)              |
| Layer without outline (`no_outline`) | shadow, necklace, the fist over the weapon and element overlays                                   |
| Explicit outline (slot `O`)     | inner lines that must be an outline: a visor slit, a gap between fingers            |
| Explicit shade (`SHADE`)        | a gem's glint, a spark in an eye, a deliberate highlight. No more than 8 per sprite |
| Going past the canvas edge      | forbidden for shapes. Shapes stay at least 1 px from the edge so the outline fits   |

---

## 5. Auto-shading and outline

### 5.1 Slot ramps

After the commands run, each slot turns into a ramp of **6 colors**: 0 is the outline color of the ramp, 1-4 are the shades of the form (dark to light), 5 is the highlight. Auto shading has 4 levels for `P K E D U R`, which become colors 1, 2, 3, 4, and 3 levels for `S H A Y`, which become colors 1, 2, 4. Color 5 is never produced by auto shading: only explicit colors (`BITMAP`, `SHADE`) and the rune glow bands use it.

| Slot            | Ramp                                                                |
| --------------- | ------------------------------------------------------------------- |
| `P` / `K`       | 6 colors                                                            |
| `S` / `H`       | 6 colors                                                            |
| `A` / `Y`       | 6 colors                                                            |
| `E` / `D` / `R` | 6 colors                                                            |
| `U`             | counted as slot `P` when shading, recolored by the rules of 9.4     |
| `O`             | the global outline color                                            |

### 5.2 Shading rule (light from the top left)

A neighbor is "own" if it is in the same layer and in the same slot.

```
L = left neighbor is not own
T = top neighbor is not own
R = right neighbor is not own
B = bottom neighbor is not own
n = number of shades of the slot (4 or 3, section 5.1)

if (L or T) and not (R or B):
    shade = n-1, if both L and T (corner highlight)
    otherwise = min(2, n-1)
else if (R or B) and not (L or T):
    shade = 0 (shadow)
else:
    shade = 1 (middle)
```

The result is a shade level; section 5.1 maps levels to ramp colors (1-4 for 4 levels, 1, 2, 4 for 3 levels).

Consequence: **a border between slots works as a facet**. A blade `P` next to a hilt `S` automatically gets a light and a dark edge at the joint.

### 5.3 Outline

- For a layer without the `no_outline` flag: an empty pixel of the layer that has a non-empty 4-neighbor in the same layer becomes an outline pixel.
- The outline is drawn **over the lower layers**, but under the fill of its own layer.
- The outline pixel takes **color 0 of the ramp of its first filled 4-neighbor**, in the order left, top, right, bottom, together with that pixel's enchantment cast. If the neighbor is slot `O`, the global outline color is used (almost black with a slight violet). A state can recolor the outline: curse, cold.

### 5.4 Dithering

A material has a dither flag: cloth, leather, stone and plants dither between neighboring shades, metal does not. Rule: in the material's `P`, `S` and `U` slots, a pixel on an odd `x + y` whose same-slot 4-neighbor is exactly one color darker takes that color. The rule is ordered and uses integers only.

### 5.5 Overlap shadow

Before a layer with an outline is drawn, the pixels of the lower layers right below its footprint get one ramp color darker. The footprint is the layer's fill plus its outline. A pixel at color 1 stays.

---

## 6. Pose, anchor points and character layers

### 6.1 Pose

- **View:** 3/4, facing right. All characters share one pose, otherwise the layers will not line up.
- **Height:** about 70-74 px, feet on the baseline **y = 91**.
- **Head:** about 13×14 px, larger than realistic so that the face reads.
- All races share one silhouette. The races differ in color, hair and features; the anchors lie below the neck and are the same for all.
- **Monsters** are drawn facing **left**. The site mirrors a monster toward its target: the hero it attacked last, otherwise the nearest living hero. Mirroring is picture only; heroes are never mirrored.

### 6.2 Anchor points

Starting values. After the first 2-3 characters they are frozen together with the rasterizer version.

| Anchor        | Coordinates | Purpose                                   |
| ------------- | ----------- | ----------------------------------------- |
| `HAND_FRONT`  | (63, 62)    | primary weapon, glove of the near hand    |
| `HAND_BACK`   | (34, 63)    | shield, second weapon, glove of the far hand |
| `HEAD_CENTER` | (48, 27)    | helmets, hoods, crowns                    |
| `NECK`        | (48, 35)    | necklace, pendant                         |
| `BACK`        | (44, 42)    | cloak, weapon on the back                 |
| `WAIST`       | (48, 57)    | belt, top of the pants                    |
| `FEET`        | (48, 91)    | boots, shadow                             |

### 6.3 Visible equipment

There are no class outfits. A character is the body, the hair and the items actually worn (`Character.gear`). One sprite per item base serves all races. Equip and unequip change the image.

| Equipment slot      | Visible on the character      | Layer                       |
| ------------------- | ----------------------------- | --------------------------- |
| Primary weapon      | yes                           | `MAINHAND`                  |
| Shield / second weapon | yes                        | `OFFHAND`                   |
| Helmet              | yes                           | `HELMET`                    |
| Torso armor         | yes                           | `ARMOR_BODY`                |
| Pants               | yes                           | `PANTS`                     |
| Boots               | yes                           | `BOOTS`                     |
| Gloves              | yes, separately on each hand (the front glove over the fist) | `GLOVE_BACK`, `GLOVE_FRONT` |
| Necklace            | yes                           | `NECKLACE`                  |
| Cloak               | yes                           | `BACK_ITEM`                 |
| Rings               | no, only the 32×32 icon       |                             |

### 6.4 Layer order (bottom to top)

The index of a layer in the render request is its place in this list.

1. `SHADOW` shadow (no_outline)
2. `BACK_ITEM` cloak, bag, weapon on the back
3. `BODY` the whole body of the race in one sprite: head, torso, arms, legs, briefs. One sprite so that no outline seams appear at the shoulder, elbow, wrist or knee.
4. `GLOVE_BACK`
5. `PANTS`
6. `BOOTS`
7. `ARMOR_BODY` torso armor or clothing, belt
8. `NECKLACE` (no_outline, over the armor)
9. `HAIR`
10. `OFFHAND` shield or second weapon, held in front of the torso on the back arm, over the body, the armor and the hair; long hair falls behind it; its near rim is in front
11. `HELMET`
12. `MAINHAND` primary weapon
13. `FIST_FRONT` the front fist alone, the same pixels as in the body, drawn over the weapon so the fingers wrap the grip (no_outline)
14. `GLOVE_FRONT` over the fist
15. `OVERLAY_ELEMENT` frost, fire on the body (no_outline)
16. `FX` procedural effects, not stored as sprites

An empty layer is not drawn. A long robe covers the pants through the ordering: `ARMOR_BODY` comes after `PANTS`. There is no wound overlay and no torn variant of an item: only worn items change the image, plus the gray of death.

### 6.5 Sprite id layout

| Ids       | What                                                            |
| --------- | --------------------------------------------------------------- |
| 1-399     | character bodies, hair and item bases                           |
| 400-499   | monsters                                                        |
| 500-599   | tiles of Caves and Ruins and of Forest, map points              |
| 600-799   | icons of resources and consumables                              |
| 800-999   | tiles of Crypt                                                  |
| 1000 and up | site-only sprites (city, interface); they never go into the on-chain books |

Crafted items need no sprites of their own: an item is drawn with the sprite of its base, and its colors come from its materials (section 9.2).

---

## 7. Command order inside a sprite

Because of the painter's algorithm, the order of commands determines the result.

1. **Silhouette.** Large masses of the primary slot (`P` or `K`): `RECT`, `ELLIPSE`, `POLY`. As few commands as possible.
2. **Secondary parts** (`S`/`H`): hilts, shafts, straps, hair.
3. **Cutouts** (slot `_`): gaps, holes, the inner side of a bow. Always after the shape being cut.
4. **Mirroring** (`MIRROR_X`/`MIRROR_Y`) for symmetric items: first a half, then the mirror.
5. **Accents** (`A`/`Y`): buckles, trim, eyes. Only permanent details of the shape, not what crafting provides.
6. **Rune zone** (`U`): a line or strip along the primary material where runes can appear.
7. **Element core** (`E`): the place where the enchantment glows strongest (staff head, center of a blade). Without an enchantment the slot is drawn as `P`.
8. **Socket points** (`SOCKET`): places where crafting can create sockets.
9. **Explicit outlines** (`O`): slits, inner lines.
10. **Explicit shades** (`SHADE`): highlights, no more than 4.

Limit: **no more than 48 commands per sprite** (a starting value, refined on the first sprites). If it does not fit, the shape is too detailed for its canvas.

---

## 8. Which geometry to use for what

| Object                  | Geometry                                                                               |
| ----------------------- | -------------------------------------------------------------------------------------- |
| Straight blade          | `RECT` 2 px wide + `PIXEL` tip; guard `RECT` in `A`; hilt `RECT`/`PIXEL` in `S`        |
| Diagonal blade          | two parallel `LINE`s offset by 1 px                                                    |
| Axe                     | `RECT` handle in `S` + `POLY` blade in `P`                                             |
| Staff, wand             | `RECT` 2 px in `S` + `ELLIPSE` or `POLY` head in `P`/`E`                               |
| Bow                     | `RING` + `RECT` in `_` to leave the arc; bowstring `LINE` in `S` (1 px allowed)        |
| Round shield            | `ELLIPSE` in `P` + a smaller `ELLIPSE` in `A` (umbo)                                   |
| Kite shield             | `POLY` of a half + `MIRROR_X`, umbo `ELLIPSE` in `A`                                   |
| Helmet                  | a stack of `RECT`s (dome in rows) or an `ELLIPSE` top + `RECT` rim in `A`              |
| Torso armor             | `RECT` base + `RECT` pauldrons + `RECT` belt in `S` + `PIXEL` buckle in `A`            |
| Body                    | one `BITMAP` per race: head, torso, arms, legs in a single sprite                      |
| Gems, pendants          | `RECT` 2×2 in `A` + `SHADE` highlight                                                  |
| Necklace                | `PIXEL` chain in `S` + `RECT` 2×2 pendant in `A`, no_outline layer                     |
| Ring (icon)             | `RING` in `P` + `RECT` 2×2 gem in `A`                                                  |
| Cloak                   | `RECT` and `POLY` in `P`, edges visible behind the body                                |
| Rune zone on a blade    | `LINE` or `RECT` 1 px along the center of the blade in `U`, at least 12 px long        |
| Rune zone on a shield, armor | `RECT` strip in `U` along the rim or along the center of the plate               |
| Weapon sockets          | `SOCKET` on the guard and the pommel, for a two-handed weapon also on the blade near the guard |
| Shield, armor sockets   | `SOCKET` at the center and at the corners of the main plate                            |

### Allowed line slopes

Clean pixel slopes: **0°, 90°, 45°, 1:2 and 2:1**. Other angles give ragged "staircases". Exception: bowstrings and chains.

---

## 9. Palette, materials, elements

### 9.1 Master palette

The wide palette lives **at the level of the whole game**, not of a sprite: for example, 48 ramps of 6 colors = 288 colors. It includes metals, wood, leather, fabrics, stones, the skin tones of the races, blood and all elements. The palette may be larger than 256 colors, but one frame uses at most 256. A ramp has its shadows shifted cold and its light shifted warm. A material is a primary ramp, a secondary ramp and flags (for example, the dither flag, 5.4). A new palette version only appends to the old one.

### 9.2 Materials and crafting

A base item (for example, "two-handed sword") exists only as a shape. The specific look comes from crafting: the player brings resources, the crafting program converts them into item state fields, and the rasterizer draws the result.

| Item state field        | Where it comes from                                             | What it changes in the image                                                          |
| ----------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `base_id`               | recipe (item type)                                              | which sprites: in hand, portrait, icon                                                |
| `primary_material`      | main resource (iron, onyx, gold...)                             | ramp of slots `P` and `U`                                                             |
| `secondary_material`    | hilt or shaft resource (oak, leather, bone...)                  | ramp of slot `S`                                                                      |
| `accent_material`       | trim (bronze, silver, gold...)                                  | ramp of slot `A`                                                                      |
| `enchantment`           | enchantment reagent (darkness, blood, cold, fire, lightning) or none | **a color cast on the material itself** (formula below), ramp of the core `E`, procedural effect (9.3) |
| `enchant_level`         | sharpening, strengthening                                       | strength of the enchantment color cast, highlights (section 10)                       |
| `runes[]`               | inscribed runes, up to 3                                        | runes in the zone `U` (9.4)                                                           |
| `sockets[]`             | number of sockets and what is set in them                       | sockets at the `SOCKET` points (9.4)                                                  |

So a crafted item never needs a new sprite. One sprite of a two-handed sword covers all combinations: a steel one with no runes, an onyx one with a darkness enchantment and three runes, an iron one with two empty sockets.

**Enchantment color cast.** An enchantment does not draw a glow around the item, it recolors the material itself. The ramp of the primary material (slots `P` and `U`) is blended with the enchantment ramp shade by shade, in integers:

```
k = min(64 + 16 × enchant_level, 160)          // enchantment share out of 256
color[i] = (material[i] × (256 - k) + enchantment[i] × k) / 256   // per RGB channel
```

- With no sharpening the cast is 25%, and as `enchant_level` grows it strengthens to about 62%, but the material always stays recognizable: an onyx sword with cold is bluish onyx, not simply a blue sword.
- The secondary material (`S`) and the accent (`A`) are not recolored: the hilt and the trim stay their own.
- The core `E` is drawn with the pure enchantment ramp: this is the place where the enchantment "comes out".
- For darkness the blend uses a black-violet ramp, and the material darkens.

Race = a different ramp for slot `K`. Curse = recoloring ramps and outline. Death = all ramps become 6 shades of gray (brightness `(77r + 150g + 29b) >> 8`); the outline does not change, because by 5.3 only states such as curse and cold recolor it.

### 9.3 Elements and damage types

| Type                 | Ramp (dark → light)                                    | Procedural effect                                         |
| -------------------- | ------------------------------------------------------ | --------------------------------------------------------- |
| Fire                 | dark red, orange, yellow, almost white                 | color cast on the material, particles above the item      |
| Cold                 | dark blue, light blue, pale light blue, white          | light blue outline, frost crystals on edge pixels         |
| Lightning            | violet, blue, yellow, white                            | jagged sparks, flicker of the lightest shades             |
| Bleeding / Blood     | near-black red, dark red, red, highlight               | drops falling from the lowest pixels of the mask          |
| Darkness             | black, dark violet, violet, pale lilac                 | the material darkens, occasional smoke particles          |
| Blunt                | no ramp of its own                                     | cracks inside the mask, slight desaturation               |
| Piercing             | no ramp of its own                                     | stronger highlights on sharp edges                        |

A combo (for example, Piercing Frost) = the ramp of one element + the effect of another type. No new sprites are needed.

### 9.4 Runes and sockets

**Rune zone (`U`).**

- Without runes: `U` pixels are drawn exactly like `P` (the same material, the same shading). The zone is invisible.
- With runes: the zone is divided into equal segments by the number of runes (1-3), and pixels are enumerated row by row. In each segment the PRNG (seed: item address + rune index) picks a pattern: each pixel is either "carved out" (shade 0 of the material) or "glowing" (the rune ramp). Every sword gets its own unique but reproducible rune pattern.
- The rune color is taken from the ramp of its type (fire rune, cold rune, darkness rune...).

**Layered glow ("rainbow").**

Each rune adds its own glow band 2 px thick around the item. The bands lie concentrically, from the item outward, in the order the runes were inserted. An enchantment gives no band, it recolors the material itself (9.2).

| Distance from the item | Band   |
| ---------------------- | ------ |
| 1-2 px                 | rune 1 |
| 3-4 px                 | rune 2 |
| 5-6 px                 | rune 3 |

- Maximum 3 bands.
- Item mask = all pixels of slots `P`, `U`, `E` of this layer. Distance is counted by Chebyshev. Bands are drawn only on pixels outside the mask.
- Band color = the lightest shade of the rune ramp.
- Opacity decreases outward: 55%, 40%, 30%.
- The band of a darkness rune does not glow but darkens: the pixels under it are blended with black-violet at the same opacity.
- Bands are drawn over the lower layers (body, armor) and may spill onto them. On the character canvas, whatever does not fit past the edge is clipped.
- **Portraits of items that can carry runes leave 6 px of margin from the canvas edges**, so that all bands fit.

**Sockets (`SOCKET`).**

- A sprite defines up to 4 points. Crafting determines how many sockets actually exist (`sockets.len()` ≤ number of points). Sockets take the points in order.
- Empty socket: a 4×4 square with its top left corner at the point, shade 0 of the primary material, with a 1 px frame of shade 1 around it. It looks like a recess.
- Filled socket: the same square in the ramp of the inserted gem or rune, the top left pixel is the lightest (highlight), the rest by shading.
- Sockets are drawn after the layer's shading and before effects, so the enchantment glow does not cover them.

---

## 10. Procedural effects

The rasterizer does not implement these effects yet (frost, sparks, blood drops, sharpening). They are planned for later.

Effects **never use coordinates**. They are computed from layer masks.

| Effect              | Mask                                            | Rule                                                                            |
| ------------------- | ----------------------------------------------- | ------------------------------------------------------------------------------- |
| Enchantment color cast | pixels of slots `P` and `U`                  | blending the material ramp with the enchantment ramp (formula in 9.2)           |
| Rune glow           | pixels of slots `P`, `U`, `E`                   | concentric bands of 2 px, runes in order of insertion (rules in 9.4)            |
| Frost               | edge pixels of the mask (has a transparent 4-neighbor) | crystals on edge pixels chosen by the PRNG                               |
| Sparks, particles   | all pixels of the mask                          | count = `clamp(area / 8, 2, 8)`, positions chosen by the PRNG                   |
| Blood drops         | the lowest pixels of the mask in each column    | drops 1-2 px long going down                                                    |
| Sharpening          | pixels of the slot's lightest shade             | lightening toward white; `enchant_level` also strengthens the enchantment color cast (9.2) |

### PRNG

- Seed: the first 8 bytes of `SHA-256(asset_address ‖ effect_id)`, little-endian `u64`.
- Generator: `splitmix64`.
- Pixel pick: `next() % mask_pixel_count`, mask pixels are enumerated row by row from top to bottom, left to right.

Every item instance is unique, but anyone can reproduce the image 1:1.

---

## 11. Character states

| State           | Implementation                                                                |
| --------------- | ----------------------------------------------------------------------------- |
| Curse           | recoloring of ramps and outline                                               |
| Cold            | recoloring of the outline                                                     |
| Death           | tombstone sprite + the character portrait in gray ramps                       |
| Sharpening      | the item's `enchant_level`, effect from section 10                            |

Only worn items change the image, plus the gray of death and the curse and cold flags. There are no wound overlays and no severed arm.

All of this is state bytes. The image reassembles itself automatically.

---

## 12. Readability at 96×96

- Silhouette matters more than details: an item must read if filled with a single color.
- A weapon extends beyond the body silhouette and contrasts with it in brightness.
- Length: a one-handed weapon 20-28 px, two-handed weapons and staves 32-44 px.
- Minimum detail 2 px (exceptions in 4.6).
- Thin items (bow, wand, shaft) are at least 2 px thick at key places, otherwise effects have nothing to catch on.
- The 32×32 icon and the portrait are drawn as separate sprites, not by scaling.

---

## 13. Output PNG

The encoder is written inside the rasterizer, without external crates.

- **Scale:** nearest neighbor, the integer `k = 512 / max(w, h)` (96 → x5 = 480 px, 64 → x8, 128 → x4, 32 → x16). Otherwise wallets blur the pixel art when stretching.
- **Format:** indexed PNG (color type 3), 8 bits per pixel, a `PLTE` chunk with the colors of the final image. Transparency through `tRNS`, or the card background is filled with a color from the palette.
- **Filter:** every canvas row becomes `k` identical rows: the first gets the `None` filter, the other `k - 1` get the `Up` filter (after it they consist of zeros).
- **Colors:** more than 256 colors in one frame is a rasterizer error (code 10). There is no other format.
- **Compression:** a custom minimal deflate: fixed Huffman tables, repeats with only two distances: 1 (identical pixels in a row) and the row length (a row repeat).
- **Checksums:** CRC32 for chunks, Adler32 for the zlib stream.
- **Result:** an image of up to 512×512 of the order of a few kilobytes, encoder code of the order of a few kilobytes in WASM.

---

## 14. The AI artist process

The AI draws **base shapes**, not specific items. It does not draw material, enchantment, runes and sockets, it only marks the places where they will appear after crafting.

1. Receive the assignment: base type and layer type ("two-handed sword, MAINHAND"), plus a short description of the shape ("wide blade, cross guard, long hilt").
2. Load the template: the base body with anchors and 5-10 reference sprites in the DSL.
3. Write the commands in the order of section 7, using the geometry from section 8:
   - shape and material slots `P`, `S`, `A`;
   - element core `E`, if the item type has a place for a glow;
   - rune zone `U`, if the item type can carry runes;
   - `SOCKET` points, if the item type can get sockets.
4. Run the validator (section 15).
5. Render the **preview matrix** with the same rasterizer, ×10 and 1:1, on the template body and separately:
   - 3 materials (steel, onyx, gold);
   - no enchantment and 2 enchantments (for example, darkness and cold);
   - no runes and 3 runes;
   - no sockets, with empty sockets, with filled sockets;
   - the debug slot palette.
6. Look at the matrix and fix: the shape must look good in **all** combinations, not in one. At most 5 iterations, after that simplify the shape.
7. Deliver the sprite in the DSL and in binary form.

Three sprites are created for each base item: in hand, portrait and icon. The zones `U`, `E` and the `SOCKET` points must agree between them (the same number of sockets, runes on the same parts of the item).

---

## 15. Validator: mandatory checks

- [ ] The canvas size matches the layer type (character layer 96, tile 64, monster 96 or 128), all shapes are inside the canvas with a 1 px margin
- [ ] Only slots allowed for the `layer_type`, in the commands and in the rows of a `BITMAP`
- [ ] No more than 48 commands, no more than 8 `SHADE`, no more than 4 `SOCKET`
- [ ] `FIST_FRONT` has no outline and matches the fist of the body
- [ ] A report of the bytes is produced; a sprite book is at most 64 KiB; a frame has at most 256 colors
- [ ] Anchors: a weapon's hilt at the hand anchor, a helmet at `HEAD_CENTER`, and so on by layer type
- [ ] No details thinner than 2 px, except the allowed exceptions
- [ ] `LINE` slopes only the allowed ones
- [ ] The silhouette is continuous, no "hanging" pixels
- [ ] An item in hand does not cover the character's face
- [ ] The rune zone `U` lies inside the `P` mask, no shorter than 12 px for a weapon
- [ ] Each `SOCKET` point gives a 4×4 square entirely inside the `P` mask, and sockets do not overlap
- [ ] An item portrait with a `U` zone has a 6 px margin from the edges for the rune glow bands
- [ ] The number of `SOCKET` points is the same in the in-hand sprite, the portrait and the icon
- [ ] The preview matrix (materials × enchantments × runes × sockets) rendered without errors
- [ ] Repeated rasterization gives the same result (determinism)
