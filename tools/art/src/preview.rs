//! Preview rendering: sprites and composed characters through `deadroll_render::render`.

use std::collections::BTreeMap;

use deadroll_render::craft::{Craft, Rune, SocketFill, SOCKETS_MAX};
use deadroll_render::raster::rasterize;
use deadroll_render::sprite::{layer, slot, Cmd, SOCKET_SIZE};
use deadroll_render::text::{compile_palette, PaletteSrc};
use deadroll_render::{
    book, encode_books, render, render_crafted, scale, Image, LayerRef, Output, OutputMode,
    RenderRequest, ASSET_LEN, FLAG_GRAY,
};

use crate::validate::Compiled;
use deadroll_rules::config::ItemTuning;
use deadroll_rules::items::starter_kit;
use deadroll_types::{Class, EquipSlot, ItemBase};

pub type Rgb = [u8; 3];

pub const SHEET_BG: Rgb = [0x6a, 0x6c, 0x78];

/// A decoded preview image (unflattened RGBA).
pub struct Img {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

/// Palette source plus the compiled bytes.
pub struct Pal {
    pub src: PaletteSrc,
}

impl Pal {
    pub fn load(text: &str) -> Result<Self, String> {
        compile_palette(text)
            .map(|src| Self { src })
            .map_err(|e| format!("palette: {e}"))
    }

    fn ramp(&self, name: &str) -> Result<u8, String> {
        self.src
            .ramp(name)
            .ok_or_else(|| format!("unknown ramp `{name}`"))
    }

    fn material(&self, name: &str) -> Result<u8, String> {
        self.src
            .material(name)
            .ok_or_else(|| format!("unknown material `{name}`"))
    }
}

/// How a layer is colored in a preview.
#[derive(Clone, Copy)]
pub struct Look {
    pub material: &'static str,
    pub accent: &'static str,
    pub element: &'static str,
}

const EYES: &str = "eyes";
const GOLD: &str = "gold";
const COLD: &str = "cold";

/// Colors of each monster sprite (material, accent for `Y`, element for `R`); the app keeps the
/// same table (`app/src/chain/appearance.ts`, `MONSTER_SPRITES`).
pub const MONSTER_LOOKS: &[(&str, Look)] = &[
    ("wolf", lk3("wolf_dark", "glow_red", "fur")),
    ("goblin", lk3("goblin_dark", "glow_red", "steel")),
    ("goblin_chief", lk3("goblin_dark", "gold", "steel")),
    ("goblin_warrior", lk3("goblin_dark", "glow_red", "steel")),
    ("goblin_shaman", lk3("goblin_dark", "glow_toxic", "bone")),
    ("bear", lk3("bear", "glow_red", "bone")),
    ("treant", lk3("treant", "glow_toxic", "bone")),
    ("wood_fairy", lk3("fairy", "glow_soul", "glow_soul")),
    ("troll", lk3("troll", "glow_red", "bone")),
    ("wood_fairy_king", lk3("fairy", "glow_soul", "glow_soul")),
    ("skeleton", lk3("bone", "glow_soul", "steel")),
    ("skeleton_warrior", lk3("bone", "glow_soul", "steel")),
    ("skeleton_mage", lk3("bone_robe", "fire", "fire")),
    ("skeleton_necromancer", lk3("bone_robe", "glow_toxic", "wood")),
    ("ghoul", lk3("ghoul", "glow_soul", "bone")),
];

const fn lk3(material: &'static str, accent: &'static str, element: &'static str) -> Look {
    Look {
        material,
        accent,
        element,
    }
}

/// Default colors for a lone sprite, chosen from its name and layer type.
pub fn default_look(name: &str, layer_type: u8) -> Look {
    if layer_type == layer::MONSTER {
        if let Some((_, look)) = MONSTER_LOOKS.iter().find(|(n, _)| *n == name) {
            return *look;
        }
    }
    let material = if name.contains("elf") {
        "elf"
    } else if name.contains("wolf") {
        "wolf"
    } else if name.contains("goblin") {
        "goblin"
    } else if let Some(base) = ITEM_NAMES.iter().position(|n| *n == name) {
        ITEM_MATERIALS[base]
    } else if name.starts_with("tile_") {
        match name {
            "tile_wall" => "tile_wall",
            "tile_water" => "tile_water",
            "tile_door" => "tile_door",
            _ => "tile_floor",
        }
    } else if name.contains("tombstone") {
        "stone"
    } else if name.contains("chest") {
        "chest"
    } else if layer::is_body(layer_type) {
        "human"
    } else {
        "leather"
    };
    Look {
        material,
        accent: if layer_type == layer::MONSTER {
            GOLD
        } else if layer::is_body(layer_type) {
            EYES
        } else if name == "tile_water" {
            COLD
        } else if name == "tile_door" || name.contains("chest") {
            GOLD
        } else if layer_type == layer::TILE {
            "steel"
        } else {
            GOLD
        },
        element: COLD,
    }
}

fn request(pal: &Pal, layers: &[(usize, u16, Look)]) -> Result<RenderRequest, String> {
    let mut req = RenderRequest {
        palette_version: pal.src.palette_version,
        ..Default::default()
    };
    for &(idx, id, look) in layers {
        req.layers[idx] = LayerRef {
            sprite_id: id,
            material_ramp: pal.material(look.material)?,
            accent_ramp: pal.ramp(look.accent)?,
            element_ramp: pal.ramp(look.element)?,
        };
    }
    Ok(req)
}

/// Id of the first book `art pack` builds: books 2.. hold sprite format 2 (book 1, format 1, is
/// frozen on-chain).
pub const BOOK_ID: u16 = 2;
/// `SpriteBook` body limit in programs/render.
pub const BOOK_MAX: usize = 64 * 1024;
/// `GameConfig.sprite_books` slots.
pub const BOOKS_MAX: usize = 4;

fn items(sprites: &[&Compiled]) -> Vec<(u16, Vec<u8>)> {
    sprites
        .iter()
        .map(|c| (c.src.id, c.src.bytes.clone()))
        .collect()
}

fn pack_items(id: u16, items: &[(u16, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let list: Vec<(u16, &[u8])> = items.iter().map(|(i, b)| (*i, b.as_slice())).collect();
    book::pack(id, &list).map_err(|e| format!("book: {e:?}"))
}

/// One book holding every compiled sprite, for previews (no size limit).
pub fn pack_book(sprites: &[Compiled]) -> Result<Vec<u8>, String> {
    pack_items(BOOK_ID, &items(&sprites.iter().collect::<Vec<_>>()))
}

/// Books for upload: sprites in id order, the first book id `first`, a new book (id + 1) whenever
/// the next sprite would push the body past `BOOK_MAX`. At most `BOOKS_MAX` books (the `GameConfig`
/// slots). A book frozen on a chain never changes, so changed sprites go to books with new ids.
pub fn pack_books(sprites: &[Compiled], first: u16) -> Result<Vec<Vec<u8>>, String> {
    let mut sorted: Vec<&Compiled> = sprites.iter().collect();
    sorted.sort_by_key(|c| c.src.id);
    let mut books = Vec::new();
    let mut cur: Vec<(u16, Vec<u8>)> = Vec::new();
    for c in sorted {
        cur.push((c.src.id, c.src.bytes.clone()));
        let id = first + books.len() as u16;
        if pack_items(id, &cur)?.len() > BOOK_MAX {
            let last = cur.pop().expect("just pushed");
            if cur.is_empty() {
                return Err(format!("sprite {} alone is over {BOOK_MAX} bytes", last.0));
            }
            books.push(pack_items(id, &cur)?);
            cur = vec![last];
        }
    }
    if !cur.is_empty() {
        books.push(pack_items(first + books.len() as u16, &cur)?);
    }
    if books.len() > BOOKS_MAX {
        return Err(format!(
            "{} books, GameConfig holds {BOOKS_MAX}",
            books.len()
        ));
    }
    Ok(books)
}

fn run(pal: &Pal, book_bytes: &[u8], req: &RenderRequest, k: u32) -> Result<Img, String> {
    let out = render(
        &encode_books(&[book_bytes]),
        &pal.src.bytes,
        &req.encode(),
        &[0u8; ASSET_LEN],
        OutputMode::RawRgba,
    )
    .map_err(|e| format!("render: {e:?}"))?;
    scaled(out, k)
}

/// [`run`] with crafted layers (`deadroll_render::render_crafted`).
fn run_crafted(
    pal: &Pal,
    book_bytes: &[u8],
    req: &RenderRequest,
    crafts: &[(usize, Craft)],
    k: u32,
) -> Result<Img, String> {
    let out = render_crafted(
        &encode_books(&[book_bytes]),
        &pal.src.bytes,
        &req.encode(),
        crafts,
        OutputMode::RawRgba,
    )
    .map_err(|e| format!("render: {e:?}"))?;
    scaled(out, k)
}

fn scaled(out: Output, k: u32) -> Result<Img, String> {
    let img = Image {
        width: out.width as usize,
        height: out.height as usize,
        rgba: out.bytes,
    };
    let big = scale(&img, k).map_err(|e| format!("scale {k}: {e:?}"))?;
    Ok(Img {
        w: big.width,
        h: big.height,
        rgba: big.rgba,
    })
}

/// Renders one sprite alone with its default look (or a material override).
pub fn render_sprite(
    pal: &Pal,
    book_bytes: &[u8],
    c: &Compiled,
    scale: u32,
    material: Option<&'static str>,
) -> Result<Img, String> {
    let mut look = default_look(&c.src.name, c.sprite.header.layer_type);
    if let Some(m) = material {
        look.material = m;
    }
    render_scaled(pal, book_bytes, c, look, scale)
}

/// Enlarges a picture `k` times, pixel for pixel.
pub fn upscaled(img: Img, k: u32) -> Result<Img, String> {
    if k == 1 {
        return Ok(img);
    }
    let big = scale(
        &Image {
            width: img.w,
            height: img.h,
            rgba: img.rgba,
        },
        k,
    )
    .map_err(|e| format!("scale {k}: {e:?}"))?;
    Ok(Img {
        w: big.width,
        h: big.height,
        rgba: big.rgba,
    })
}

/// Renders one sprite alone in the given colors, at one pixel scale.
pub fn render_look(pal: &Pal, book_bytes: &[u8], c: &Compiled, look: Look) -> Result<Img, String> {
    render_scaled(pal, book_bytes, c, look, 1)
}

fn render_scaled(
    pal: &Pal,
    book_bytes: &[u8],
    c: &Compiled,
    look: Look,
    scale: u32,
) -> Result<Img, String> {
    let lt = c.sprite.header.layer_type;
    let idx = if lt <= layer::CHARACTER_MAX {
        lt as usize
    } else {
        0
    };
    run(
        pal,
        book_bytes,
        &request(pal, &[(idx, c.src.id, look)])?,
        scale,
    )
}

/// One kit entry: layer type, sprite id, look.
pub type Entry = (u8, u16, Look);

const fn lk(material: &'static str, accent: &'static str) -> Look {
    Look {
        material,
        accent,
        element: COLD,
    }
}

/// Body layers of a race: (layer type, sprite id). The whole body is one sprite, hair is its own
/// (helmets cover it), and the front fist (shared by every race) is drawn over the weapon.
const HUMAN: [(u8, u16); 3] = [
    (layer::BODY, 10),
    (layer::HAIR, 11),
    (layer::FIST_FRONT, 14),
];
const ELF: [(u8, u16); 3] = [
    (layer::BODY, 12),
    (layer::HAIR, 13),
    (layer::FIST_FRONT, 14),
];

/// Item sprites, one per `ItemBase` in discriminant order: sprite id `ITEM_SPRITE_BASE + base`
/// (assets/sprites/items/items.dsl), and the material an item without its own is drawn in.
pub const ITEM_SPRITE_BASE: u16 = 100;
pub const ITEM_NAMES: [&str; 6] = [
    "sword",
    "staff",
    "wand",
    "leather_armor",
    "robe",
    "light_leather_armor",
];
pub const ITEM_MATERIALS: [&str; 6] = ["steel", "leather", "leather", "leather", "robe", "leather"];

/// The character layer an equipment slot is drawn in; slots without one draw nothing yet.
pub fn slot_layer(slot: EquipSlot) -> Option<u8> {
    Some(match slot {
        EquipSlot::MainHand => layer::MAINHAND,
        EquipSlot::OffHand => layer::OFFHAND,
        EquipSlot::Helmet => layer::HELMET,
        EquipSlot::Body => layer::ARMOR_BODY,
        EquipSlot::Pants => layer::PANTS,
        EquipSlot::Boots => layer::BOOTS,
        EquipSlot::Necklace => layer::NECKLACE,
        EquipSlot::Cloak => layer::BACK_ITEM,
        EquipSlot::Gloves | EquipSlot::Ring1 | EquipSlot::Ring2 => return None,
    })
}

/// The layers of one worn item: its sprite in its slot's layer.
pub fn worn_item(base: ItemBase) -> Option<Entry> {
    let slot = ItemTuning::DEFAULT.base(base).slot;
    let i = base as usize;
    Some((
        slot_layer(slot)?,
        ITEM_SPRITE_BASE + i as u16,
        lk(ITEM_MATERIALS[i], GOLD),
    ))
}

/// A class starter kit, worn: what `Gear::starter` gives a new character.
fn starter(class: Class) -> Vec<Entry> {
    starter_kit(class, &ItemTuning::DEFAULT)
        .filter_map(worn_item)
        .collect()
}

const CLASSES: [(&str, Class); 3] = [
    ("warrior", Class::Warrior),
    ("mage", Class::Mage),
    ("support", Class::Support),
];

fn body(race: &str) -> Vec<Entry> {
    let (layers, mat) = if race == "elf" {
        (ELF, "elf")
    } else {
        (HUMAN, "human")
    };
    let mut v = vec![(layer::SHADOW, 1, lk("stone", GOLD))];
    v.extend(layers.iter().map(|&(ty, id)| (ty, id, lk(mat, EYES))));
    v
}

/// Preview figures: each bare body, then each class starter kit worn on each body: (title, layers).
/// A character is drawn as its body, its hair and the items it wears, nothing else.
pub fn characters() -> Vec<(String, Vec<Entry>)> {
    let mut out = Vec::new();
    for race in ["human", "elf"] {
        out.push((race.to_string(), body(race)));
    }
    for (name, class) in CLASSES {
        for race in ["human", "elf"] {
            let mut layers = body(race);
            layers.extend(starter(class));
            layers.sort_by_key(|l| l.0);
            out.push((format!("{race}_{name}"), layers));
        }
    }
    out
}

/// Renders one composed character, alive or dead (gray ramps). Fails if a sprite id is missing.
pub fn render_character(
    pal: &Pal,
    book_bytes: &[u8],
    layers: &[Entry],
    scale: u32,
    dead: bool,
) -> Result<Img, String> {
    let l: Vec<(usize, u16, Look)> = layers
        .iter()
        .map(|&(t, id, lk)| (t as usize, id, lk))
        .collect();
    let mut req = request(pal, &l)?;
    if dead {
        req.flags = FLAG_GRAY;
    }
    run(pal, book_bytes, &req, scale)
}

/// Columns of the craft matrix (`ART_DESCRIPTION.md` 14 step 5), in order.
pub const MATRIX_COLUMNS: [&str; 11] = [
    "steel",
    "stone",
    "leather",
    "steel + darkness enchant",
    "steel + cold enchant level 4",
    "steel + 3 runes (fire, cold, darkness)",
    "steel + empty sockets",
    "steel + filled sockets",
    "steel + darkness + 3 runes + filled sockets",
    "debug slots (P red, S green, A yellow, E magenta, U cyan, sockets white)",
    "on the human body, full craft (character layers only)",
];

/// One row of the craft matrix for an item sprite: see [`MATRIX_COLUMNS`].
pub fn craft_row(pal: &Pal, book_bytes: &[u8], c: &Compiled, k: u32) -> Result<Vec<Img>, String> {
    let lt = c.sprite.header.layer_type;
    let character_layer = lt <= layer::CHARACTER_MAX;
    let idx = if character_layer { lt as usize } else { 0 };
    let points = c
        .sprite
        .cmds
        .iter()
        .filter(|c| matches!(c, Cmd::Socket { .. }))
        .count() as u8;
    let rune = |name: &str, darkens| -> Result<Option<Rune>, String> {
        Ok(Some(Rune {
            ramp: pal.ramp(name)?,
            darkens,
        }))
    };
    let runes = [
        rune("fire", false)?,
        rune(COLD, false)?,
        rune("dark", true)?,
    ];
    let mut gems = [SocketFill::Empty; SOCKETS_MAX];
    for (g, name) in gems.iter_mut().zip([GOLD, "green", COLD, "fire"]) {
        *g = SocketFill::Ramp(pal.ramp(name)?);
    }
    let plain = Craft {
        seed: [c.src.id as u8; 32],
        ..Default::default()
    };
    let item = |material: &str, element: &str| -> Result<LayerRef, String> {
        Ok(LayerRef {
            sprite_id: c.src.id,
            material_ramp: pal.material(material)?,
            accent_ramp: pal.ramp(GOLD)?,
            element_ramp: if element.is_empty() {
                0
            } else {
                pal.ramp(element)?
            },
        })
    };
    let one = |l: LayerRef, craft: Craft| -> Result<Img, String> {
        let mut req = request(pal, &[])?;
        req.layers[idx] = l;
        run_crafted(pal, book_bytes, &req, &[(idx, craft)], k)
    };
    let full = Craft {
        enchant_level: 2,
        runes,
        sockets_len: points,
        sockets: gems,
        ..plain
    };
    let mut row = vec![
        one(item("steel", "")?, plain)?,
        one(item("stone", "")?, plain)?,
        one(item("leather", "")?, plain)?,
        one(item("steel", "dark")?, plain)?,
        one(
            item("steel", COLD)?,
            Craft {
                enchant_level: 4,
                ..plain
            },
        )?,
        one(item("steel", "")?, Craft { runes, ..plain })?,
        one(
            item("steel", "")?,
            Craft {
                sockets_len: points,
                ..plain
            },
        )?,
        one(
            item("steel", "")?,
            Craft {
                sockets_len: points,
                sockets: gems,
                ..plain
            },
        )?,
        one(item("steel", "dark")?, full)?,
        debug_slots(c, k),
    ];
    if character_layer {
        let l: Vec<(usize, u16, Look)> = body("human")
            .iter()
            .map(|&(t, id, lk)| (t as usize, id, lk))
            .collect();
        let mut req = request(pal, &l)?;
        req.layers[idx] = item("steel", "dark")?;
        row.push(run_crafted(pal, book_bytes, &req, &[(idx, full)], k)?);
    }
    Ok(row)
}

/// The slot grid of a sprite in fixed debug colors (brighter = lighter shade), socket squares white.
pub fn debug_slots(c: &Compiled, k: u32) -> Img {
    let g = rasterize(&c.sprite);
    let mut rgba = vec![0u8; g.width * g.height * 4];
    for (i, cell) in g.cells.iter().enumerate() {
        let base: Rgb = match cell.slot {
            slot::OUTLINE => [0x12, 0x0c, 0x16],
            slot::PRIMARY => [210, 70, 70],
            slot::SECONDARY => [70, 180, 80],
            slot::ACCENT => [235, 205, 50],
            slot::ELEMENT => [205, 70, 205],
            slot::RUNE => [70, 205, 225],
            _ => continue,
        };
        let n = slot::shades(cell.slot).max(2) as u32;
        let f = 55 + 45 * cell.shade as u32 / (n - 1);
        let o = i * 4;
        for ch in 0..3 {
            rgba[o + ch] = (base[ch] as u32 * f / 100) as u8;
        }
        rgba[o + 3] = 255;
    }
    let side = SOCKET_SIZE as usize;
    for cmd in &c.sprite.cmds {
        if let Cmd::Socket { x, y } = *cmd {
            for py in y as usize..(y as usize + side).min(g.height) {
                for px in x as usize..(x as usize + side).min(g.width) {
                    let o = (py * g.width + px) * 4;
                    rgba[o..o + 4].copy_from_slice(&[245, 245, 245, 255]);
                }
            }
        }
    }
    let img = Image {
        width: g.width,
        height: g.height,
        rgba,
    };
    let big = scale(&img, k).expect("debug scale");
    Img {
        w: big.width,
        h: big.height,
        rgba: big.rgba,
    }
}

/// Alpha-blends onto a flat background, making every pixel opaque.
pub fn flatten(img: &mut Img, bg: Rgb) {
    for p in img.rgba.chunks_exact_mut(4) {
        let a = p[3] as u32;
        for i in 0..3 {
            p[i] = ((p[i] as u32 * a + bg[i] as u32 * (255 - a)) / 255) as u8;
        }
        p[3] = 255;
    }
}

/// Lays images out in a grid, `cols` per row, each in a cell of the largest image size plus a gap.
pub fn grid(images: &[Img], cols: usize, gap: usize, bg: Rgb) -> Img {
    let cw = images.iter().map(|i| i.w).max().unwrap_or(1) + gap;
    let ch = images.iter().map(|i| i.h).max().unwrap_or(1) + gap;
    let cols = cols.max(1).min(images.len().max(1));
    let rows = images.len().div_ceil(cols).max(1);
    let (w, h) = (cols * cw + gap, rows * ch + gap);
    let mut rgba = vec![255u8; w * h * 4];
    for px in rgba.chunks_exact_mut(4) {
        px[..3].copy_from_slice(&bg);
    }
    for (n, im) in images.iter().enumerate() {
        let (ox, oy) = (gap + (n % cols) * cw, gap + (n / cols) * ch);
        for y in 0..im.h {
            let src = &im.rgba[y * im.w * 4..(y + 1) * im.w * 4];
            let at = ((oy + y) * w + ox) * 4;
            rgba[at..at + im.w * 4].copy_from_slice(src);
        }
    }
    Img { w, h, rgba }
}

/// RGBA PNG for previews. Sheets can hold more than the 256 colors of the on-chain indexed
/// encoder, so previews use the `png` crate.
pub fn to_png(img: &Img) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, img.w as u32, img.h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().expect("png header");
    w.write_image_data(&img.rgba).expect("png data");
    drop(w);
    out
}

/// Reads a PNG (any color type) as RGBA.
pub fn from_png(bytes: &[u8]) -> Result<Img, String> {
    let mut dec = png::Decoder::new(bytes);
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = dec.read_info().map_err(|e| format!("png: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("png: {e}"))?;
    buf.truncate(info.buffer_size());
    let (w, h) = (info.width as usize, info.height as usize);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::GrayscaleAlpha => buf
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        other => return Err(format!("png: unexpected color type {other:?}")),
    };
    Ok(Img { w, h, rgba })
}

/// Draws `top` over `dst` at `(x, y)` with alpha (source over), clipped.
pub(crate) fn over(dst: &mut Img, top: &Img, x: i32, y: i32) {
    for ty in 0..top.h as i32 {
        for tx in 0..top.w as i32 {
            let (dx, dy) = (x + tx, y + ty);
            if dx < 0 || dy < 0 || dx >= dst.w as i32 || dy >= dst.h as i32 {
                continue;
            }
            let s = &top.rgba[(ty as usize * top.w + tx as usize) * 4..][..4];
            let o = (dy as usize * dst.w + dx as usize) * 4;
            let a = s[3] as u32;
            let px = &mut dst.rgba[o..o + 4];
            for (d, &c) in px.iter_mut().zip(&s[..3]) {
                *d = ((c as u32 * a + *d as u32 * (255 - a)) / 255) as u8;
            }
            px[3] = px[3].max(s[3]);
        }
    }
}

/// Map cell side: one tile (`FORMAT.md` 1.4).
pub const CELL: usize = 64;

/// A small map at one pixel scale: a wall row, flagstones, a trap, a chest and figures standing
/// bottom-center on their cells, the way the app draws them (`app/src/components/grid`).
/// `objects` are `(cell x, cell y, sprite id)` drawn alone; `figures` are `(cell x, cell y, layers)`.
pub fn scene(
    pal: &Pal,
    book_bytes: &[u8],
    sprites: &[Compiled],
    tiles: &[&[u16]],
    objects: &[(usize, usize, u16)],
    figures: &[(usize, usize, &[Entry])],
    k: u32,
) -> Result<Img, String> {
    let ids = by_id(sprites);
    let sprite = |id: u16| ids.get(&id).copied().ok_or(format!("sprite {id} missing"));
    let (cw, ch) = (
        tiles.iter().map(|r| r.len()).max().unwrap_or(0),
        tiles.len(),
    );
    let mut img = Img {
        w: cw * CELL,
        h: ch * CELL,
        rgba: vec![0; cw * CELL * ch * CELL * 4],
    };
    for (cy, row) in tiles.iter().enumerate() {
        for (cx, &id) in row.iter().enumerate() {
            let t = render_sprite(pal, book_bytes, sprite(id)?, 1, None)?;
            over(&mut img, &t, (cx * CELL) as i32, (cy * CELL) as i32);
        }
    }
    // Bottom-center on the cell: a canvas taller or wider than the cell overlaps its neighbors.
    let place = |img: &mut Img, t: &Img, cx: usize, cy: usize| {
        let x = (cx * CELL + CELL / 2) as i32 - t.w as i32 / 2;
        let y = ((cy + 1) * CELL) as i32 - t.h as i32;
        over(img, t, x, y);
    };
    for &(cx, cy, id) in objects {
        let t = render_sprite(pal, book_bytes, sprite(id)?, 1, None)?;
        place(&mut img, &t, cx, cy);
    }
    let mut figs: Vec<_> = figures.to_vec();
    figs.sort_by_key(|f| (f.1, f.0));
    for (cx, cy, layers) in figs {
        let t = render_character(pal, book_bytes, layers, 1, false)?;
        place(&mut img, &t, cx, cy);
    }
    let big = scale(
        &Image {
            width: img.w,
            height: img.h,
            rgba: img.rgba,
        },
        k,
    )
    .map_err(|e| format!("scale {k}: {e:?}"))?;
    Ok(Img {
        w: big.width,
        h: big.height,
        rgba: big.rgba,
    })
}

/// Parses `#rrggbb`.
pub fn parse_hex(s: &str) -> Option<Rgb> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let b = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([b(0)?, b(2)?, b(4)?])
}

/// Sprites keyed by id, for lookups.
pub fn by_id(sprites: &[Compiled]) -> BTreeMap<u16, &Compiled> {
    sprites.iter().map(|c| (c.src.id, c)).collect()
}
