//! Preview rendering: sprites and composed characters through `deadroll_render::render`.

use std::collections::BTreeMap;

use deadroll_render::sprite::layer;
use deadroll_render::text::{compile_palette, PaletteSrc};
use deadroll_render::{book, encode_input, png, render, LayerRef, RenderRequest, OUT_RGBA};

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
    } else if name.starts_with("tile_") || name.contains("tombstone") {
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

/// Sprite books for rendering: one book holding every compiled sprite.
pub fn pack_book(sprites: &[Compiled]) -> Result<Vec<u8>, String> {
    let items: Vec<(u16, &[u8])> = sprites
        .iter()
        .map(|c| (c.src.id, c.src.bytes.as_slice()))
        .collect();
    book::pack(1, &items).map_err(|e| format!("book: {e:?}"))
}

fn run(pal: &Pal, book_bytes: &[u8], req: &RenderRequest, scale: u32) -> Result<Img, String> {
    let input = encode_input(&req.encode(), &[0u8; 32], &pal.src.bytes, &[book_bytes]);
    let out = render(&input, OUT_RGBA, scale).map_err(|e| format!("render: {e:?}"))?;
    Ok(Img {
        w: out.width as usize,
        h: out.height as usize,
        rgba: out.bytes,
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

/// The three demo characters: (title, layers).
pub fn characters() -> Vec<(&'static str, Vec<Entry>)> {
    let body_h = |ty, id| (ty, id, lk("human", EYES));
    let body_e = |ty, id| (ty, id, lk("elf", EYES));
    let warrior = vec![
        (0, 1, lk("stone", GOLD)),
        (1, 100, lk("cape", GOLD)),
        (4, 101, lk("steel", GOLD)),
        body_h(2, 2),
        body_h(5, 3),
        (6, 102, lk("leather", GOLD)),
        (7, 103, lk("leather", GOLD)),
        body_h(8, 4),
        (9, 104, lk("steel", GOLD)),
        body_h(11, 5),
        body_h(12, 6),
        (13, 105, lk("steel", GOLD)),
        body_h(14, 7),
        (16, 106, lk("steel", GOLD)),
    ];
    let mage = vec![
        (0, 1, lk("stone", GOLD)),
        (1, 205, lk("leather", GOLD)),
        body_e(2, 20),
        body_e(5, 21),
        (6, 202, lk("robe", GOLD)),
        (7, 203, lk("leather", GOLD)),
        body_e(8, 22),
        (9, 201, lk("robe", GOLD)),
        body_e(11, 23),
        body_e(12, 24),
        (13, 200, lk("robe", GOLD)),
        body_e(14, 25),
        (16, 204, lk("leather", GOLD)),
    ];
    let support = vec![
        (0, 1, lk("stone", GOLD)),
        (1, 304, lk("leather", GOLD)),
        body_h(2, 2),
        body_h(5, 3),
        (7, 302, lk("leather", GOLD)),
        body_h(8, 4),
        (9, 301, lk("steel", GOLD)),
        (10, 305, lk("steel", GOLD)),
        body_h(11, 5),
        body_h(12, 6),
        (13, 300, lk("steel", GOLD)),
        body_h(14, 7),
        (16, 303, lk("steel", GOLD)),
    ];
    vec![
        ("human_warrior", warrior),
        ("elf_mage", mage),
        ("human_support", support),
    ]
}

/// Renders one composed character. Fails if a sprite id is missing from the book.
pub fn render_character(
    pal: &Pal,
    book_bytes: &[u8],
    layers: &[Entry],
    scale: u32,
) -> Result<Img, String> {
    let l: Vec<(usize, u16, Look)> = layers
        .iter()
        .map(|&(t, id, lk)| (t as usize, id, lk))
        .collect();
    run(pal, book_bytes, &request(pal, &l)?, scale)
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

pub fn to_png(img: &Img) -> Vec<u8> {
    png::encode(img.w, img.h, &img.rgba)
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
