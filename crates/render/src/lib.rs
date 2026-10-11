//! Deadroll sprite rasterizer and PNG encoder. Integers only, `no_std` + `alloc`.
//!
//! The contract (DSL, binary formats, rules, ABI) is `crates/render/FORMAT.md`.
//! The same code runs natively (tools, tests) and as WASM (`crates/render/wasm`), so the output
//! must be bit-identical everywhere: no floats, no time, no randomness, no hash maps.
#![no_std]

extern crate alloc;

pub mod book;
pub mod compose;
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

/// `out_mode` of [`render`].
pub const OUT_RGBA: u32 = 0;
pub const OUT_PNG: u32 = 1;
/// Asset address bytes in the render input (PRNG seed for effects).
pub const ASSET_LEN: usize = 32;

/// Result of [`render`]: RGBA pixels or a PNG file, and the scaled size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Parsed render input (`FORMAT.md` 7).
pub struct Input<'a> {
    pub request: &'a [u8],
    pub asset: &'a [u8],
    pub palette: &'a [u8],
    pub books: Vec<&'a [u8]>,
}

fn take<'a>(b: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
    if b.len() < n {
        return Err(Error::Truncated);
    }
    let (head, tail) = b.split_at(n);
    *b = tail;
    Ok(head)
}

fn take_prefixed<'a>(b: &mut &'a [u8]) -> Result<&'a [u8]> {
    let len = take(b, 4)?;
    let len = u32::from_le_bytes([len[0], len[1], len[2], len[3]]) as usize;
    take(b, len)
}

/// Splits the input layout of `FORMAT.md` 7.
pub fn parse_input(mut b: &[u8]) -> Result<Input<'_>> {
    let request = take_prefixed(&mut b)?;
    let asset = take(&mut b, ASSET_LEN)?;
    let palette = take_prefixed(&mut b)?;
    let mut books = Vec::new();
    while !b.is_empty() {
        books.push(take_prefixed(&mut b)?);
    }
    if books.is_empty() {
        return Err(Error::Request);
    }
    Ok(Input {
        request,
        asset,
        palette,
        books,
    })
}

/// Builds the input layout of `FORMAT.md` 7.
pub fn encode_input(
    request: &[u8],
    asset: &[u8; ASSET_LEN],
    palette: &[u8],
    books: &[&[u8]],
) -> Vec<u8> {
    let mut v = Vec::new();
    let put = |v: &mut Vec<u8>, part: &[u8]| {
        v.extend_from_slice(&(part.len() as u32).to_le_bytes());
        v.extend_from_slice(part);
    };
    put(&mut v, request);
    v.extend_from_slice(asset);
    put(&mut v, palette);
    for b in books {
        put(&mut v, b);
    }
    v
}

/// The whole pipeline: input bytes -> RGBA or PNG. The WASM `render` export calls this.
pub fn render(input: &[u8], out_mode: u32, scale_by: u32) -> Result<Output> {
    if out_mode > OUT_PNG {
        return Err(Error::Request);
    }
    let input = parse_input(input)?;
    let request = RenderRequest::decode(input.request)?;
    let palette = Palette::parse(input.palette)?;
    let mut books = Vec::with_capacity(input.books.len());
    for b in &input.books {
        books.push(Book::parse(b)?);
    }
    let img = scale(&compose(&request, &palette, &books)?, scale_by)?;
    let bytes = if out_mode == OUT_PNG {
        png::encode(img.width, img.height, &img.rgba)
    } else {
        img.rgba
    };
    Ok(Output {
        bytes,
        width: img.width as u32,
        height: img.height as u32,
    })
}
