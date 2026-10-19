//! Layer composition, ramps -> RGBA, scaling (`FORMAT.md` sections 2.9-2.11, 3.2, 3.4, 4).
//!
//! Layers are drawn into an indexed canvas: a pixel keeps its ramp and ramp color until the end,
//! so a later layer can darken it by one shade and the request flags recolor it in one place.

use alloc::vec;
use alloc::vec::Vec;

use crate::request::{LayerRef, RenderRequest, FLAGS_ALL, FLAG_COLD, FLAG_CURSE, FLAG_GRAY};

use crate::book::Book;
use crate::error::{Error, Result};
use crate::palette::{Palette, Rgb};
use crate::raster::{outline_mask, rasterize, Grid};
use crate::sprite::{self, layer, slot, Cmd, Header};

/// Alpha of a `shadow` layer pixel over transparency, and the matching darkening factor.
pub const SHADOW_ALPHA: u32 = 90;
pub const MAX_SCALE: u32 = 16;
/// Curse share out of 256 on every ramp color under `FLAG_CURSE`.
pub const CURSE_SHARE: u32 = 128;
/// Ramp color of the outline under `FLAG_CURSE` (curse ramp) and `FLAG_COLD` (cold ramp).
pub const CURSE_OUTLINE: u8 = 1;
pub const COLD_OUTLINE: u8 = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    /// Row-major RGBA.
    pub rgba: Vec<u8>,
}

/// One pixel of the indexed canvas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Px {
    #[default]
    Empty,
    /// Ramp color `ci`; `cast != 0` blends it with the same color of ramp `cast` at `k / 256`
    /// (the enchantment cast, `FORMAT.md` 8).
    Ramp { ramp: u8, ci: u8, cast: u8, k: u8 },
    /// The palette outline color (slot `O`).
    Outline,
    /// A finished color (shadow layer, glow bands).
    Rgba([u8; 4]),
}

/// The indexed canvas.
pub(crate) struct Canvas {
    pub width: usize,
    pub px: Vec<Px>,
}

/// Palette plus the request flags: turns canvas pixels into colors.
#[derive(Clone, Copy)]
pub(crate) struct Ctx<'a> {
    pub p: Palette<'a>,
    pub gray: bool,
    pub curse: bool,
    pub cold: bool,
}

/// `(a * (256 - k) + b * k) / 256` per channel.
pub(crate) fn blend(a: Rgb, b: Rgb, k: u32) -> Rgb {
    core::array::from_fn(|ch| ((a[ch] as u32 * (256 - k) + b[ch] as u32 * k) / 256) as u8)
}

impl Ctx<'_> {
    /// Ramp color with the cast, then the curse, then gray (`FORMAT.md` 3.4, 8).
    pub fn rgb(&self, ramp: u8, ci: u8, cast: u8, k: u8) -> Rgb {
        let mut c = self.p.color(ramp, ci);
        if cast != 0 {
            c = blend(c, self.p.color(cast, ci), k as u32);
        }
        if self.curse {
            c = blend(c, self.p.color(self.p.curse_ramp, ci), CURSE_SHARE);
        }
        if self.gray {
            gray(c)
        } else {
            c
        }
    }

    pub fn resolve(&self, px: Px) -> [u8; 4] {
        let opaque = |c: Rgb| [c[0], c[1], c[2], 255];
        match px {
            Px::Empty => [0; 4],
            Px::Ramp { ramp, ci, cast, k } => opaque(self.rgb(ramp, ci, cast, k)),
            Px::Outline => opaque(self.p.outline),
            Px::Rgba(c) => c,
        }
    }

    /// An outline pixel next to a cell of ramp `ramp` (`FORMAT.md` 2.9): color 0 of that ramp,
    /// or the state outline under `FLAG_CURSE` / `FLAG_COLD`.
    fn outline(&self, ramp: Option<(u8, u8, u8)>) -> Px {
        if self.curse {
            return Px::Ramp {
                ramp: self.p.curse_ramp,
                ci: CURSE_OUTLINE,
                cast: 0,
                k: 0,
            };
        }
        if self.cold {
            return Px::Ramp {
                ramp: self.p.cold_ramp,
                ci: COLD_OUTLINE,
                cast: 0,
                k: 0,
            };
        }
        match ramp {
            Some((ramp, cast, k)) => Px::Ramp {
                ramp,
                ci: 0,
                cast,
                k,
            },
            None => Px::Outline,
        }
    }
}

/// One decoded and shaded layer of a request.
#[cfg_attr(not(feature = "craft"), allow(dead_code))]
pub(crate) struct Layer {
    /// Index in `RenderRequest.layers`.
    pub index: usize,
    pub lref: LayerRef,
    pub header: Header,
    pub grid: Grid,
    /// The decoded commands (crafting reads the `SOCKET` points).
    pub cmds: Vec<Cmd>,
    /// Enchantment cast on `P` and `U`: `(ramp, k)`, ramp 0 = none.
    pub cast: (u8, u8),
}

impl Layer {
    /// Ramp of a filled cell and its cast (`FORMAT.md` 1.3, 3.2); `None` for the outline slot.
    pub fn ramp(&self, p: &Palette, s: u8) -> Option<(u8, u8, u8)> {
        let l = &self.lref;
        let body = layer::is_body(self.header.layer_type);
        let (primary, secondary) = p.material(l.material_ramp);
        let element = if l.element_ramp == 0 {
            primary
        } else {
            l.element_ramp
        };
        let (cast, k) = self.cast;
        Some(match s {
            slot::PRIMARY if !body => (primary, cast, k),
            slot::PRIMARY => (primary, 0, 0),
            slot::SECONDARY => (secondary, 0, 0),
            slot::ACCENT => (l.accent_ramp, 0, 0),
            slot::ELEMENT if body => (p.dark_ramp, 0, 0),
            slot::ELEMENT => (element, 0, 0),
            slot::RUNE if body => (element, 0, 0),
            slot::RUNE => (primary, cast, k),
            _ => return None,
        })
    }
}

/// Renders a request to an unscaled RGBA image.
pub fn compose(req: &RenderRequest, palette: &Palette, books: &[Book]) -> Result<Image> {
    compose_with(req, palette, books, |_| (0, 0), |_, _, _| Ok(()))
}

/// [`compose`] with an enchantment cast per layer index and a step run on the canvas right after
/// each layer's fill (crafting, `FORMAT.md` 8).
pub(crate) fn compose_with(
    req: &RenderRequest,
    palette: &Palette,
    books: &[Book],
    cast: impl Fn(usize) -> (u8, u8),
    mut after_layer: impl FnMut(&Layer, &mut Canvas, &Ctx) -> Result<()>,
) -> Result<Image> {
    if req.flags & !FLAGS_ALL != 0 {
        return Err(Error::Request);
    }
    if req.palette_version != palette.palette_version {
        return Err(Error::Palette);
    }
    let ctx = Ctx {
        p: *palette,
        gray: req.flags & FLAG_GRAY != 0,
        curse: req.flags & FLAG_CURSE != 0,
        cold: req.flags & FLAG_COLD != 0,
    };
    let mut layers = Vec::new();
    for (index, l) in req.layers.iter().enumerate().filter(|(_, l)| !l.is_empty()) {
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
        let mut grid = rasterize(&s);
        if palette.dithers(l.material_ramp) && s.header.layer_type != layer::SHADOW {
            dither(&mut grid);
        }
        layers.push(Layer {
            index,
            lref: *l,
            header: s.header,
            grid,
            cmds: s.cmds,
            cast: cast(index),
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
    let mut cv = Canvas {
        width,
        px: vec![Px::Empty; width * height],
    };

    for l in &layers {
        let g = &l.grid;
        let shadow = l.header.layer_type == layer::SHADOW;
        if !shadow && !l.header.no_outline() {
            let mask = outline_mask(g);
            overlap_shadow(g, &mask, &mut cv);
            for (i, _) in mask.iter().enumerate().filter(|(_, &m)| m) {
                let (x, y) = ((i % g.width) as i32, (i / g.width) as i32);
                let ramp = [(-1, 0), (0, -1), (1, 0), (0, 1)]
                    .iter()
                    .filter_map(|&(dx, dy)| g.get(x + dx, y + dy))
                    .find(|c| c.slot != slot::ERASE)
                    .and_then(|c| l.ramp(palette, c.slot));
                cv.px[y as usize * width + x as usize] = ctx.outline(ramp);
            }
        }
        for (i, c) in g
            .cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.slot != slot::ERASE)
        {
            let o = (i / g.width) * width + i % g.width;
            cv.px[o] = if shadow {
                match ctx.resolve(cv.px[o]) {
                    [_, _, _, 0] => Px::Rgba([0, 0, 0, SHADOW_ALPHA as u8]),
                    [r, g, b, a] => {
                        let d = |ch: u8| (ch as u32 * (255 - SHADOW_ALPHA) / 255) as u8;
                        Px::Rgba([d(r), d(g), d(b), a])
                    }
                }
            } else {
                match l.ramp(palette, c.slot) {
                    Some((ramp, cast, k)) => Px::Ramp {
                        ramp,
                        ci: c.shade,
                        cast,
                        k,
                    },
                    None if ctx.curse || ctx.cold => ctx.outline(None),
                    None => Px::Outline,
                }
            };
        }
        after_layer(l, &mut cv, &ctx)?;
    }

    let mut rgba = Vec::with_capacity(width * height * 4);
    for &px in &cv.px {
        rgba.extend_from_slice(&ctx.resolve(px));
    }
    Ok(Image {
        width,
        height,
        rgba,
    })
}

/// `FORMAT.md` 2.10: ordered dithering on shade borders of a dithering material. A cell of a
/// material slot (`P`, `U`, `S`) on an odd `x + y` with a same-slot 4-neighbor exactly one
/// color darker takes that color. Color 1 never becomes the outline color 0.
fn dither(g: &mut Grid) {
    let src = g.clone();
    for y in 0..g.height as i32 {
        for x in 0..g.width as i32 {
            let Some(c) = src.get(x, y) else { continue };
            let material = matches!(c.slot, slot::PRIMARY | slot::SECONDARY | slot::RUNE);
            if !material || c.shade < 2 || (x + y) & 1 == 0 {
                continue;
            }
            let darker = [(-1, 0), (0, -1), (1, 0), (0, 1)].iter().any(|&(dx, dy)| {
                src.get(x + dx, y + dy)
                    .is_some_and(|o| o.slot == c.slot && o.shade + 1 == c.shade)
            });
            if darker {
                g.cells[y as usize * g.width + x as usize].shade = c.shade - 1;
            }
        }
    }
}

/// `FORMAT.md` 2.11: before a layer is drawn, canvas pixels right below its footprint (fill and
/// outline) that the layer leaves uncovered get one ramp color darker, never below color 1.
fn overlap_shadow(g: &Grid, outline: &[bool], cv: &mut Canvas) {
    let foot = |x: usize, y: usize| {
        let i = y * g.width + x;
        g.cells[i].slot != slot::ERASE || outline[i]
    };
    for y in 1..g.height {
        for x in 0..g.width {
            if !foot(x, y - 1) || foot(x, y) {
                continue;
            }
            if let Px::Ramp { ci, .. } = &mut cv.px[y * cv.width + x] {
                if *ci > 1 {
                    *ci -= 1;
                }
            }
        }
    }
}

/// Gray copy of a ramp color: a dead character is drawn with every ramp in gray
/// (`ART_DESCRIPTION.md` 9.2, 11). The palette outline color is not a ramp and keeps its color.
pub const fn gray(c: Rgb) -> Rgb {
    let l = ((77 * c[0] as u32 + 150 * c[1] as u32 + 29 * c[2] as u32) >> 8) as u8;
    [l, l, l]
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
    use crate::palette::tests::{rc, sample};
    use crate::sprite::bitmap;

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

    fn with_material(id: u16, m: u8) -> LayerRef {
        LayerRef {
            material_ramp: m,
            ..lref(id)
        }
    }

    fn bm(x: u8, y: u8, w: u8, h: u8, px: &[u8]) -> Vec<u8> {
        let mut v = vec![sprite::op::BITMAP << 4, x, y, w, h];
        v.extend(bitmap::encode(px));
        v
    }

    fn spr(size: u8, layer_type: u8, flags: u8, cmds: &[&[u8]]) -> Vec<u8> {
        let mut v = vec![sprite::FORMAT_VERSION, size, size, layer_type, flags];
        v.push(cmds.len() as u8);
        for c in cmds {
            v.extend_from_slice(c);
        }
        v
    }

    /// 1: item PIXEL P at (2,2). 2: body D at (2,2). 3: shadow 4x4 at (0,0). 4: 64x64 item.
    /// 5: item BITMAP at (2,2), 2x2: P color 4, A auto / O, S color 3.
    /// 6: item BITMAP 1x4 column of P colors 5,4,3,2 at (10,1) (dither test).
    /// 7: item 4x1 P rect at (1,5) (an upper layer for the overlap shadow).
    fn sprites() -> Vec<u8> {
        let s = |sl: u8, c: u8| bitmap::value(sl, c);
        let item = spr(32, 16, 0, &[&[0x22, 2, 2]]);
        let body = spr(32, 11, 0, &[&[0x25, 2, 2]]);
        let shadow = spr(32, 0, 1, &[&[0x12, 0, 0, 4, 4]]);
        let big = spr(64, 16, 1, &[&[0x22, 40, 40]]);
        let pixels = spr(
            32,
            16,
            0,
            &[&bm(2, 2, 2, 2, &[s(2, 5), s(4, 0), s(1, 0), s(3, 4)])],
        );
        let column = spr(
            32,
            9,
            0,
            &[&bm(
                10,
                1,
                2,
                4,
                &[
                    s(2, 6),
                    s(2, 6),
                    s(2, 5),
                    s(2, 5),
                    s(2, 4),
                    s(2, 4),
                    s(2, 3),
                    s(2, 3),
                ],
            )],
        );
        let upper = spr(32, 13, 0, &[&[0x12, 1, 5, 4, 1]]);
        pack(
            1,
            &[
                (1, &item[..]),
                (2, &body[..]),
                (3, &shadow[..]),
                (4, &big[..]),
                (5, &pixels[..]),
                (6, &column[..]),
                (7, &upper[..]),
            ],
        )
        .unwrap()
    }

    fn px(img: &Image, x: usize, y: usize) -> [u8; 4] {
        let o = (y * img.width + x) * 4;
        img.rgba[o..o + 4].try_into().unwrap()
    }

    fn op(c: [u8; 3]) -> [u8; 4] {
        [c[0], c[1], c[2], 255]
    }

    fn run(layers: &[(usize, LayerRef)], flags: u32) -> Image {
        let pal = sample();
        let p = Palette::parse(&pal).unwrap();
        let b = sprites();
        let books = [Book::parse(&b).unwrap()];
        compose(&req(layers, flags), &p, &books).unwrap()
    }

    #[test]
    fn colors_outline_and_dark_ramp() {
        let img = run(&[(0, lref(1))], 0);
        assert_eq!((img.width, img.height), (32, 32));
        // Single pixel: L,T,R,B all foreign -> middle level 1 = color 2 of steel (ramp 1).
        assert_eq!(px(&img, 2, 2), op(rc(1, 2)));
        // Colored outline: color 0 of the neighbor's ramp.
        assert_eq!(px(&img, 2, 1), op(rc(1, 0)));
        assert_eq!(px(&img, 1, 1), [0, 0, 0, 0]); // diagonal: no outline
                                                  // Body layer slot 5 = D uses dark ramp 4, not the element ramp.
        let img = run(&[(0, lref(2))], 0);
        assert_eq!(px(&img, 2, 2), op(rc(4, 2)));
        assert_eq!(px(&img, 3, 2), op(rc(4, 0)));
    }

    #[test]
    fn bitmap_pixels_slots_and_outline() {
        let img = run(&[(0, lref(5))], 0);
        assert_eq!(px(&img, 2, 2), op(rc(1, 4)), "P explicit color 4");
        // A auto alone: its L/T neighbors are foreign but R/B too -> middle.
        assert_eq!(px(&img, 3, 2), op(rc(3, 2)), "A auto, accent ramp 3");
        assert_eq!(
            px(&img, 2, 3),
            [0x12, 0x0c, 0x16, 255],
            "O is the palette outline"
        );
        assert_eq!(px(&img, 3, 3), op(rc(2, 3)), "S explicit color 3, leather");
        // Outline left of (2,3) touches O first: palette outline; above (3,2) touches A.
        assert_eq!(px(&img, 1, 3), [0x12, 0x0c, 0x16, 255]);
        assert_eq!(px(&img, 3, 1), op(rc(3, 0)));
    }

    #[test]
    fn dithering_only_for_dithering_materials() {
        // Column x = 10..12, colors 5,4,3,2 from the top. Material 1 dithers (leather ramp 2).
        let plain = run(&[(0, lref(6))], 0);
        let dith = run(&[(0, with_material(6, 1))], 0);
        // Odd x + y with a one-darker neighbor drops one color: (10,1) 5 -> 4, (11,2) 4 -> 3.
        assert_eq!(px(&plain, 10, 1), op(rc(1, 5)));
        assert_eq!(px(&dith, 10, 1), op(rc(2, 4)));
        assert_eq!(px(&dith, 11, 1), op(rc(2, 5)), "even x + y stays");
        assert_eq!(px(&dith, 11, 2), op(rc(2, 3)));
        assert_eq!(px(&dith, 10, 2), op(rc(2, 4)));
        // Color 2 at the bottom never drops to the outline color.
        assert_eq!(px(&dith, 11, 4), op(rc(2, 2)));
    }

    #[test]
    fn overlap_shadow_darkens_one_color() {
        // A lone fill at (1, 0) of a 4x4 layer over a canvas of color 3, (1, 2) already color 1.
        let mut g = Grid::new(4, 4);
        g.cells[1] = crate::raster::Cell {
            slot: 2,
            shade: 3,
            explicit: false,
        };
        let mask = outline_mask(&g);
        let ramp = |ci| Px::Ramp {
            ramp: 1,
            ci,
            cast: 0,
            k: 0,
        };
        let mut cv = Canvas {
            width: 4,
            px: vec![ramp(3); 16],
        };
        cv.px[2 * 4 + 1] = ramp(1);
        overlap_shadow(&g, &mask, &mut cv);
        // Footprint: fill (1,0), outline (0,0) (2,0) (1,1). Darkened: right below the
        // footprint and outside it: (0,1), (2,1) and (1,2); (1,2) is color 1 and stays.
        assert_eq!(cv.px[4], ramp(2));
        assert_eq!(cv.px[4 + 2], ramp(2));
        assert_eq!(cv.px[2 * 4 + 1], ramp(1));
        assert_eq!(cv.px[1], ramp(3), "inside the footprint: untouched");
        assert_eq!(cv.px[4 + 1], ramp(3), "the outline cell itself: untouched");
        assert_eq!(cv.px[3 * 4 + 1], ramp(3));
    }

    #[test]
    fn overlap_shadow_in_compose() {
        // Lower: the rect (7) at y 5, x 1..5. Upper: the bitmap (5) at x 2..4, y 2..4; its
        // outline row is y 4, so the rect cells at x 2 and 3 get one color darker.
        let alone = run(&[(0, lref(7))], 0);
        let both = run(&[(0, lref(7)), (1, lref(5))], 0);
        let darker = |c: [u8; 4]| {
            let ci = (1..6).find(|&i| op(rc(1, i)) == c).unwrap();
            op(rc(1, ci - 1))
        };
        for x in 2..4 {
            assert_eq!(px(&both, x, 5), darker(px(&alone, x, 5)), "x {x}");
        }
        assert_eq!(
            px(&both, 1, 5),
            px(&alone, 1, 5),
            "not under the upper layer"
        );
    }

    #[test]
    fn layer_order_shadow_flags_and_canvas() {
        // Shadow over transparency, then an item on top of it at (2,2).
        let img = run(&[(0, lref(3)), (5, lref(1))], 0);
        assert_eq!(px(&img, 0, 0), [0, 0, 0, 90]);
        assert_eq!(px(&img, 2, 2), op(rc(1, 2)));
        // Shadow above the item darkens it.
        let img = run(&[(0, lref(1)), (5, lref(3))], 0);
        let d = |c: u8| (c as u32 * 165 / 255) as u8;
        let c = rc(1, 2);
        assert_eq!(px(&img, 2, 2), [d(c[0]), d(c[1]), d(c[2]), 255]);
        // Gray flag: the ramps (and the colored outline) turn gray, the palette outline stays.
        let img = run(&[(0, lref(5))], FLAG_GRAY);
        assert_eq!(px(&img, 2, 2), op(gray(rc(1, 4))));
        assert_eq!(px(&img, 3, 1), op(gray(rc(3, 0))));
        assert_eq!(px(&img, 2, 3), [0x12, 0x0c, 0x16, 255]);
        // Curse: every ramp half way to the curse ramp (4), outline curse color 1, O too.
        let img = run(&[(0, lref(5))], FLAG_CURSE);
        assert_eq!(px(&img, 2, 2), op(blend(rc(1, 4), rc(4, 4), 128)));
        let curse_outline = op(blend(rc(4, 1), rc(4, 1), 128));
        assert_eq!(px(&img, 3, 1), curse_outline);
        assert_eq!(px(&img, 2, 3), curse_outline);
        // Cold: only the outline, cold ramp (3) color 3. Curse and gray together.
        let img = run(&[(0, lref(5))], FLAG_COLD);
        assert_eq!(px(&img, 2, 2), op(rc(1, 4)));
        assert_eq!(px(&img, 3, 1), op(rc(3, 3)));
        assert_eq!(px(&img, 2, 3), op(rc(3, 3)));
        let img = run(&[(0, lref(5))], FLAG_CURSE | FLAG_GRAY);
        assert_eq!(px(&img, 2, 2), op(gray(blend(rc(1, 4), rc(4, 4), 128))));
        // Canvas is the max size; smaller layers are top-left aligned.
        let img = run(&[(0, lref(1)), (1, lref(4))], 0);
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
        assert_eq!(r(&[(0, lref(1))], 8), Err(Error::Request), "reserved flag");
        assert_eq!(r(&[(0, lref(99))], 0), Err(Error::Sprite), "missing sprite");
        let bad = |m, a, e| LayerRef {
            sprite_id: 1,
            material_ramp: m,
            accent_ramp: a,
            element_ramp: e,
        };
        assert_eq!(r(&[(0, bad(2, 0, 0))], 0), Err(Error::Palette), "material");
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
