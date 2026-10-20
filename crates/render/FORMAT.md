# FORMAT: sprite DSL, binary encodings and rasterizer ABI

Format version **2** (rasterizer version 2, palette format 2). Format 1 (canvases 16, 32, 48, 64,
palette 1 with 4-color ramps) is drawn only by rasterizer version 1. Format 2 has character layers
at 96x96, pixel-by-pixel `BITMAP` sprites, 6-color ramps, a colored outline, dithering and
overlap shadows. This file is the contract between `crates/render`, `tools/art`, the sprite authors
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
size 64 64
layer mainhand
RECT 44 10 4 24 P
RECT 44 8 2 2 P
RECT 40 34 12 2 A
RECT 44 36 4 4 S
RECT 45 12 2 12 U
SOCKET 44 26
RECT 44 10 2 2 P
SHADE 44 10 P 3
```

### 1.1 Sprite header (in this order, once per sprite)

| Statement | Meaning |
|---|---|
| `sprite <id> <name>` | starts a sprite. `id` 1..65535, unique in the whole repo (ranges in 1.5). `name` is `[a-z0-9_]{1,32}`, unique in the repo |
| `size <w> <h>` | canvas, each of `32 64 96 128` (per layer type, 1.4) |
| `layer <type>` | layer type name from 1.4 |
| `flags no_outline` | optional; the layer gets no auto outline |
| `copy <name>` | optional, after `layer`: the commands of an earlier sprite of the same file, then this sprite's own. Same bytes, so `art pack` stores them once (3.3) |

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
| `SHADE x y S shade` | 0x9 | 4 | one pixel with an explicit ramp color `0..5` (0 = darkest; `O` only 0) |
| `SOCKET x y` | 0xA | 3 | socket point, draws nothing; the 4x4 square at `(x, y)` is the socket |
| `BITMAP x y w h` + `h` rows | 0xB | 5 + runs | pixel by pixel, a slot (and optionally a color) per pixel (1.2.1, 2.7, 3.1) |

Note: the slot comes last for every command except `POLY` (first, because the vertex list is variable)
and `SHADE` (before the shade). `ELLIPSE`/`RING` take the **bounding rect**, not center and radii.

### 1.2.1 BITMAP rows

`BITMAP x y w h` is followed by exactly `h` lines of exactly `w` characters (no spaces), the rows
top to bottom. The rect must lie inside the canvas. Characters:

| Character | Pixel |
|---|---|
| `.` | nothing drawn: the cell keeps what earlier commands drew |
| `_` | erase |
| `O` | explicit outline pixel |
| `P S A E U` / `K H Y D R` | the slot, auto shaded (2.8) |
| `0`-`5` | slot 2 (`P`/`K`) with ramp color 0-5 |
| `a`-`f` | slot 3 (`S`/`H`) with ramp color 0-5 |
| `g`-`l` | slot 4 (`A`/`Y`) with ramp color 0-5 |
| `m`-`r` | slot 5 (`E`/`D`) with ramp color 0-5 |
| `s`-`x` | slot 6 (`U`/`R`) with ramp color 0-5 |

An explicit color is not auto shaded, like `SHADE`; every pixel still stores its slot, so
materials, enchantments, runes, sockets and the request flags work on it as on geometry.
`BITMAP` mixes freely with the other commands (painter's order).

### 1.3 Slots

The slot is the role of a pixel, not a color. Items/armor letters and body letters are aliases
for the same value; use the one that matches the layer.

| Value | Item letter | Body letter | Auto levels | Ramp (3.2) |
|---|---|---|---|---|
| 0 | `_` erase | `_` | - | transparent |
| 1 | `O` explicit outline | `O` | 1 | palette outline color |
| 2 | `P` primary material | `K` skin | 4 | material primary |
| 3 | `S` secondary material | `H` hair, fur | 3 | material secondary |
| 4 | `A` accent | `Y` eyes | 3 | accent ramp |
| 5 | `E` element core | `D` dark details | 4 | `E`: element ramp (primary if element ramp is 0). `D`: palette `dark_ramp` |
| 6 | `U` rune zone | `R` racial features | 4 | `U`: material primary, shaded as `P`. `R`: element ramp (primary if 0) |

A cell's shade is a **ramp color index 0..5** (`RAMP_COLORS = 6`). Auto shading (2.8) gives a level
that maps to a color; explicit shades (`SHADE`, `BITMAP` digits and lowercase letters) are the
color itself. `O` has only color 0.

Values 7..15 are invalid.

**Body layers** use the body meaning of slots 2..6 (colors and shading): layer types `body`,
`hair`, `fist_front` and `monster`. Every other layer type uses the item meaning. So value 5 is `D` (dark ramp) on a body layer and `E` (element ramp) elsewhere; value 6 is
`R` (element ramp, own slot when shading) on a body layer and `U` (primary ramp, shaded as `P`)
elsewhere.

### 1.4 Layer types

Character layers 0..15 are the stacking order of `ART_DESCRIPTION.md` 6.4, bottom to top. The render
order is the **index in `RenderRequest.layers`**, not this byte; the layer type is used by the
validator and for the `shadow` blend (4.3). Use the same index in `Character.appearance` as the
layer type.

| Byte | Name | Byte | Name |
|---|---|---|---|
| 0 | `shadow` | 8 | `hair` (under the shield: long hair falls behind it) |
| 1 | `back_item` (cloak, bag) | 9 | `offhand` (shield, second weapon: in front of the body on the back arm) |
| 2 | `body` (the whole body of a race) | 10 | `helmet` |
| 3 | `glove_back` | 11 | `mainhand` |
| 4 | `pants` | 12 | `fist_front` (the front fist alone, over the weapon) |
| 5 | `boots` | 13 | `glove_front` |
| 6 | `armor_body` | 14 | `overlay_element` |
| 7 | `necklace` | 15 | `fx` (reserved, not stored) |
| 16..19 | reserved (error 3) | | |
| 20 | `tile` (map cell, 64x64) | 22 | `icon` (32x32) |
| 21 | `monster` (whole monster in one sprite, 96 or 128 square) | 23 | `portrait` |

The body of a race is **one** sprite: head, torso, arms, legs and briefs in one grid, so the outline
never draws seams at the shoulder, elbow, wrist or knee. Hair is its own layer (a helmet covers it).
`fist_front` repeats the body's front fist pixel for pixel and is drawn over `mainhand`, so the
fingers wrap the grip; it is shared by every race (the race material colors it). A glove on the
front hand goes over the fist. There is no wound overlay: only worn items change the image, plus
the gray of death.

Layers that must carry `flags no_outline`: `shadow`, `necklace`, `fist_front` (the body under it
already has the outline; its own would draw a seam across the wrist), `overlay_element`, floor tiles.

Sizes per layer type: character layers 0..14 are **96x96**, `tile` is 64x64 (one map cell),
`icon` is 32x32, `monster` is 96x96 or 128x128, `portrait` any allowed size. Characters and monsters are drawn at the
tile pixel size, bottom-center on their map cell, overlapping the row above; the app aligns them,
the rasterizer does not.

Character anchors on the 96 canvas (shared by every race and every item, `ART_DESCRIPTION.md` 6.2):
`HAND_FRONT` (63, 62), `HAND_BACK` (34, 63), `HEAD_CENTER` (48, 27), `NECK` (48, 35),
`BACK` (44, 42), `WAIST` (48, 57), `FEET` (48, 91). Every race has the same silhouette and
anchors below the neck; races differ in head, ears, hair, face and skin ramp.

### 1.5 Sprite id ranges

| Ids | Content | Ids | Content |
|---|---|---|---|
| 0 | empty layer (never a sprite) | 400-499 | monsters, tombstone |
| 1-99 | shadow (1), race bodies, hair, the front fist | 500-599 | tiles |
| 100-399 | items: one sprite per `ItemBase`, id `100 + ItemBase` | 600+ | reserved |

There are no class outfits. A character is drawn as its body, its hair and the items it actually
wears (`Character.gear`): each worn item's sprite goes into the layer of its equipment slot
(`MainHand` -> `mainhand`, `Body` -> `armor_body`, `OffHand` -> `offhand`, `Helmet` -> `helmet`,
`Pants` -> `pants`, `Boots` -> `boots`, `Necklace` -> `necklace`, `Cloak` -> `back_item`; gloves and
rings draw nothing yet). Every item is drawn once, on the shared anchors, and fits any race.

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
are skipped. `a = 64` mirrors between pixels 31 and 32, `a = 63` through the center of pixel 31.
`MIRROR_Y a`: same with `(X, a - 1 - Y)`. The source cells do not change. Empty cells are not copied.

### 2.7 SHADE, SOCKET, BITMAP

`SHADE` sets slot, shade (a ramp color) and `explicit = true`; auto shading leaves it alone.
`BITMAP x y w h`: pixel `i` (row-major) goes to `(x + i % w, y + i / w)`. A skip pixel changes
nothing; erase empties the cell; a slot with code 0 sets the slot with `explicit = false` (auto
shaded); code `1 + c` sets the slot, shade `c` and `explicit = true`.
`SOCKET` draws nothing. The socket is the 4x4 square with its top left corner at `(x, y)`; it must
lie inside the canvas (error 6). `render` ignores the points; the crafted look (section 8) draws
sockets, runes and the enchantment cast.

### 2.8 Auto shading (light from the top left)

For every non-empty, non-explicit cell of slot `s` with `n` levels (`n` from 1.3):
a neighbor is **own** if it is inside the canvas and has the same slot, where on item layers `U`
counts as `P` (on body layers `R` is its own slot).
```
L, T, R, B = left, top, right, bottom neighbor is NOT own
if (L or T) and not (R or B):  shade = (L and T) ? n-1 : min(2, n-1)
elif (R or B) and not (L or T): shade = 0
else:                           shade = 1
```
Slots with `n <= 2` (only `O`) get shade 0. The level is stored as a ramp color: `n = 4` ->
levels 0,1,2,3 are colors 1,2,3,4; `n = 3` -> levels 0,1,2 are colors 1,2,4. Color 0 is left to
the outline and color 5 to explicit highlights.

### 2.9 Outline

Unless the sprite has `no_outline`: every **empty** cell of the layer grid with a non-empty
4-neighbor in the same grid becomes an outline pixel. Shapes must keep a 1 px margin from the
canvas edge or their outline is cut off.

The outline is **colored**: an outline pixel takes color 0 of the ramp of its first non-empty
4-neighbor in the order left, top, right, bottom, with that cell's enchantment cast (section 8).
A neighbor of slot `O` gives the palette outline color. Under `CURSE` or `COLD` (3.4) every
outline pixel, and every `O` cell, is the state outline instead.

### 2.10 Dithering

A material with the `dither` flag (3.2) dithers its shade borders, after auto shading: a cell of
slot `P`, `S` or `U` with `(x + y)` odd, shade `c >= 2` and a 4-neighbor of the same slot with
shade exactly `c - 1` takes shade `c - 1` (all neighbors are read before any change). Metal
materials have no flag. The `shadow` layer never dithers.

### 2.11 Overlap shadow

Before a layer with an outline is drawn (not `shadow`, not `no_outline`), every canvas pixel at
`(x, y)` inside the layer canvas, with the layer's footprint (filled cells and outline pixels) at
`(x, y - 1)` and not at `(x, y)`, gets one ramp color darker if it is a ramp pixel of color 2 or
more. Pixels drawn by earlier layers right under an upper layer's edge read as in its shadow.

---

## 3. Binary encodings

### 3.1 Sprite (as stored in a sprite book)

```
u8 version      = 2
u8 width        32 | 64 | 96 | 128
u8 height       32 | 64 | 96 | 128
u8 layer_type   0..23 (1.4)
u8 flags        bit0 = no_outline, other bits must be 0
u8 cmd_count
commands: u8 (opcode << 4 | slot), then the arguments, one u8 each, in the text order
          (POLY: n, then n pairs x y; SHADE: x y shade; MIRROR: a; SOCKET: x y;
          BITMAP: x y w h, then runs until w * h pixels)
```
`MIRROR_X`, `MIRROR_Y`, `SOCKET` and `BITMAP` must have slot 0. The sprite ends exactly after the
last command.

`BITMAP` pixels are run-length coded, row-major, runs may cross rows. A pixel value is
`slot << 3 | code`: slot 0..6, or 7 = skip; code 0 = auto shading, `1 + c` = explicit color `c`.
Erase and skip take only code 0, `O` codes 0..1, the other slots 0..6. A run byte is
`len << 6 | value`: `len` 0..2 is a run of `len + 1` pixels; `len` 3 reads one more byte `n` for a
run of `4 + n` (4..259). A run past `w * h` is error 6.

### 3.2 Palette (body of the `Palette` account)

Palette format **2** (format 1, 4-color ramps, is palette 1 for rasterizer 1; rasterizer 2 rejects it):
```
u8  format              = 2
u16 palette_version     must equal RenderRequest.palette_version
u8  outline r, g, b     the palette outline color (slot O)
u8  dark_ramp           ramp index for slot D (mouth, brows, scars) on every layer
u8  curse_ramp          ramp of the CURSE flag (3.4)
u8  cold_ramp           ramp of the COLD flag (3.4)
u8  ramp_count          1..=255
ramp_count x 18 bytes   ramp: 6 colors r g b, dark -> light. Ramp 0 = neutral gray.
u8  material_count      1..=255
material_count x 3 bytes  material: primary_ramp, secondary_ramp, flags (bit 0 = dither, 2.10)
```
The largest palette is `10 + 255 * 18 + 1 + 255 * 3 = 5366` bytes, under the 8 KiB `Palette`
account. Color 0 of each ramp is its outline color (2.9); shadows lean cold and lights warm.
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
ramp <name> #rrggbb x 6           # dark -> light; ramps are indexed in file order
dark <ramp name>
curse <ramp name>
cold <ramp name>
material <name> <primary ramp name> <secondary ramp name> [dither]   # indexed in file order
```
The first `ramp` (index 0) must be the neutral gray. Names are `[a-z0-9_]{1,32}`, unique per kind.
On-chain data and `LayerRef` carry only the indices, so a new palette keeps every index of the one
before it and only appends (palette 2 keeps the 20 ramps and 13 materials of palette 1).

### 3.3 Sprite book (body of the `SpriteBook` account)

```
u8  format    = 1
u16 book_id
u16 count
count x { u16 sprite_id, u32 offset, u16 len }   sorted by sprite_id, strictly ascending
sprite blobs  offset is from the start of the book body
```
Entries may share an offset: `art pack` stores identical sprites once. A book body is at most
64 KiB (`SpriteBook` account); `art pack` splits the sprites in id order into books 2, 3, ... (at
most 4, the `GameConfig.sprite_books` slots), and a sprite id is looked up in the books in order.

### 3.4 Render request

`RenderRequest` from `crates/types`, Borsh, 106 bytes: `palette_version: u16`,
`layers: [LayerRef; 20]` (`sprite_id: u16, material_ramp: u8, accent_ramp: u8, element_ramp: u8`),
`flags: u32`.

| Flag bit | Meaning |
|---|---|
| 0 `GRAY` | dead character: every ramp is drawn in gray, each ramp color `(r, g, b)` becomes `l = (77r + 150g + 29b) >> 8` (`ART_DESCRIPTION.md` 9.2, 11), 6 gray shades per ramp. The palette outline color is not a ramp and stays; the colored outline is a ramp color and grays; a `shadow` layer still darkens |
| 1 `CURSE` | cursed: every ramp color is blended half way (128/256) to the same color of `curse_ramp`; every outline pixel and `O` cell is `curse_ramp` color 1 |
| 2 `COLD` | chilled or frozen: every outline pixel and `O` cell is `cold_ramp` color 3 (`CURSE` wins when both are set) |
| 3..31 | reserved, must be 0 |

A ramp color is computed as: the ramp color, then the enchantment cast (section 8), then the curse
blend, then gray.

---

## 4. Composition and output

1. The canvas is `max(width) x max(height)` over the non-empty layers; every layer is aligned to the
   top left corner. All canvas pixels start transparent `(0, 0, 0, 0)`. The canvas keeps each
   pixel's ramp and ramp color until the end (so 2.11 can darken it); colors are computed last.
2. Layers are drawn in order of their index in `RenderRequest.layers` (0 first). Empty layers
   (`sprite_id == 0`) are skipped. For each layer: the overlap shadow (2.11), then its outline pixels
   (2.9), then its filled cells. Slot `O` cells use the palette outline color.
3. A layer of type `shadow` is not opaque: every filled cell blends black with alpha `90`:
   over a transparent pixel it writes `(0, 0, 0, 90)`; over a non-transparent one
   `rgb = rgb * 165 / 255` (alpha unchanged).
4. All other pixels are opaque (alpha 255). Ramp colors are gray under the `GRAY` flag (3.4).
5. Output `RAW_RGBA`: the canvas as is, `width x height` pixels, row-major, 4 bytes each.
6. Output `PNG` (`ART_DESCRIPTION.md` 13): the canvas enlarged by nearest neighbor with the integer
   scale `k = 512 / max(width, height)` (96 -> x5 = 480 px, 64 -> x8, 128 -> x4, 32 -> x16):
   - color type 3 (indexed), bit depth 8. `PLTE` holds the image colors in the order they first
     appear (row-major over the unscaled canvas); `tRNS` holds the alpha of entries `0..=last`
     non-opaque one and is omitted when every color is opaque. More than 256 colors is error 10;
   - filters: each canvas row becomes `k` identical rows, the first one with filter `None`, the
     other `k - 1` with `Up` (zeros after the filter);
   - one IDAT: zlib header `78 01`, one final fixed-Huffman deflate block, then Adler-32. At each
     position the longer repeat at distance 1 or at the row length (`width*k + 1`) is taken if it
     is 3..258 bytes (distance 1 wins a tie), otherwise a literal;
   - chunks `IHDR PLTE [tRNS] IDAT IEND`, CRC-32 each.

## 5. Errors (never panic)

The decoder rejects, with an error code:

| Code | Error |
|---|---|
| 1 | truncated input or trailing bytes |
| 2 | unsupported format version |
| 3 | canvas size not in 32/64/96/128, unknown layer type, or reserved flag bits |
| 4 | unknown opcode (0x0, 0xC..0xF) |
| 5 | slot > 6, or non-zero slot on `MIRROR_*`/`SOCKET`/`BITMAP`, or slot 0 on `SHADE`, or a `BITMAP` pixel slot that is not 0..6 or skip |
| 6 | argument out of range: rect/ellipse/bitmap with `w` or `h` 0 or past the canvas, pixel/line point outside, POLY `n` not 3..8 or vertex past `w`/`h`, mirror axis > `2*size`, shade >= the slot's colors (6, `O` 1), socket square (4x4) outside, a `BITMAP` pixel code out of range or a run past `w * h` |
| 7 | sprite id not found in any book, or book index not sorted / out of bounds |
| 8 | palette: bad format (not 2), version mismatch, ramp or material index out of range, reserved material flag bits |
| 9 | bad render request, reserved flag bits, bad output mode or scale, no sprite book |
| 10 | more than 256 colors: the image cannot be an indexed PNG |

## 6. Validator rules (`tools/art`, not the decoder)

- at most 48 commands (a `BITMAP` is one), at most 8 `SHADE`, at most 4 `SOCKET`;
- every shape at least 1 px from the canvas edge (not for `no_outline` layers);
- `BITMAP` inside the canvas (the decoder too), its uppercase letters allowed for the layer type;
- the report lists every sprite's bytes as stored in a book, and the sprites still at 64 px;
- `LINE` slopes only 0, 90, 45 degrees, 1:2 and 2:1;
- required `no_outline` for the layer types listed in 1.4; size allowed for the layer type (1.4);
- slots allowed for the layer type (body layers: `K H Y D R O _`), in commands and `BITMAP` rows;
- socket squares (4x4) inside the `P` mask and not overlapping; `U` inside the `P` mask, at least 12 px for a weapon;
- a `portrait` with a `U` zone keeps 6 px from every canvas edge (three 2 px glow bands, section 8);
- id range and unique ids and names across `assets/sprites/`;
- the same sprite rendered twice gives the same bytes.

## 7. WASM C ABI

The interface of `ART_DESCRIPTION.md` 2.2. Exports (no `wasm-bindgen`; same module in the browser,
a worker and Node):

```
alloc(len: u32) -> ptr          dealloc(ptr: u32, len: u32)
render(sprites_ptr, sprites_len, palette_ptr, palette_len, state_ptr, state_len, asset_ptr, mode)
       -> result_ptr   (all u32; 0 = error)
result_len() -> u32             result_width() -> u32      result_height() -> u32
last_error() -> u32             (section 5 code, 0 = ok)
```

- `sprites`: one or more `u32 len + sprite book (3.3)`; a sprite id is looked up in the books in
  the order given (the website passes the books of `GameConfig.sprite_books` in that order);
- `palette`: the palette body (3.2);
- `state`: the `RenderRequest` (3.4), Borsh, 106 bytes;
- `asset`: 32 bytes, the asset address (PRNG seed for effects; unused in format 2, pass zeros);
- `mode`: 0 = `RAW_RGBA`, 1 = `PNG` (section 4).

`result_width/height` are the size of the output: the canvas for `RAW_RGBA`, the enlarged image
for `PNG`. The result buffer is owned by the module and valid until the next `render` call.

## 8. Crafted look (feature `craft`, previews only)

`render_crafted(sprites, palette, state, crafts, mode)` is `render` plus a `Craft` per item layer
(`ART_DESCRIPTION.md` 9.2, 9.4, 10). It is not in `RenderRequest` or the WASM ABI yet; `tools/art`
renders the preview matrix with it, and crafting adds the same struct to the item state.

```
Craft {
  seed: [u8; 32]                 item address, the rune pattern seed
  enchant_level: u8
  runes: [Option<Rune>; 3]       insertion order, no Some after a None
  sockets_len: u8                sockets that exist, <= SOCKET points of the sprite, <= 4
  sockets: [SocketFill; 4]       Empty | Ramp(r): the gem or rune set in socket i
}
Rune { ramp: u8, darkens: bool } the rune type's ramp; darkens = a darkness rune
```

The enchantment is the layer's `element_ramp` (0 = none), so it also colors `E`. `crafts` holds
`(index in RenderRequest.layers, craft)`; each index at most once, on a non-empty item layer
(otherwise error 9). A rune or gem ramp past the palette is error 8; `sockets_len` past the
points, a rune after an empty rune slot or a body layer is error 9.

Right after a crafted layer's fill (2.9 outline, then cells), in this order, all colors grayed
under `GRAY`:

1. **Enchantment cast** (`element_ramp != 0`), drawn with the layer's fill and its colored outline:
   every `P` and `U` cell gets material color `i` blended per channel with color `i` of the
   enchantment ramp: `k = min(64 + 16 * enchant_level, 160)`, `c = (m * (256 - k) + e * k) / 256`.
2. **Runes**: the `U` cells in row-major order are split into `r` equal segments
   (segment `j` = cells `j*N/r .. (j+1)*N/r`). Segment `j` uses splitmix64 seeded with the first
   8 bytes (little-endian) of `SHA-256(seed || j as u8)`; per cell an even `next()` is carved
   (material color 1, with the cast), otherwise the rune ramp at the cell's shade.
3. **Sockets**, the first `sockets_len` points in command order: a 1 px frame around the 4x4
   square in material color 2, only on cells of the layer's `P`/`U`/`E` mask; then the square:
   empty = material color 1; filled = the fill ramp shaded as a lone 4x4 shape (2.8 with 4
   levels, colors 1..4, the top left cell is the lightest).
4. **Glow bands**: Chebyshev distance `d` from the layer's `P`/`U`/`E` mask, within the layer
   canvas. Rune `j` (0-based) covers `d = 2j+1 ..= 2j+2`, opacity 55, 40, 30 percent. The color is
   ramp color 5, or color 0 for a darkening rune. Over a transparent pixel the band writes
   `(color, opacity * 255 / 100)`; otherwise `rgb = (rgb * (100 - op) + color * op) / 100` and the
   alpha becomes the larger of the two.
