# FORMAT: sprite DSL, binary encodings and rasterizer ABI

Format version **1**. This file is the contract between `crates/render`, `tools/art`, the sprite authors
(`assets/sprites/*.dsl`), `tools/scripts/upload-sprites.ts` and the app loader.
It is derived from `docs/ART_DESCRIPTION.md` (sections 4-6). Where they differ, this file wins for code.
Any change to the rules below after sprites ship is a **new rasterizer version**, never a silent fix.

All integers are unsigned. Multi-byte integers are little-endian. Coordinates are in pixels,
origin top left, `x` to the right, `y` down.

---

## 1. Text DSL (`assets/sprites/<name>.dsl`)

One file holds one or more sprites. Line based, one statement per line, tokens separated by spaces.
A comment is a `#` at the start of a token followed by a space or the end of the line; it runs to
the end of the line (so `#120c16` in a palette is a color, not a comment). Empty lines are ignored.
Keywords are case-sensitive.

```
# Warrior one-handed sword, held in the front hand
sprite 199 example_sword
size 32 32
layer mainhand
RECT 22 5 2 12 P
PIXEL 22 4 P
RECT 20 17 6 1 A
RECT 22 18 2 2 S
LINE 22 7 22 14 U
SOCKET 22 17
SHADE 22 5 P 3
```

### 1.1 Sprite header (in this order, once per sprite)

| Statement | Meaning |
|---|---|
| `sprite <id> <name>` | starts a sprite. `id` 1..65535, unique in the whole repo (ranges in 1.5). `name` is `[a-z0-9_]{1,32}`, unique in the repo |
| `size <w> <h>` | canvas, each of `16 32 48 64` |
| `layer <type>` | layer type name from 1.4 |
| `flags no_outline` | optional; the layer gets no auto outline |

Then the commands until the next `sprite` or end of file.

### 1.2 Commands

| Text | Opcode | Bytes | Arguments |
|---|---|---|---|
| `RECT x y w h S` | 0x1 | 5 | fills `x <= X < x+w`, `y <= Y < y+h` |
| `PIXEL x y S` | 0x2 | 3 | one pixel |
| `LINE x0 y0 x1 y1 S` | 0x3 | 5 | 1 px line, both ends included (2.2) |
| `ELLIPSE x y w h S` | 0x4 | 5 | filled ellipse inscribed in the rect `x y w h` (2.3) |
| `RING x y w h S` | 0x5 | 5 | 1 px ellipse outline (2.4) |
| `POLY S x1 y1 x2 y2 x3 y3 ...` | 0x6 | 2 + 2n | filled polygon, 3..8 vertices, vertices are pixel **corners** (2.5) |
| `MIRROR_X a` | 0x7 | 2 | mirror the layer about a vertical axis, `a` in doubled coordinates (2.6) |
| `MIRROR_Y a` | 0x8 | 2 | same, horizontal axis |
| `SHADE x y S shade` | 0x9 | 4 | one pixel with an explicit shade `0..n-1` of the slot (0 = darkest) |
| `SOCKET x y` | 0xA | 3 | socket point, draws nothing; the 2x2 square at `(x, y)` is the socket |

Note: the slot comes last for every command except `POLY` (first, because the vertex list is variable)
and `SHADE` (before the shade). `ELLIPSE`/`RING` take the **bounding rect**, not center and radii.

### 1.3 Slots

The slot is the role of a pixel, not a color. Items/armor letters and body letters are aliases
for the same value; use the one that matches the layer.

| Value | Item letter | Body letter | Shades `n` | Ramp (3.2) |
|---|---|---|---|---|
| 0 | `_` erase | `_` | - | transparent |
| 1 | `O` explicit outline | `O` | 1 | palette outline color |
| 2 | `P` primary material | `K` skin | 4 | material primary |
| 3 | `S` secondary material | `H` hair, fur | 3 | material secondary |
| 4 | `A` accent | `Y` eyes | 3 | accent ramp |
| 5 | `E` element core | `D` dark details | 4 | `E`: element ramp (primary if element ramp is 0). `D`: palette `dark_ramp` |
| 6 | `U` rune zone | `R` racial features | 4 | `U`: material primary, shaded as `P`. `R`: element ramp (primary if 0) |

Values 7..15 are invalid.

**Body layers** use the body meaning of slots 2..6 (colors and shading): layer types `arm_back`,
`legs`, `torso`, `head`, `hair`, `arm_front` and `monster`. Every other layer type uses the item
meaning. So value 5 is `D` (dark ramp) on a body layer and `E` (element ramp) elsewhere; value 6 is
`R` (element ramp, own slot when shading) on a body layer and `U` (primary ramp, shaded as `P`)
elsewhere.

### 1.4 Layer types

Character layers 0..19 follow the stacking order of `ART_DESCRIPTION.md` 6.4. The render order is the
**index in `RenderRequest.layers`**, not this byte; the layer type is used by the validator and for
the `shadow` blend (4.3). Use the same index in `Character.appearance` as the layer type.

| Byte | Name | Byte | Name |
|---|---|---|---|
| 0 | `shadow` | 10 | `necklace` |
| 1 | `back_item` | 11 | `head` |
| 2 | `arm_back` | 12 | `hair` |
| 3 | `glove_back` | 13 | `helmet` |
| 4 | `offhand` | 14 | `arm_front` |
| 5 | `legs` | 15 | `glove_front` |
| 6 | `pants` | 16 | `mainhand` |
| 7 | `boots` | 17 | `overlay_wounds` |
| 8 | `torso` | 18 | `overlay_element` |
| 9 | `armor_body` | 19 | `fx` (reserved, not stored) |
| 20 | `tile` (map cell, 32x32) | 22 | `icon` (16x16) |
| 21 | `monster` (whole monster in one sprite, 32, 48 or 64 square) | 23 | `portrait` |

Layers that must carry `flags no_outline`: `shadow`, `necklace`, `overlay_wounds`, `overlay_element`,
floor tiles.

Sizes per layer type: character layers 0..18 and `tile` are 32x32, `icon` is 16x16, `monster` is
32x32, 48x48 or 64x64, `portrait` any allowed size. A bigger monster is drawn on its own canvas;
the app aligns it bottom-center on its map cell, the rasterizer does not.

### 1.5 Sprite id ranges

| Ids | Content | Ids | Content |
|---|---|---|---|
| 0 | empty layer (never a sprite) | 300-399 | support kit |
| 1-99 | bodies: races, shadow, arms, heads, hair | 400-499 | monsters, tombstone |
| 100-199 | warrior kit | 500-599 | tiles |
| 200-299 | mage kit | 600+ | reserved |

---

## 2. Rasterization rules (normative)

Each sprite is drawn into its own **layer grid** of `w x h` cells. A cell holds `slot` (0 = empty),
`shade` and an `explicit` flag. Commands run in order; a later command overwrites earlier cells
(painter's algorithm) and clears their `explicit` flag; slot 0 makes cells empty.

Doubled coordinates: the center of pixel `X` is `2X+1`; a pixel corner `x` is `2x`.

### 2.1 RECT, PIXEL

As in the table in 1.2.

### 2.2 LINE

Bresenham, exactly this variant (all `i32`):
```
dx = |x1-x0|, sx = x0<x1 ? 1 : -1
dy = -|y1-y0|, sy = y0<y1 ? 1 : -1
err = dx + dy; (x, y) = (x0, y0)
loop:
  emit (x, y); if (x, y) == (x1, y1) break
  e2 = 2*err
  if e2 >= dy { err += dy; x += sx }
  if e2 <= dx { err += dx; y += sy }
```
Then pixel-perfect cleanup over the emitted list `p[0..n]`:
```
out = [p[0]]
for i in 1..n-1:
  a = last(out), b = p[i], c = p[i+1]
  if |a.x-c.x| == 1 and |a.y-c.y| == 1 and (a.x == b.x or a.y == b.y) and (c.x == b.x or c.y == b.y):
     skip b
  else push b
push p[n-1]            (if n > 1)
```

### 2.3 ELLIPSE

`cx = 2x + w`, `cy = 2y + h`. Pixel `(X, Y)` is filled if
`(2X+1-cx)^2 * h^2 + (2Y+1-cy)^2 * w^2 <= w^2 * h^2` (compute in `i64`).
Only pixels inside the rect `x y w h` are tested.

### 2.4 RING

Pixels that pass the `ELLIPSE` test with the same arguments and have at least one 4-neighbor
that fails it (the test is evaluated for neighbors outside the rect and outside the canvas too).

### 2.5 POLY

Vertices `(xi, yi)` are pixel corners, `0 <= xi <= w`, `0 <= yi <= h`, doubled to `(2xi, 2yi)`.
Pixel `(X, Y)` with doubled center `(px, py) = (2X+1, 2Y+1)` is filled if the number of edges `(a, b)`
(consecutive vertices, last to first) that satisfy both conditions is odd:
```
(a.y > py) != (b.y > py)
num = (a.x - px) * (b.y - a.y) + (py - a.y) * (b.x - a.x)
num * sign(b.y - a.y) > 0
```
This is even-odd on a ray to the right with the top-left rule: a center exactly on a left edge is
filled, on a right edge it is not. Centers never lie on a vertex or a horizontal edge (odd vs even).

### 2.6 MIRROR_X, MIRROR_Y

`MIRROR_X a`: for every non-empty cell `(X, Y)`, in row-major order of a snapshot taken before the
command, write the same cell (slot, shade, explicit) to `(a - 1 - X, Y)`. Targets outside the canvas
are skipped. `a = 32` mirrors between pixels 15 and 16, `a = 31` through the center of pixel 15.
`MIRROR_Y a`: same with `(X, a - 1 - Y)`. The source cells do not change. Empty cells are not copied.

### 2.7 SHADE, SOCKET

`SHADE` sets slot, shade and `explicit = true`; auto shading leaves it alone.
`SOCKET` draws nothing. Sockets, runes and enchantment effects are `TODO(post-demo)`; in format 1 the
points are decoded, validated and ignored by the renderer.

### 2.8 Auto shading (light from the top left)

For every non-empty, non-explicit cell of slot `s` with `n` shades (`n` from 1.3):
a neighbor is **own** if it is inside the canvas and has the same slot, where on item layers `U`
counts as `P` (on body layers `R` is its own slot).
```
L, T, R, B = left, top, right, bottom neighbor is NOT own
if (L or T) and not (R or B):  shade = (L and T) ? n-1 : min(2, n-1)
elif (R or B) and not (L or T): shade = 0
else:                           shade = 1
```
Slots with `n <= 2` (only `O`) get shade 0.
Shade `s` of a slot with `n` shades uses ramp color `s * 3 / (n - 1)` (integer division):
`n = 4` -> colors 0,1,2,3; `n = 3` -> colors 0,1,3.

### 2.9 Outline

Unless the sprite has `no_outline`: every **empty** cell of the layer grid with a non-empty
4-neighbor in the same grid becomes an outline pixel. Shapes must keep a 1 px margin from the
canvas edge or their outline is cut off.

---

## 3. Binary encodings

### 3.1 Sprite (as stored in a sprite book)

```
u8 version      = 1
u8 width        16 | 32 | 48 | 64
u8 height       16 | 32 | 48 | 64
u8 layer_type   0..23 (1.4)
u8 flags        bit0 = no_outline, other bits must be 0
u8 cmd_count
commands: u8 (opcode << 4 | slot), then the arguments, one u8 each, in the text order
          (POLY: n, then n pairs x y; SHADE: x y shade; MIRROR: a; SOCKET: x y)
```
`MIRROR_X`, `MIRROR_Y` and `SOCKET` must have slot 0. The sprite ends exactly after the last command.

### 3.2 Palette (body of the `Palette` account)

```
u8  format              = 1
u16 palette_version     must equal RenderRequest.palette_version
u8  outline r, g, b     the global outline color (#120c16)
u8  dark_ramp           ramp index for slot D (mouth, brows, scars) on every layer
u8  ramp_count          1..=255
ramp_count x 12 bytes   ramp: 4 colors r g b, dark -> light. Ramp 0 = neutral gray.
u8  material_count      1..=255
material_count x 2 bytes  material: primary_ramp, secondary_ramp
```
How a `LayerRef` picks colors:
- `material_ramp` is an index into **materials**: slots `P`/`K` and `U` use its `primary_ramp`,
  `S`/`H` its `secondary_ramp`;
- `accent_ramp` is a ramp index for `A`/`Y`;
- `element_ramp` is a ramp index for `E`/`R`; `0` means none, then `E` and `R` use the primary ramp;
- slot `D` always uses the palette `dark_ramp`, whatever the `LayerRef` says.

### 3.2.1 Palette text (`assets/palette/palette_<version>.pal`)

The source that `tools/art` compiles into 3.2. Same lexical rules as the DSL.
```
palette <palette_version>
outline #120c16
ramp <name> #rrggbb #rrggbb #rrggbb #rrggbb     # dark -> light; ramps are indexed in file order
dark <ramp name>
material <name> <primary ramp name> <secondary ramp name>   # indexed in file order
```
The first `ramp` (index 0) must be the neutral gray. Names are `[a-z0-9_]{1,32}`, unique per kind.
On-chain data and `LayerRef` carry only the indices.

### 3.3 Sprite book (body of the `SpriteBook` account)

```
u8  format    = 1
u16 book_id
u16 count
count x { u16 sprite_id, u32 offset, u16 len }   sorted by sprite_id, strictly ascending
sprite blobs  offset is from the start of the book body
```

### 3.4 Render request

`RenderRequest` from `crates/types`, Borsh, 106 bytes: `palette_version: u16`,
`layers: [LayerRef; 20]` (`sprite_id: u16, material_ramp: u8, accent_ramp: u8, element_ramp: u8`),
`flags: u32`.

| Flag bit | Meaning |
|---|---|
| 0 `GRAY` | dead character: every output color becomes gray `l = (77r + 150g + 29b) >> 8` |
| 1..31 | reserved, must be 0 |

---

## 4. Composition and output

1. The canvas is `max(width) x max(height)` over the non-empty layers; every layer is aligned to the
   top left corner. All canvas pixels start transparent `(0, 0, 0, 0)`.
2. Layers are drawn in order of their index in `RenderRequest.layers` (0 first). Empty layers
   (`sprite_id == 0`) are skipped. For each layer: first its outline pixels are written in the
   outline color, then its filled cells. Slot `O` cells use the outline color.
3. A layer of type `shadow` is not opaque: every filled cell blends black with alpha `90`:
   over a transparent pixel it writes `(0, 0, 0, 90)`; over a non-transparent one
   `rgb = rgb * 165 / 255` (alpha unchanged).
4. All other pixels are opaque (alpha 255). Then the `GRAY` flag is applied.
5. Output `RGBA`: `width*scale x height*scale` pixels, row-major, 4 bytes each, nearest neighbor
   integer scale `1..=16`. Output `PNG`: the same pixels as an RGBA8 PNG with stored (uncompressed)
   deflate blocks (`TODO(post-demo)`: indexed PNG with fixed Huffman, `ART_DESCRIPTION.md` 13).

## 5. Errors (never panic)

The decoder rejects, with an error code:

| Code | Error |
|---|---|
| 1 | truncated input or trailing bytes |
| 2 | unsupported format version |
| 3 | canvas size not in 16/32/48/64, unknown layer type, or reserved flag bits |
| 4 | unknown opcode (0x0, 0xB..0xF) |
| 5 | slot > 6, or non-zero slot on `MIRROR_*`/`SOCKET`, or slot 0 on `SHADE` |
| 6 | argument out of range: rect/ellipse with `w` or `h` 0 or past the canvas, pixel/line point outside, POLY `n` not 3..8 or vertex past `w`/`h`, mirror axis > `2*size`, shade >= `n`, socket square outside |
| 7 | sprite id not found in any book, or book index not sorted / out of bounds |
| 8 | palette: bad format, version mismatch, ramp or material index out of range |
| 9 | bad render request, reserved flag bits, bad output mode or scale |

## 6. Validator rules (`tools/art`, not the decoder)

- at most 24 commands, at most 4 `SHADE`, at most 4 `SOCKET`;
- every shape at least 1 px from the canvas edge (not for `no_outline` layers);
- `LINE` slopes only 0, 90, 45 degrees, 1:2 and 2:1;
- required `no_outline` for the layer types listed in 1.4; size allowed for the layer type (1.4);
- slots allowed for the layer type (body layers: `K H Y D R O _`);
- socket squares inside the `P` mask and not overlapping; `U` inside the `P` mask, at least 6 px for a weapon;
- id range and unique ids and names across `assets/sprites/`;
- the same sprite rendered twice gives the same bytes.

## 7. WASM C ABI

Exports (no `wasm-bindgen`; same module in the browser, a worker and Node):

```
alloc(len: u32) -> ptr          dealloc(ptr: u32, len: u32)
render(input_ptr: u32, input_len: u32, out_mode: u32, scale: u32) -> result_ptr   (0 = error)
result_len() -> u32             result_width() -> u32      result_height() -> u32
last_error() -> u32             (section 5 code, 0 = ok)
```
`out_mode`: 0 = RGBA, 1 = PNG. `scale`: 1..=16. `result_width/height` are the scaled size.
The result buffer is owned by the module and valid until the next `render` call.

Input layout:
```
u32 len + RenderRequest (Borsh, 3.4)
[u8; 32] asset address (PRNG seed for effects; unused in format 1, pass zeros)
u32 len + palette (3.2)
then one or more: u32 len + sprite book (3.3), until the end of the input
```
A sprite id is looked up in the books in the order they are given.
