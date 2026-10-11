# ART_DESCRIPTION: Deadroll art rendering

Version 0.4. This document covers **how the art of the game is drawn**: sprite geometry, the rasterizer, shading, layers, effects, the output image and the work of the AI artist.

Where the data lives, how the rasterizer is loaded and how the image reaches wallets is described in `STORAGE_AND_DELIVERY.md`.

Everything described here defines the on-chain data format. Changing the standard after the first sprites ship is expensive, so the rules are stated strictly.

---

## 1. Core principles

1. **The source of truth is geometry.** Every sprite is described by a short program of geometric commands (`RECT`, `LINE`, `ELLIPSE`...). The commands are stored, not the image.
2. **Commands define shape and role, not color.** The AI artist places shapes and specifies a role slot (primary material, hilt, accent). Shadows, highlights, outline and colors are computed by the rasterizer.
3. **Rendering is deterministic.** One universal function turns data into pixels. The same data always gives the same image, on any device.
4. **The image is assembled from layers.** A character is a stack of layers (body, armor, weapon...). A change of state (injury, a new helmet, sharpening) changes a few bytes, and the image reassembles itself.
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
- `PNG`: a finished PNG, enlarged by nearest neighbor to 512 px on the longer side.

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
| Character layer (body, armor, weapon in hand...)     | 32×32           |
| Item icon in the inventory                           | 16×16           |
| Item portrait for the NFT card: shield, helmet, amulet | 32×32         |
| Portrait of a long item: sword, staff, bow, spear    | 16×48 or 32×64  |

Width and height are stored in the sprite header. Allowed values: 16, 32, 48, 64.

An item has three sprites: **in hand** (a simplified silhouette inside the character canvas), **portrait** (detailed, on its own canvas) and **icon** 16×16. Materials, elements and effects apply to all of them the same way.

Why 32×32 and not larger: the cost of storing geometry barely depends on resolution, but on larger canvases several times more commands are needed for details, the style stops being pixel art, and it is harder for the AI artist to hit the anchors. A move to 64×64 is possible later without changing the format.

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

### 4.4 Exact rasterization rules

These rules are mandatory for implementations: they are what makes all clients draw the same thing.

- **Order:** commands run strictly one after another. A later command overwrites an earlier one (painter's algorithm). Slot 0 erases.
- **RECT:** pixels with `x ≤ X < x+w` and `y ≤ Y < y+h`.
- **LINE:** Bresenham's algorithm from `(x0,y0)` to `(x1,y1)`, both ends included. Then **pixel-perfect cleanup**: if points `i-1` and `i+1` are diagonal neighbors and point `i` touches both orthogonally, point `i` is removed. This way diagonals do not get "steps" 2 px thick.
- **ELLIPSE:** in doubled coordinates the center is `cx = 2x + w`, `cy = 2y + h`. Pixel `(X, Y)` is filled if `(2X+1-cx)² · h² + (2Y+1-cy)² · w² ≤ w² · h²`. Works for even and odd sizes.
- **RING:** the pixels that fall inside the `ELLIPSE` with the same arguments and have at least one 4-neighbor outside it.
- **POLY:** a pixel is filled if its center is inside the polygon by the even-odd rule, checked in doubled coordinates. A center exactly on an edge is filled if the edge is a left or a top edge (top-left rule).
- **MIRROR_X a:** for every non-empty pixel `(X, Y)` of the layer, the same pixel is written at `(a - 1 - X, Y)`. The axis is in doubled coordinates: `a = 32` mirrors between pixels 15 and 16, `a = 31` through the center of pixel 15. The source does not change.
- **MIRROR_Y:** the same vertically.
- **SHADE:** the pixel gets the slot from the nibble and a specific shade `shade` (0 = darkest). Auto-shading does not touch it.

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

A typical sprite: 6-15 commands, **20-80 bytes**. The same sprite as a 32×32 bitmap at 4 bits would take 512 bytes.

### 4.6 Exceptions

| Exception                       | Where allowed                                                                       |
| ------------------------------- | ----------------------------------------------------------------------------------- |
| Details 1 px thick              | bowstring, necklace chain, strands of hair, eyes, highlights (`SHADE`)              |
| Layer without outline (`no_outline`) | shadow, necklace, wound and element overlays                                   |
| Explicit outline (slot `O`)     | inner lines that must be an outline: a visor slit, a gap between fingers            |
| Explicit shade (`SHADE`)        | a gem's glint, a spark in an eye, a deliberate highlight. No more than 4 per sprite |
| Going past the canvas edge      | forbidden for shapes. Shapes stay at least 1 px from the edge so the outline fits   |

---

## 5. Auto-shading and outline

### 5.1 Slot ramps

After the commands run, each slot turns into a range of indices, shades from dark to light:

| Slot            | Indices                                                             | Shades   |
| --------------- | ------------------------------------------------------------------- | -------- |
| `P` / `K`       | 2-5                                                                 | 4        |
| `S` / `H`       | 6-8                                                                 | 3        |
| `A` / `Y`       | 9-11                                                                | 3        |
| `E` / `D` / `R` | 12-15                                                               | 4        |
| `U`             | counted as slot `P` when shading, recolored by the rules of 9.4     | 4        |
| `O`             | 1                                                                   | 1        |

### 5.2 Shading rule (light from the top left)

A neighbor is "own" if it is in the same layer and in the same slot.

```
L = left neighbor is not own
T = top neighbor is not own
R = right neighbor is not own
B = bottom neighbor is not own
n = number of shades of the slot

if (L or T) and not (R or B):
    shade = n-1, if both L and T (corner highlight)
    otherwise = min(2, n-1)
else if (R or B) and not (L or T):
    shade = 0 (shadow)
else:
    shade = 1 (middle)
```

Slots with 1-2 shades are not shaded, the first shade is used.

Consequence: **a border between slots works as a facet**. A blade `P` next to a hilt `S` automatically gets a light and a dark edge at the joint.

### 5.3 Outline

- For a layer without the `no_outline` flag: an empty pixel of the layer that has a non-empty 4-neighbor in the same layer becomes an outline pixel.
- The outline is drawn **over the lower layers**, but under the fill of its own layer.
- The outline color is one global color (almost black with a slight violet), and a state can recolor it: curse, cold.

---

## 6. Pose, anchor points and character layers

### 6.1 Pose

- **View:** 3/4, facing right. All characters share one pose, otherwise the layers will not line up.
- **Height:** about 22-24 px, feet on the baseline **y = 30**.
- **Headroom:** 6 px for helmets, hoods, a raised weapon.
- **Body width:** about 10-12 px.
- **Head:** about 8-9 px, larger than realistic so that the face and wounds read.

### 6.2 Anchor points

Starting values. After the first 2-3 characters they are frozen together with the rasterizer version.

| Anchor        | Coordinates | Purpose                                   |
| ------------- | ----------- | ----------------------------------------- |
| `HAND_FRONT`  | (21, 19)    | primary weapon, glove of the near hand    |
| `HAND_BACK`   | (10, 19)    | shield, second weapon, glove of the far hand |
| `HEAD_CENTER` | (15, 10)    | helmets, hoods, crowns                    |
| `NECK`        | (15, 15)    | necklace, pendant around y = 16-17        |
| `BACK`        | (12, 15)    | cloak, weapon on the back                 |
| `WAIST`       | (15, 22)    | belt, top of the pants                    |
| `FEET`        | (15, 30)    | boots, shadow                             |

### 6.3 Visible equipment

| Equipment slot      | Visible on the character      | Layer                       |
| ------------------- | ----------------------------- | --------------------------- |
| Primary weapon      | yes                           | `MAINHAND`                  |
| Shield / second weapon | yes                        | `OFFHAND`                   |
| Helmet              | yes                           | `HELMET`                    |
| Torso armor         | yes                           | `ARMOR_BODY`                |
| Pants               | yes                           | `PANTS`                     |
| Boots               | yes                           | `BOOTS`                     |
| Gloves              | yes, separately on each hand  | `GLOVE_BACK`, `GLOVE_FRONT` |
| Necklace            | yes                           | `NECKLACE`                  |
| Cloak               | yes                           | `BACK_ITEM`                 |
| Rings               | no, only the 16×16 icon       |                             |

### 6.4 Layer order (bottom to top)

1. `SHADOW` shadow (no_outline)
2. `BACK_ITEM` cloak, weapon on the back
3. `ARM_BACK` far arm with the hand
4. `GLOVE_BACK`
5. `OFFHAND` shield or second weapon
6. `LEGS` legs
7. `PANTS`
8. `BOOTS`
9. `TORSO` torso and neck
10. `ARMOR_BODY` torso armor or clothing, belt
11. `NECKLACE` (no_outline, over the armor)
12. `HEAD` head, eyes, mouth
13. `HAIR`
14. `HELMET`
15. `ARM_FRONT` near arm with the hand
16. `GLOVE_FRONT`
17. `MAINHAND` primary weapon
18. `OVERLAY_WOUNDS` blood, wounds (no_outline)
19. `OVERLAY_ELEMENT` frost, fire on the body (no_outline)
20. `FX` procedural effects, not stored as sprites

An empty layer is not drawn. A long robe covers the pants through the ordering: `ARMOR_BODY` comes after `PANTS`.

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

Limit: **no more than 24 commands per sprite** (a starting value, refined on the first sprites). If it does not fit, the shape is too detailed for 32×32.

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
| Body                    | only `RECT`s from a template: head, torso, arms, legs as separate layers               |
| Gems, pendants          | `RECT` 2×2 in `A` + `SHADE` highlight                                                  |
| Necklace                | `PIXEL` chain in `S` + `RECT` 2×2 pendant in `A`, no_outline layer                     |
| Ring (icon)             | `RING` in `P` + `RECT` 2×2 gem in `A`                                                  |
| Cloak                   | `RECT` and `POLY` in `P`, edges visible behind the body                                |
| Rune zone on a blade    | `LINE` or `RECT` 1 px along the center of the blade in `U`, at least 6 px long         |
| Rune zone on a shield, armor | `RECT` strip in `U` along the rim or along the center of the plate               |
| Weapon sockets          | `SOCKET` on the guard and the pommel, for a two-handed weapon also on the blade near the guard |
| Shield, armor sockets   | `SOCKET` at the center and at the corners of the main plate                            |

### Allowed line slopes

Clean pixel slopes: **0°, 90°, 45°, 1:2 and 2:1**. Other angles give ragged "staircases". Exception: bowstrings and chains.

---

## 9. Palette, materials, elements

### 9.1 Master palette

The wide palette lives **at the level of the whole game**, not of a sprite: for example, 48 ramps of 4 shades = 192 colors. It includes metals, wood, leather, fabrics, stones, the skin tones of the races, blood and all elements.

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

One sprite of a two-handed sword covers all combinations: a steel one with no runes, an onyx one with a darkness enchantment and three runes, an iron one with two empty sockets.

**Enchantment color cast.** An enchantment does not draw a glow around the item, it recolors the material itself. The ramp of the primary material (slots `P` and `U`) is blended with the enchantment ramp shade by shade, in integers:

```
k = min(64 + 16 × enchant_level, 160)          // enchantment share out of 256
color[i] = (material[i] × (256 - k) + enchantment[i] × k) / 256   // per RGB channel
```

- With no sharpening the cast is 25%, and as `enchant_level` grows it strengthens to about 62%, but the material always stays recognizable: an onyx sword with cold is bluish onyx, not simply a blue sword.
- The secondary material (`S`) and the accent (`A`) are not recolored: the hilt and the trim stay their own.
- The core `E` is drawn with the pure enchantment ramp: this is the place where the enchantment "comes out".
- For darkness the blend uses a black-violet ramp, and the material darkens.

Race = a different ramp for slot `K`. Curse and tombstone = recoloring all ramps to special ones.

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

Each rune adds its own glow band 1 px thick around the item. The bands lie concentrically, from the item outward, in the order the runes were inserted. An enchantment gives no band, it recolors the material itself (9.2).

| Distance from the item | Band   |
| ---------------------- | ------ |
| 1 px                   | rune 1 |
| 2 px                   | rune 2 |
| 3 px                   | rune 3 |

- Maximum 3 bands.
- Item mask = all pixels of slots `P`, `U`, `E` of this layer. Distance is counted by Chebyshev. Bands are drawn only on pixels outside the mask.
- Band color = the lightest shade of the rune ramp.
- Opacity decreases outward: 55%, 40%, 30%.
- The band of a darkness rune does not glow but darkens: the pixels under it are blended with black-violet at the same opacity.
- Bands are drawn over the lower layers (body, armor) and may spill onto them. On the character canvas, whatever does not fit past the edge is clipped.
- **Portraits of items that can carry runes leave 3 px of margin from the canvas edges**, so that all bands fit.

**Sockets (`SOCKET`).**

- A sprite defines up to 4 points. Crafting determines how many sockets actually exist (`sockets.len()` ≤ number of points). Sockets take the points in order.
- Empty socket: a 2×2 square with its top left corner at the point, shade 0 of the primary material, with 1 px of shade 1 around it. It looks like a recess.
- Filled socket: the same square in the ramp of the inserted gem or rune, the top left pixel is the lightest (highlight), the rest by shading.
- Sockets are drawn after the layer's shading and before effects, so the enchantment glow does not cover them.

---

## 10. Procedural effects

Effects **never use coordinates**. They are computed from layer masks.

| Effect              | Mask                                            | Rule                                                                            |
| ------------------- | ----------------------------------------------- | ------------------------------------------------------------------------------- |
| Enchantment color cast | pixels of slots `P` and `U`                  | blending the material ramp with the enchantment ramp (formula in 9.2)           |
| Rune glow           | pixels of slots `P`, `U`, `E`                   | concentric bands of 1 px, runes in order of insertion (rules in 9.4)            |
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
| Severed arm     | the layers of that arm, glove and weapon are not drawn, a stump sprite replaces the arm |
| Wounds          | `OVERLAY_WOUNDS`, 3 stages                                                    |
| Curse           | recoloring of ramps and outline                                               |
| Death           | tombstone sprite + the character portrait in gray ramps                       |
| Sharpening      | the item's `enchant_level`, effect from section 10                            |

All of this is state bytes. The image reassembles itself automatically.

---

## 12. Readability at 32×32

- Silhouette matters more than details: an item must read if filled with a single color.
- A weapon extends beyond the body silhouette and contrasts with it in brightness.
- Length: a one-handed weapon 10-14 px, two-handed weapons and staves 16-22 px.
- Minimum detail 2 px (exceptions in 4.6).
- Thin items (bow, wand, shaft) are at least 2 px thick at key places, otherwise effects have nothing to catch on.
- The 16×16 icon and the portrait are drawn as separate sprites, not by scaling.

---

## 13. Output PNG

The encoder is written inside the rasterizer, without external crates.

- **Scale:** nearest neighbor up to 512 px on the longer side (32×32 → ×16). Otherwise wallets blur the pixel art when stretching.
- **Format:** indexed PNG (color type 3), 8 bits per pixel, a `PLTE` chunk with the colors of the final image. Transparency through `tRNS`, or the card background is filled with a color from the palette.
- **Filter:** the first row of each block of 16 identical rows gets the `None` filter, the other 15 get the `Up` filter (after it they consist of zeros).
- **Compression:** a custom minimal deflate: fixed Huffman tables, repeats with only two distances: 1 (identical pixels in a row) and the row length (a row repeat).
- **Checksums:** CRC32 for chunks, Adler32 for the zlib stream.
- **Result:** a 512×512 image of the order of a few kilobytes, encoder code of the order of a few kilobytes in WASM.

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

- [ ] The canvas size is allowed, all shapes are inside the canvas with a 1 px margin
- [ ] Only slots allowed for the `layer_type`
- [ ] No more than 24 commands, no more than 4 `SHADE`
- [ ] Anchors: a weapon's hilt at the hand anchor, a helmet at `HEAD_CENTER`, and so on by layer type
- [ ] No details thinner than 2 px, except the allowed exceptions
- [ ] `LINE` slopes only the allowed ones
- [ ] The silhouette is continuous, no "hanging" pixels
- [ ] An item in hand does not cover the character's face
- [ ] The rune zone `U` lies inside the `P` mask, no shorter than 6 px for a weapon
- [ ] Each `SOCKET` point gives a 2×2 square entirely inside the `P` mask, and sockets do not overlap
- [ ] An item portrait with a `U` zone has a 3 px margin from the edges for the rune glow bands
- [ ] The number of `SOCKET` points is the same in the in-hand sprite, the portrait and the icon
- [ ] The preview matrix (materials × enchantments × runes × sockets) rendered without errors
- [ ] Repeated rasterization gives the same result (determinism)
