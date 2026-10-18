//! Layer composition, ramps -> RGBA, scaling (`FORMAT.md` sections 3.2, 3.4, 4).

use alloc::vec;
use alloc::vec::Vec;

use crate::request::{LayerRef, RenderRequest, FLAG_GRAY};

use crate::book::Book;
use crate::error::{Error, Result};
use crate::palette::{Palette, Rgb};
use crate::raster::{outline_mask, rasterize, Grid};
use crate::sprite::{self, layer, slot, Header};

/// Alpha of a `shadow` layer pixel over transparency, and the matching darkening factor.
pub const SHADOW_ALPHA: u32 = 90;
pub const MAX_SCALE: u32 = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    /// Row-major RGBA.
    pub rgba: Vec<u8>,
}

struct Layer {
    lref: LayerRef,
    header: Header,
    grid: Grid,
}

/// Renders a request to an unscaled RGBA image.
pub fn compose(req: &RenderRequest, palette: &Palette, books: &[Book]) -> Result<Image> {
    if req.flags & !FLAG_GRAY != 0 {
        return Err(Error::Request);
    }
    if req.palette_version != palette.palette_version {
        return Err(Error::Palette);
    }
    let mut layers = Vec::new();
    for l in req.layers.iter().filter(|l| !l.is_empty()) {
        if l.material_ramp as usize >= palette.material_count()
            || l.accent_ramp as usize >= palette.ramp_count()
            || l.element_ramp as usize >= palette.ramp_count()
        {
            return Err(Error::Palette);
        }
        let bytes = books
            .iter()
            .find_map(|b| b.get(l.sprite_id))
            .ok_or(Error::Sprite)?;
        let s = sprite::decode(bytes)?;
        layers.push(Layer {
            lref: *l,
            header: s.header,
            grid: rasterize(&s),
        });
    }
    let width = layers
        .iter()
        .map(|l| l.grid.width)
        .max()
        .ok_or(Error::Request)?;
    let height = layers
        .iter()
        .map(|l| l.grid.height)
        .max()
        .ok_or(Error::Request)?;
    let mut rgba = vec![0u8; width * height * 4];

    for l in &layers {
        let g = &l.grid;
        let put = |rgba: &mut [u8], x: usize, y: usize, c: Rgb| {
            let o = (y * width + x) * 4;
            rgba[o..o + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        };
        if !l.header.no_outline() {
            let mask = outline_mask(g);
            for (i, _) in mask.iter().enumerate().filter(|(_, &m)| m) {
                put(&mut rgba, i % g.width, i / g.width, palette.outline);
            }
        }
        let body = layer::is_body(l.header.layer_type);
        let gray = req.flags & FLAG_GRAY != 0;
        let shadow = l.header.layer_type == layer::SHADOW;
        for (i, c) in g
            .cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.slot != slot::ERASE)
        {
            let (x, y) = (i % g.width, i / g.width);
            if shadow {
                let o = (y * width + x) * 4;
                if rgba[o + 3] == 0 {
                    rgba[o..o + 4].copy_from_slice(&[0, 0, 0, SHADOW_ALPHA as u8]);
                } else {
                    for ch in &mut rgba[o..o + 3] {
                        *ch = (*ch as u32 * (255 - SHADOW_ALPHA) / 255) as u8;
                    }
                }
            } else {
                put(
                    &mut rgba,
                    x,
                    y,
                    color(palette, &l.lref, body, gray, c.slot, c.shade),
                );
            }
        }
    }

    Ok(Image {
        width,
        height,
        rgba,
    })
}

/// Gray copy of a ramp color: a dead character is drawn with every ramp in gray
/// (`ART_DESCRIPTION.md` 9.2, 11). The outline is not a ramp and keeps its color.
pub const fn gray(c: Rgb) -> Rgb {
    let l = ((77 * c[0] as u32 + 150 * c[1] as u32 + 29 * c[2] as u32) >> 8) as u8;
    [l, l, l]
}

/// Slot + shade -> color (`FORMAT.md` 1.3, 2.8, 3.2, 3.4).
fn color(p: &Palette, l: &LayerRef, body: bool, gray_ramps: bool, s: u8, shade: u8) -> Rgb {
    let n = slot::shades(s);
    let ci = if n <= 1 { 0 } else { shade * 3 / (n - 1) };
    let (primary, secondary) = p.material(l.material_ramp);
    let element = if l.element_ramp == 0 {
        primary
    } else {
        l.element_ramp
    };
    let ramp = match s {
        slot::OUTLINE => return p.outline,
        slot::PRIMARY => primary,
        slot::SECONDARY => secondary,
        slot::ACCENT => l.accent_ramp,
        slot::ELEMENT if body => p.dark_ramp,
        slot::ELEMENT => element,
        slot::RUNE if body => element,
        _ => primary, // slot::RUNE on items
    };
    let c = p.color(ramp, ci);
    if gray_ramps {
        gray(c)
    } else {
        c
    }
}

/// Nearest-neighbor integer scale, `1..=16`.
pub fn scale(img: &Image, k: u32) -> Result<Image> {
    if !(1..=MAX_SCALE).contains(&k) {
        return Err(Error::Request);
    }
    let k = k as usize;
    if k == 1 {
        return Ok(img.clone());
    }
    let (w, h) = (img.width * k, img.height * k);
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..img.height {
        let mut row = Vec::with_capacity(w * 4);
        for x in 0..img.width {
            let o = (y * img.width + x) * 4;
            for _ in 0..k {
                row.extend_from_slice(&img.rgba[o..o + 4]);
            }
        }
        for _ in 0..k {
            rgba.extend_from_slice(&row);
        }
    }
    Ok(Image {
        width: w,
        height: h,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::pack;
    use crate::palette::tests::sample;

    fn req(layers: &[(usize, LayerRef)], flags: u32) -> RenderRequest {
        let mut r = RenderRequest {
            palette_version: 1,
            flags,
            ..Default::default()
        };
        for (i, l) in layers {
            r.layers[*i] = *l;
        }
        r
    }

    fn lref(id: u16) -> LayerRef {
        LayerRef {
            sprite_id: id,
            material_ramp: 0,
            accent_ramp: 3,
            element_ramp: 0,
        }
    }

    // 32x32 item layer with one PIXEL P at (2,2); and a body layer with D at (2,2).
    fn sprites() -> Vec<u8> {
        let v = sprite::FORMAT_VERSION;
        let item = [v, 32, 32, 16, 0, 1, 0x22, 2, 2];
        let body = [v, 32, 32, 11, 0, 1, 0x25, 2, 2];
        let shadow = [v, 32, 32, 0, 1, 1, 0x12, 0, 0, 4, 4];
        let big = [v, 64, 64, 16, 1, 1, 0x22, 40, 40];
        pack(
            1,
            &[
                (1, &item[..]),
                (2, &body[..]),
                (3, &shadow[..]),
                (4, &big[..]),
            ],
        )
        .unwrap()
    }

    fn px(img: &Image, x: usize, y: usize) -> [u8; 4] {
        let o = (y * img.width + x) * 4;
        img.rgba[o..o + 4].try_into().unwrap()
    }

    #[test]
    fn colors_outline_and_dark_ramp() {
        let pal = sample();
        let p = Palette::parse(&pal).unwrap();
        let b = sprites();
        let books = [Book::parse(&b).unwrap()];
        let img = compose(&req(&[(0, lref(1))], 0), &p, &books).unwrap();
        assert_eq!((img.width, img.height), (32, 32));
        // Single pixel: L,T,R,B all foreign -> middle shade 1 of steel (ramp 1).
        assert_eq!(px(&img, 2, 2), [51, 61, 71, 255]);
        assert_eq!(px(&img, 2, 1), [0x12, 0x0c, 0x16, 255]); // outline
        assert_eq!(px(&img, 1, 1), [0, 0, 0, 0]); // diagonal: no outline
                                                  // Body layer slot 5 = D uses dark ramp 4, not the element ramp.
        let img = compose(&req(&[(0, lref(2))], 0), &p, &books).unwrap();
        assert_eq!(px(&img, 2, 2), [171, 181, 191, 255]);
    }

    #[test]
    fn layer_order_shadow_gray_and_canvas() {
        let pal = sample();
        let p = Palette::parse(&pal).unwrap();
        let b = sprites();
        let books = [Book::parse(&b).unwrap()];
        // Shadow over transparency, then an item on top of it at (2,2).
        let img = compose(&req(&[(0, lref(3)), (5, lref(1))], 0), &p, &books).unwrap();
        assert_eq!(px(&img, 0, 0), [0, 0, 0, 90]);
        assert_eq!(px(&img, 2, 2), [51, 61, 71, 255]);
        // Shadow above the item darkens it.
        let img = compose(&req(&[(0, lref(1)), (5, lref(3))], 0), &p, &books).unwrap();
        let d = |c: u32| (c * 165 / 255) as u8;
        assert_eq!(px(&img, 2, 2), [d(51), d(61), d(71), 255]);
        // Gray flag: the ramps turn gray, the outline keeps its color, the shadow still darkens.
        let img = compose(&req(&[(0, lref(1))], FLAG_GRAY), &p, &books).unwrap();
        let l = ((77 * 51 + 150 * 61 + 29 * 71) >> 8) as u8;
        assert_eq!(px(&img, 2, 2), [l, l, l, 255]);
        assert_eq!(px(&img, 2, 1), [0x12, 0x0c, 0x16, 255]);
        let img = compose(&req(&[(0, lref(1)), (5, lref(3))], FLAG_GRAY), &p, &books).unwrap();
        let g = d(l as u32);
        assert_eq!(px(&img, 2, 2), [g, g, g, 255]);
        // Canvas is the max size; smaller layers are top-left aligned.
        let img = compose(&req(&[(0, lref(1)), (1, lref(4))], 0), &p, &books).unwrap();
        assert_eq!((img.width, img.height), (64, 64));
        assert_eq!(px(&img, 40, 40)[3], 255);
    }

    #[test]
    fn rejections() {
        let pal = sample();
        let p = Palette::parse(&pal).unwrap();
        let b = sprites();
        let books = [Book::parse(&b).unwrap()];
        let r = |layers: &[(usize, LayerRef)], flags| compose(&req(layers, flags), &p, &books);
        assert_eq!(r(&[], 0), Err(Error::Request), "no layers");
        assert_eq!(r(&[(0, lref(1))], 2), Err(Error::Request), "reserved flag");
        assert_eq!(r(&[(0, lref(99))], 0), Err(Error::Sprite), "missing sprite");
        let bad = |m, a, e| LayerRef {
            sprite_id: 1,
            material_ramp: m,
            accent_ramp: a,
            element_ramp: e,
        };
        assert_eq!(r(&[(0, bad(1, 0, 0))], 0), Err(Error::Palette), "material");
        assert_eq!(r(&[(0, bad(0, 5, 0))], 0), Err(Error::Palette), "accent");
        assert_eq!(r(&[(0, bad(0, 0, 5))], 0), Err(Error::Palette), "element");
        let mut other = req(&[(0, lref(1))], 0);
        other.palette_version = 2;
        assert_eq!(
            compose(&other, &p, &books),
            Err(Error::Palette),
            "palette version"
        );
    }

    #[test]
    fn scaling() {
        let img = Image {
            width: 2,
            height: 1,
            rgba: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let s = scale(&img, 2).unwrap();
        assert_eq!((s.width, s.height), (4, 2));
        assert_eq!(
            &s.rgba[..16],
            &[1, 2, 3, 4, 1, 2, 3, 4, 5, 6, 7, 8, 5, 6, 7, 8]
        );
        assert_eq!(&s.rgba[..16], &s.rgba[16..]);
        assert_eq!(scale(&img, 0), Err(Error::Request));
        assert_eq!(scale(&img, 17), Err(Error::Request));
    }
}
