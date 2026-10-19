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

/// Default colors for a lone sprite, chosen from its name and layer type.
pub fn default_look(name: &str, layer_type: u8) -> Look {
    let material = if name.contains("elf") {
        "elf"
    } else if name.contains("wolf") {
        "wolf"
    } else if name.contains("goblin") {
        "goblin"
    } else if name.contains("cape") {
        "cape"
    } else if name.starts_with("mage_") && !name.contains("staff") {
        "robe"
    } else if name.starts_with("warrior_") {
        match name {
            "warrior_pants" | "warrior_boots" => "leather",
            _ => "steel",
        }
    } else if name.starts_with("support_") {
        "leather"
    } else if name.starts_with("tile_") {
        match name {
            "tile_wall" => "tile_wall",
            "tile_water" => "tile_water",
            "tile_door" => "tile_door",
            _ => "tile_floor",
        }
    } else if name.contains("tombstone") {
        "stone"
    } else if layer::is_body(layer_type) {
        "human"
    } else {
        "leather"
    };
    Look {
        material,
        accent: if layer::is_body(layer_type) {
            EYES
        } else if name == "tile_water" {
            COLD
        } else if name == "tile_door" {
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

/// Id of the book `art pack` builds: book 2 holds sprite format 2 (book 1, format 1, is frozen on-chain).
pub const BOOK_ID: u16 = 2;

/// Sprite books for rendering and upload: one book holding every compiled sprite.
pub fn pack_book(sprites: &[Compiled]) -> Result<Vec<u8>, String> {
    let items: Vec<(u16, &[u8])> = sprites
        .iter()
        .map(|c| (c.src.id, c.src.bytes.as_slice()))
        .collect();
    book::pack(BOOK_ID, &items).map_err(|e| format!("book: {e:?}"))
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
    let lt = c.sprite.header.layer_type;
    let mut look = default_look(&c.src.name, lt);
    if let Some(m) = material {
        look.material = m;
    }
    let idx = if (lt as usize) < 20 { lt as usize } else { 0 };
    run(
        pal,
        book_bytes,
        &request(pal, &[(idx, c.src.id, look)])?,
        scale,
    )
}

/// One kit entry: layer type, sprite id, look.
type Entry = (u8, u16, Look);

const fn lk(material: &'static str, accent: &'static str) -> Look {
    Look {
        material,
        accent,
        element: COLD,
    }
}

/// Body layers of a race: (layer type, sprite id).
const HUMAN: [(u8, u16); 6] = [(2, 2), (5, 3), (8, 4), (11, 5), (12, 6), (14, 7)];
const ELF: [(u8, u16); 6] = [(2, 20), (5, 21), (8, 22), (11, 23), (12, 24), (14, 25)];

/// A kit item: layer type, sprite id, material.
type KitItem = (u8, u16, &'static str);

/// Class kits.
const KITS: [(&str, &[KitItem]); 3] = [
    (
        "warrior",
        &[
            (1, 100, "cape"),
            (4, 101, "steel"),
            (6, 102, "leather"),
            (7, 103, "leather"),
            (9, 104, "steel"),
            (13, 105, "steel"),
            (16, 106, "steel"),
        ],
    ),
    (
        "mage",
        &[
            (1, 205, "leather"),
            (6, 202, "robe"),
            (7, 203, "leather"),
            (9, 201, "robe"),
            (13, 200, "robe"),
            (16, 204, "leather"),
        ],
    ),
    (
        "support",
        &[
            (1, 304, "leather"),
            (7, 302, "leather"),
            (9, 301, "leather"),
            (10, 305, "leather"),
            (13, 300, "leather"),
            (16, 303, "leather"),
        ],
    ),
];

fn body(race: &str) -> Vec<Entry> {
    let (layers, mat) = if race == "elf" {
        (ELF, "elf")
    } else {
        (HUMAN, "human")
    };
    let mut v = vec![(0, 1, lk("stone", GOLD))];
    v.extend(layers.iter().map(|&(ty, id)| (ty, id, lk(mat, EYES))));
    v
}

/// Preview figures: each bare body, then every kit on each body: (title, layers).
pub fn characters() -> Vec<(String, Vec<Entry>)> {
    let mut out = Vec::new();
    for race in ["human", "elf"] {
        out.push((race.to_string(), body(race)));
    }
    for (kit, items) in KITS {
        for race in ["human", "elf"] {
            let mut layers = body(race);
            layers.extend(items.iter().map(|&(ty, id, m)| (ty, id, lk(m, GOLD))));
            layers.sort_by_key(|l| l.0);
            out.push((format!("{race}_{kit}"), layers));
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
    let character_layer = (lt as usize) < 19;
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
