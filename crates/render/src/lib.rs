//! Deadroll sprite rasterizer and PNG encoder. Integers only, `no_std` + `alloc`.
//!
//! The contract (DSL, binary formats, rules, ABI) is `crates/render/FORMAT.md`.
//! The same code runs natively (tools, tests) and as WASM (`crates/render/wasm`), so the output
//! must be bit-identical everywhere: no floats, no time, no randomness, no hash maps.
#![no_std]

extern crate alloc;

pub mod book;
pub mod compose;
#[cfg(feature = "craft")]
pub mod craft;
pub mod error;
pub mod palette;
pub mod png;
pub mod raster;
pub mod request;
pub mod sprite;
#[cfg(feature = "text")]
pub mod text;

use alloc::vec::Vec;

pub use book::Book;
pub use compose::{compose, scale, Image};
pub use error::{Error, Result};
pub use palette::Palette;
pub use request::{LayerRef, RenderRequest, FLAG_GRAY, LAYERS_MAX};

/// Asset address bytes (PRNG seed for effects, `ART_DESCRIPTION.md` 10).
pub const ASSET_LEN: usize = 32;
/// A `Png` output is enlarged by nearest neighbor up to this many pixels on the longer side.
pub const PNG_SIDE: usize = 512;

/// Output of [`render`] (`ART_DESCRIPTION.md` 2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum OutputMode {
    /// Canvas pixels as is, row-major RGBA, for a canvas on the website.
    RawRgba = 0,
    /// A finished indexed PNG enlarged to [`PNG_SIDE`] on the longer side, for wallets.
    Png = 1,
}

impl OutputMode {
    /// The WASM `mode` argument.
    pub const fn from_u32(v: u32) -> Result<Self> {
        match v {
            0 => Ok(Self::RawRgba),
            1 => Ok(Self::Png),
            _ => Err(Error::Request),
        }
    }
}

/// Result of [`render`]: RGBA pixels or a PNG file, and its size in pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Integer nearest-neighbor scale of a `Png` output: the longer side up to [`PNG_SIDE`]
/// (64 -> x8, 96 -> x5, 128 -> x4).
pub const fn png_scale(width: usize, height: usize) -> u32 {
    let side = if width > height { width } else { height };
    if side == 0 || side >= PNG_SIDE {
        1
    } else {
        (PNG_SIDE / side) as u32
    }
}

/// Splits the `sprites` argument of [`render`]: one or more `u32 len + sprite book` (`FORMAT.md` 7).
pub fn parse_books(mut b: &[u8]) -> Result<Vec<&[u8]>> {
    let mut books = Vec::new();
    while !b.is_empty() {
        if b.len() < 4 {
            return Err(Error::Truncated);
        }
        let len = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
        let rest = &b[4..];
        if rest.len() < len {
            return Err(Error::Truncated);
        }
        books.push(&rest[..len]);
        b = &rest[len..];
    }
    if books.is_empty() {
        return Err(Error::Request);
    }
    Ok(books)
}

/// Builds the `sprites` argument of [`render`] from sprite book bodies.
pub fn encode_books(books: &[&[u8]]) -> Vec<u8> {
    let mut v = Vec::new();
    for b in books {
        v.extend_from_slice(&(b.len() as u32).to_le_bytes());
        v.extend_from_slice(b);
    }
    v
}

/// The rasterizer (`ART_DESCRIPTION.md` 2.2). The WASM `render` export calls this.
///
/// - `sprites`: the sprite books holding the needed layers, each as `u32 len + book` (`FORMAT.md` 3.3);
///   a sprite id is looked up in the books in the order given;
/// - `palette`: the master palette and material table (`FORMAT.md` 3.2);
/// - `state`: the character or item state, a Borsh `RenderRequest` (`FORMAT.md` 3.4);
/// - `asset`: the asset address, the PRNG seed of procedural effects (unused in format 2).
pub fn render(
    sprites: &[u8],
    palette: &[u8],
    state: &[u8],
    asset: &[u8; ASSET_LEN],
    mode: OutputMode,
) -> Result<Output> {
    let _ = asset;
    let request = RenderRequest::decode(state)?;
    let palette = Palette::parse(palette)?;
    let mut books = Vec::new();
    for b in parse_books(sprites)? {
        books.push(Book::parse(b)?);
    }
    let img = compose(&request, &palette, &books)?;
    output(img, mode)
}

/// [`render`] with crafted item layers (`FORMAT.md` 8): `crafts` holds
/// `(index in RenderRequest.layers, craft)`. Tools only until crafting adds it to the request.
#[cfg(feature = "craft")]
pub fn render_crafted(
    sprites: &[u8],
    palette: &[u8],
    state: &[u8],
    crafts: &[(usize, craft::Craft)],
    mode: OutputMode,
) -> Result<Output> {
    let request = RenderRequest::decode(state)?;
    let palette = Palette::parse(palette)?;
    let mut books = Vec::new();
    for b in parse_books(sprites)? {
        books.push(Book::parse(b)?);
    }
    let img = craft::compose_crafted(&request, &palette, &books, crafts)?;
    output(img, mode)
}

fn output(img: Image, mode: OutputMode) -> Result<Output> {
    Ok(match mode {
        OutputMode::RawRgba => Output {
            width: img.width as u32,
            height: img.height as u32,
            bytes: img.rgba,
        },
        OutputMode::Png => {
            let k = png_scale(img.width, img.height);
            Output {
                bytes: png::encode(&img, k)?,
                width: img.width as u32 * k,
                height: img.height as u32 * k,
            }
        }
    })
}
