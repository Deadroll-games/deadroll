//! Crafted look of an item layer (`FORMAT.md` 8; `ART_DESCRIPTION.md` 9.2, 9.4, 10):
//! enchantment color cast, runes in the `U` zone, sockets at the `SOCKET` points and rune glow
//! bands. Not in `RenderRequest` or the WASM ABI yet: tools render previews with it, and crafting
//! adds the same [`Craft`] to the item state.

use alloc::vec::Vec;

use sha2::{Digest, Sha256};

use crate::book::Book;
use crate::compose::{compose_with, Canvas, Ctx, Image, Layer, Px};
use crate::error::{Error, Result};
use crate::palette::Palette;
use crate::request::{RenderRequest, LAYERS_MAX};
use crate::sprite::{layer, slot, Cmd, SOCKET_SIZE};

/// Runes on one item.
pub const RUNES_MAX: usize = 3;
/// Sockets on one item (and `SOCKET` points on one sprite).
pub const SOCKETS_MAX: usize = 4;
/// Width of the glow band of one rune: rune `i` (0-based) covers distances `2i+1 ..= 2i+2`.
pub const RUNE_BAND: u8 = 2;
/// Band opacity in percent, rune 1 to 3.
pub const BAND_OPACITY: [u32; RUNES_MAX] = [55, 40, 30];
/// Enchantment share out of 256: `min(BASE + STEP * enchant_level, MAX)`.
pub const ENCHANT_BASE: u32 = 64;
pub const ENCHANT_STEP: u32 = 16;
pub const ENCHANT_MAX: u32 = 160;
/// Ramp colors of the crafted look (6-color ramps): a carved rune pixel, an empty socket, the
/// socket frame, and the glow band of a rune (a darkness rune darkens with its darkest color).
pub const CARVED: u8 = 1;
pub const SOCKET_EMPTY: u8 = 1;
pub const SOCKET_FRAME: u8 = 2;
pub const BAND_LIGHT: u8 = 5;
pub const BAND_DARK: u8 = 0;

/// One inscribed rune: its type is the ramp of its element. A darkness rune darkens instead of glowing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rune {
    pub ramp: u8,
    pub darkens: bool,
}

/// What is set in a socket.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SocketFill {
    #[default]
    Empty,
    /// A gem or a rune, drawn in this ramp.
    Ramp(u8),
}

/// The crafted state of one item layer (`ART_DESCRIPTION.md` 9.2). The enchantment is the layer's
/// `element_ramp` (0 = none), so it also colors the core `E`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Craft {
    /// Item address: the rune pattern seed.
    pub seed: [u8; 32],
    pub enchant_level: u8,
    /// In insertion order; no `Some` after a `None`.
    pub runes: [Option<Rune>; RUNES_MAX],
    /// Sockets that exist; they take the sprite's `SOCKET` points in order.
    pub sockets_len: u8,
    pub sockets: [SocketFill; SOCKETS_MAX],
}

/// `splitmix64` (`ART_DESCRIPTION.md` 10).
struct SplitMix(u64);

impl SplitMix {
    /// Seed: the first 8 bytes of `SHA-256(item address || rune index)`, little-endian.
    fn rune(seed: &[u8; 32], index: u8) -> Self {
        let d = Sha256::new()
            .chain_update(seed)
            .chain_update([index])
            .finalize();
        let mut b = [0u8; 8];
        b.copy_from_slice(&d[..8]);
        Self(u64::from_le_bytes(b))
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Renders a request like [`crate::compose`], then applies each craft to its layer right after the
/// layer's fill. `crafts` holds `(index in RenderRequest.layers, craft)`, each index at most once.
pub fn compose_crafted(
    req: &RenderRequest,
    palette: &Palette,
    books: &[Book],
    crafts: &[(usize, Craft)],
) -> Result<Image> {
    for (n, &(i, _)) in crafts.iter().enumerate() {
        if i >= LAYERS_MAX || req.layers[i].is_empty() || crafts[..n].iter().any(|c| c.0 == i) {
            return Err(Error::Request);
        }
    }
    let craft = |i: usize| crafts.iter().find(|c| c.0 == i).map(|c| &c.1);
    // The enchantment is the layer's element ramp; the cast is drawn with the layer's fill.
    let cast = |i: usize| match craft(i) {
        Some(c) if req.layers[i].element_ramp != 0 => {
            let k = (ENCHANT_BASE + ENCHANT_STEP * c.enchant_level as u32).min(ENCHANT_MAX);
            (req.layers[i].element_ramp, k as u8)
        }
        _ => (0, 0),
    };
    compose_with(req, palette, books, cast, |l, cv, ctx| {
        match craft(l.index) {
            Some(c) => apply(l, c, ctx, cv),
            None => Ok(()),
        }
    })
}

fn runes(c: &Craft) -> Result<Vec<Rune>> {
    let n = c.runes.iter().take_while(|r| r.is_some()).count();
    if c.runes[n..].iter().any(|r| r.is_some()) {
        return Err(Error::Request);
    }
    Ok(c.runes[..n].iter().flatten().copied().collect())
}

fn is_item_mask(s: u8) -> bool {
    matches!(s, slot::PRIMARY | slot::RUNE | slot::ELEMENT)
}

/// Auto shading level of a lone square of side `n` (`FORMAT.md` 2.8 with 4 levels): the top
/// left corner is the lightest.
fn square_level(cx: u8, cy: u8, n: u8) -> u8 {
    let (l, t, r, b) = (cx == 0, cy == 0, cx == n - 1, cy == n - 1);
    if (l || t) && !(r || b) {
        if l && t {
            3
        } else {
            2
        }
    } else if (r || b) && !(l || t) {
        0
    } else {
        1
    }
}

fn apply(l: &Layer, c: &Craft, ctx: &Ctx, cv: &mut Canvas) -> Result<()> {
    if layer::is_body(l.header.layer_type) {
        return Err(Error::Request);
    }
    let p = &ctx.p;
    let runes = runes(c)?;
    let ramps = p.ramp_count();
    let fills = c.sockets.iter().filter_map(|s| match s {
        SocketFill::Ramp(r) => Some(*r),
        SocketFill::Empty => None,
    });
    if runes
        .iter()
        .map(|r| r.ramp)
        .chain(fills)
        .any(|r| r as usize >= ramps)
    {
        return Err(Error::Palette);
    }
    let points: Vec<(u8, u8)> = l
        .cmds
        .iter()
        .filter_map(|c| match *c {
            Cmd::Socket { x, y } => Some((x, y)),
            _ => None,
        })
        .collect();
    let sockets = c.sockets_len as usize;
    if sockets > SOCKETS_MAX || sockets > points.len() {
        return Err(Error::Request);
    }

    let g = &l.grid;
    let (gw, gh) = (g.width, g.height);
    let (primary, _) = p.material(l.lref.material_ramp);
    let (cast, k) = l.cast;
    // Material color `i` of slots P and U, with the enchantment cast (ART_DESCRIPTION 9.2).
    let mat = |ci: u8| Px::Ramp {
        ramp: primary,
        ci,
        cast,
        k,
    };
    let plain = |ramp: u8, ci: u8| Px::Ramp {
        ramp,
        ci,
        cast: 0,
        k: 0,
    };
    let width = cv.width;
    let at = |x: usize, y: usize| y * width + x;

    // 1. Runes: the U zone, row by row, in equal segments; each pixel carved or glowing.
    if !runes.is_empty() {
        let zone: Vec<usize> = (0..g.cells.len())
            .filter(|&i| g.cells[i].slot == slot::RUNE)
            .collect();
        let r = runes.len();
        for (j, rune) in runes.iter().enumerate() {
            let mut rng = SplitMix::rune(&c.seed, j as u8);
            for &i in &zone[j * zone.len() / r..(j + 1) * zone.len() / r] {
                cv.px[at(i % gw, i / gw)] = if rng.next().is_multiple_of(2) {
                    mat(CARVED)
                } else {
                    plain(rune.ramp, g.cells[i].shade)
                };
            }
        }
    }

    // 2. Sockets: a 1 px frame (on the item only), then the square.
    let n = SOCKET_SIZE;
    for (&(x, y), fill) in points.iter().zip(c.sockets.iter()).take(sockets) {
        for fy in y as i32 - 1..=(y + n) as i32 {
            for fx in x as i32 - 1..=(x + n) as i32 {
                if g.get(fx, fy).is_some_and(|c| is_item_mask(c.slot)) {
                    cv.px[at(fx as usize, fy as usize)] = mat(SOCKET_FRAME);
                }
            }
        }
        for cy in 0..n {
            for cx in 0..n {
                let ci = slot::auto_color(slot::PRIMARY, square_level(cx, cy, n));
                cv.px[at((x + cx) as usize, (y + cy) as usize)] = match *fill {
                    SocketFill::Empty => mat(SOCKET_EMPTY),
                    SocketFill::Ramp(r) => plain(r, ci),
                };
            }
        }
    }

    // 3. Glow bands: Chebyshev distance from the P/U/E mask, RUNE_BAND px per rune, outward in
    //    insertion order. Clipped to the layer canvas.
    if !runes.is_empty() {
        let reach = RUNE_BAND * runes.len() as u8;
        let mut dist: Vec<u8> = g
            .cells
            .iter()
            .map(|c| if is_item_mask(c.slot) { 0 } else { u8::MAX })
            .collect();
        for step in 1..=reach {
            let prev = dist.clone();
            for y in 0..gh {
                for x in 0..gw {
                    if prev[y * gw + x] != u8::MAX {
                        continue;
                    }
                    let near = (-1i32..=1).any(|dy| {
                        (-1i32..=1).any(|dx| {
                            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                            nx >= 0
                                && ny >= 0
                                && (nx as usize) < gw
                                && (ny as usize) < gh
                                && prev[ny as usize * gw + nx as usize] == step - 1
                        })
                    });
                    if near {
                        dist[y * gw + x] = step;
                    }
                }
            }
        }
        let alpha = |op: u32| (op * 255 / 100) as u8;
        for (i, &d) in dist.iter().enumerate() {
            if d == 0 || d > reach {
                continue;
            }
            let j = ((d - 1) / RUNE_BAND) as usize;
            let rune = runes[j];
            let ci = if rune.darkens { BAND_DARK } else { BAND_LIGHT };
            let col = ctx.rgb(rune.ramp, ci, 0, 0);
            let op = BAND_OPACITY[j];
            let o = at(i % gw, i / gw);
            let mut px = ctx.resolve(cv.px[o]);
            if px[3] == 0 {
                px = [col[0], col[1], col[2], alpha(op)];
            } else {
                for ch in 0..3 {
                    px[ch] = ((px[ch] as u32 * (100 - op) + col[ch] as u32 * op) / 100) as u8;
                }
                px[3] = px[3].max(alpha(op));
            }
            cv.px[o] = Px::Rgba(px);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::pack;
    use crate::compose::{blend, gray};
    use crate::palette::tests::{rc, sample};
    use crate::request::{LayerRef, FLAG_CURSE, FLAG_GRAY};
    use crate::sprite::{bitmap, op, FORMAT_VERSION};
    use alloc::vec;

    // Sample palette: ramps gray(0) steel(1) leather(2) gold(3) dark(4); material 0 = steel/leather.

    // Item (mainhand) 32x32: P block 10..22 x 10..22, U strip 12..20 x 15, socket at (14, 17).
    fn item() -> Vec<u8> {
        let v = FORMAT_VERSION;
        let mut s = vec![v, 32, 32, 16, 0, 3];
        s.extend_from_slice(&[0x12, 10, 10, 12, 12, 0x16, 12, 15, 8, 1, 0xA0, 14, 17]);
        s
    }

    fn body() -> Vec<u8> {
        vec![FORMAT_VERSION, 32, 32, 11, 0, 1, 0x12, 10, 10, 4, 4]
    }

    /// The item of [`item`] drawn as one BITMAP (P auto, the U row auto) and the same socket.
    fn item_bitmap() -> Vec<u8> {
        let (p, u) = (bitmap::value(2, 0), bitmap::value(6, 0));
        let mut px = vec![p; 144];
        for x in 2..10 {
            px[5 * 12 + x] = u;
        }
        let mut s = vec![
            FORMAT_VERSION,
            32,
            32,
            16,
            0,
            2,
            op::BITMAP << 4,
            10,
            10,
            12,
            12,
        ];
        s.extend(bitmap::encode(&px));
        s.extend_from_slice(&[0xA0, 14, 17]);
        s
    }

    fn books() -> Vec<u8> {
        pack(
            1,
            &[(1, &item()[..]), (2, &body()[..]), (3, &item_bitmap()[..])],
        )
        .unwrap()
    }

    fn req(id: u16, element: u8, flags: u32) -> RenderRequest {
        let mut r = RenderRequest {
            palette_version: 1,
            flags,
            ..Default::default()
        };
        r.layers[16] = LayerRef {
            sprite_id: id,
            material_ramp: 0,
            accent_ramp: 3,
            element_ramp: element,
        };
        r
    }

    fn run(r: &RenderRequest, crafts: &[(usize, Craft)]) -> Result<Image> {
        let pal = sample();
        let p = Palette::parse(&pal).unwrap();
        let b = books();
        let books = [Book::parse(&b).unwrap()];
        compose_crafted(r, &p, &books, crafts)
    }

    fn px(img: &Image, x: usize, y: usize) -> [u8; 4] {
        let o = (y * img.width + x) * 4;
        img.rgba[o..o + 4].try_into().unwrap()
    }

    fn rgb(c: [u8; 3]) -> [u8; 4] {
        [c[0], c[1], c[2], 255]
    }

    #[test]
    fn no_craft_is_plain_compose() {
        let plain = run(&req(1, 0, 0), &[]).unwrap();
        let empty = run(&req(1, 0, 0), &[(16, Craft::default())]).unwrap();
        assert_eq!(plain, empty);
    }

    #[test]
    fn enchantment_cast() {
        let c = Craft {
            enchant_level: 2,
            ..Default::default()
        };
        // Interior P cell (15, 12): color 2 of steel blended with the gold ramp, k = 96.
        let img = run(&req(1, 3, 0), &[(16, c)]).unwrap();
        let (m, e) = (rc(1, 2), rc(3, 2));
        assert_eq!(px(&img, 15, 12), rgb(blend(m, e, 96)));
        // The colored outline next to a cast cell carries the cast too.
        assert_eq!(px(&img, 15, 9), rgb(blend(rc(1, 0), rc(3, 0), 96)));
        // Without a craft the element ramp does not cast.
        let plain = run(&req(1, 3, 0), &[]).unwrap();
        assert_eq!(px(&plain, 15, 12), rgb(m));
        // Level 10 is capped at 160.
        let c = Craft {
            enchant_level: 10,
            ..Default::default()
        };
        let img = run(&req(1, 3, 0), &[(16, c)]).unwrap();
        assert_eq!(px(&img, 15, 12), rgb(blend(m, e, 160)));
    }

    #[test]
    fn sockets_empty_and_filled() {
        let mut c = Craft {
            sockets_len: 1,
            ..Default::default()
        };
        let img = run(&req(1, 0, 0), &[(16, c)]).unwrap();
        assert_eq!(px(&img, 14, 17), rgb(rc(1, 1)), "recess color 1");
        assert_eq!(px(&img, 17, 20), rgb(rc(1, 1)));
        assert_eq!(px(&img, 13, 16), rgb(rc(1, 2)), "frame color 2");
        assert_eq!(px(&img, 18, 21), rgb(rc(1, 2)));
        c.sockets[0] = SocketFill::Ramp(3);
        let img = run(&req(1, 0, 0), &[(16, c)]).unwrap();
        assert_eq!(px(&img, 14, 17), rgb(rc(3, 4)), "highlight top left");
        assert_eq!(px(&img, 15, 17), rgb(rc(3, 3)));
        assert_eq!(px(&img, 15, 18), rgb(rc(3, 2)));
        assert_eq!(px(&img, 17, 20), rgb(rc(3, 1)));
        assert_eq!(px(&img, 13, 16), rgb(rc(1, 2)), "frame stays");
    }

    #[test]
    fn rune_zone_and_bands() {
        let c = Craft {
            seed: [7; 32],
            runes: [
                Some(Rune {
                    ramp: 3,
                    darkens: false,
                }),
                Some(Rune {
                    ramp: 4,
                    darkens: true,
                }),
                None,
            ],
            ..Default::default()
        };
        let img = run(&req(1, 0, 0), &[(16, c)]).unwrap();
        // Zone pixels are carved (steel color 1) or glowing (the rune ramp).
        // 8 zone pixels, 2 runes: x 12..16 is rune 1 (gold), x 16..20 rune 2 (dark).
        let zone: Vec<[u8; 4]> = (12..20).map(|x| px(&img, x, 15)).collect();
        for (n, p) in zone.iter().enumerate() {
            let ramp = if n < 4 { 3 } else { 4 };
            assert!(
                *p == rgb(rc(1, 1)) || (0..6).any(|s| *p == rgb(rc(ramp, s))),
                "zone pixel {n}: {p:?}"
            );
        }
        // Mask is 10..22. The outline at distance 1 (steel color 0) is blended 55% with gold
        // color 5.
        let o = rc(1, 0);
        let g = rc(3, 5);
        let want: [u8; 3] =
            core::array::from_fn(|i| ((o[i] as u32 * 45 + g[i] as u32 * 55) / 100) as u8);
        assert_eq!(px(&img, 9, 15), rgb(want));
        // Distance 2: transparent, rune 1 band at 55%.
        assert_eq!(px(&img, 8, 15), [g[0], g[1], g[2], 140]);
        // Distances 3-4: the darkness rune, color 0 of its ramp at 40%.
        let d = rc(4, 0);
        assert_eq!(px(&img, 7, 15), [d[0], d[1], d[2], 102]);
        assert_eq!(
            px(&img, 6, 6),
            [d[0], d[1], d[2], 102],
            "Chebyshev diagonal"
        );
        assert_eq!(px(&img, 5, 15), [0, 0, 0, 0], "distance 5: no third rune");
        // Deterministic, and the seed changes the pattern.
        assert_eq!(img, run(&req(1, 0, 0), &[(16, c)]).unwrap());
        let other = Craft { seed: [8; 32], ..c };
        let img2 = run(&req(1, 0, 0), &[(16, other)]).unwrap();
        let zone2: Vec<[u8; 4]> = (12..20).map(|x| px(&img2, x, 15)).collect();
        assert_ne!(zone, zone2);
    }

    #[test]
    fn gray_flag_grays_craft() {
        let c = Craft {
            sockets_len: 1,
            sockets: [
                SocketFill::Ramp(3),
                SocketFill::Empty,
                SocketFill::Empty,
                SocketFill::Empty,
            ],
            ..Default::default()
        };
        let img = run(&req(1, 0, FLAG_GRAY), &[(16, c)]).unwrap();
        assert_eq!(px(&img, 14, 17), rgb(gray(rc(3, 4))));
    }

    #[test]
    fn bitmap_item_crafts_like_geometry() {
        // The same item drawn as one BITMAP gets the same cast, runes, sockets and bands.
        let c = Craft {
            seed: [3; 32],
            enchant_level: 1,
            runes: [
                Some(Rune {
                    ramp: 3,
                    darkens: false,
                }),
                Some(Rune {
                    ramp: 4,
                    darkens: true,
                }),
                None,
            ],
            sockets_len: 1,
            sockets: [
                SocketFill::Ramp(2),
                SocketFill::Empty,
                SocketFill::Empty,
                SocketFill::Empty,
            ],
        };
        for flags in [0, FLAG_GRAY, FLAG_CURSE] {
            let geo = run(&req(1, 3, flags), &[(16, c)]).unwrap();
            let bmp = run(&req(3, 3, flags), &[(16, c)]).unwrap();
            assert_eq!(geo, bmp, "flags {flags}");
        }
        let plain = run(&req(1, 0, 0), &[]).unwrap();
        assert_ne!(plain, run(&req(3, 3, 0), &[(16, c)]).unwrap());
    }

    #[test]
    fn rejections() {
        let rune = Some(Rune {
            ramp: 3,
            darkens: false,
        });
        let r = req(1, 0, 0);
        let ok = Craft::default();
        assert_eq!(run(&r, &[(20, ok)]), Err(Error::Request), "index");
        assert_eq!(run(&r, &[(0, ok)]), Err(Error::Request), "empty layer");
        assert_eq!(run(&r, &[(16, ok), (16, ok)]), Err(Error::Request), "twice");
        assert_eq!(
            run(&req(2, 0, 0), &[(16, ok)]),
            Err(Error::Request),
            "body layer"
        );
        let gap = Craft {
            runes: [None, rune, None],
            ..ok
        };
        assert_eq!(run(&r, &[(16, gap)]), Err(Error::Request), "rune gap");
        let bad_rune = Craft {
            runes: [
                Some(Rune {
                    ramp: 5,
                    darkens: false,
                }),
                None,
                None,
            ],
            ..ok
        };
        assert_eq!(run(&r, &[(16, bad_rune)]), Err(Error::Palette), "rune ramp");
        let bad_gem = Craft {
            sockets_len: 1,
            sockets: [SocketFill::Ramp(5); SOCKETS_MAX],
            ..ok
        };
        assert_eq!(run(&r, &[(16, bad_gem)]), Err(Error::Palette), "gem ramp");
        let too_many = Craft {
            sockets_len: 2,
            ..ok
        };
        assert_eq!(
            run(&r, &[(16, too_many)]),
            Err(Error::Request),
            "more than the points"
        );
    }
}
