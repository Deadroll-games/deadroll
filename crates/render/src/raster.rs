//! Geometry commands -> slot grid -> auto shading -> outline mask (`FORMAT.md` section 2).

use alloc::vec;
use alloc::vec::Vec;

use crate::sprite::{bitmap, layer, slot, Cmd, Sprite, POLY_MAX};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    /// 0 = empty.
    pub slot: u8,
    pub shade: u8,
    /// Set by `SHADE`: auto shading leaves the cell alone.
    pub explicit: bool,
}

/// The layer grid of one sprite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); width * height],
        }
    }

    pub fn get(&self, x: i32, y: i32) -> Option<Cell> {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return None;
        }
        Some(self.cells[y as usize * self.width + x as usize])
    }

    fn set(&mut self, x: i32, y: i32, cell: Cell) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let w = self.width;
        self.cells[y as usize * w + x as usize] = cell;
    }

    fn paint(&mut self, x: i32, y: i32, s: u8) {
        self.set(
            x,
            y,
            Cell {
                slot: s,
                shade: 0,
                explicit: false,
            },
        );
    }

    fn filled(&self, x: i32, y: i32) -> bool {
        self.get(x, y).is_some_and(|c| c.slot != slot::ERASE)
    }
}

/// Runs the commands, then auto shading. The result is the shaded layer grid.
pub fn rasterize(sprite: &Sprite) -> Grid {
    let h = &sprite.header;
    let mut g = Grid::new(h.width as usize, h.height as usize);
    for cmd in &sprite.cmds {
        draw(&mut g, cmd, &sprite.bitmaps);
    }
    shade(&mut g, layer::is_body(h.layer_type));
    g
}

fn draw(g: &mut Grid, cmd: &Cmd, bitmaps: &[Vec<u8>]) {
    match *cmd {
        Cmd::Rect { x, y, w, h, slot } => {
            for yy in y as i32..y as i32 + h as i32 {
                for xx in x as i32..x as i32 + w as i32 {
                    g.paint(xx, yy, slot);
                }
            }
        }
        Cmd::Pixel { x, y, slot } => g.paint(x as i32, y as i32, slot),
        Cmd::Line {
            x0,
            y0,
            x1,
            y1,
            slot,
        } => {
            for (x, y) in line(x0 as i32, y0 as i32, x1 as i32, y1 as i32) {
                g.paint(x, y, slot);
            }
        }
        Cmd::Ellipse { x, y, w, h, slot } => {
            for yy in y as i32..y as i32 + h as i32 {
                for xx in x as i32..x as i32 + w as i32 {
                    if in_ellipse(xx, yy, x, y, w, h) {
                        g.paint(xx, yy, slot);
                    }
                }
            }
        }
        Cmd::Ring { x, y, w, h, slot } => {
            for yy in y as i32..y as i32 + h as i32 {
                for xx in x as i32..x as i32 + w as i32 {
                    let inside = |px, py| in_ellipse(px, py, x, y, w, h);
                    if inside(xx, yy)
                        && (!inside(xx - 1, yy)
                            || !inside(xx + 1, yy)
                            || !inside(xx, yy - 1)
                            || !inside(xx, yy + 1))
                    {
                        g.paint(xx, yy, slot);
                    }
                }
            }
        }
        Cmd::Poly { slot, n, pts } => {
            let (w, h) = (g.width as i32, g.height as i32);
            for yy in 0..h {
                for xx in 0..w {
                    if in_poly(xx, yy, &pts, n) {
                        g.paint(xx, yy, slot);
                    }
                }
            }
        }
        Cmd::MirrorX { axis } => mirror(g, axis as i32, true),
        Cmd::MirrorY { axis } => mirror(g, axis as i32, false),
        Cmd::Shade { x, y, slot, shade } => {
            g.set(
                x as i32,
                y as i32,
                Cell {
                    slot,
                    shade,
                    explicit: true,
                },
            );
        }
        // TODO(post-demo): sockets are drawn after shading (ART_DESCRIPTION 9.4).
        Cmd::Socket { .. } => {}
        // FORMAT.md 2.8: skip pixels draw nothing, code 0 is auto shaded, code c + 1 is color c.
        Cmd::Bitmap { x, y, w, index, .. } => {
            let px = bitmaps.get(index as usize).map_or(&[][..], Vec::as_slice);
            let w = w as usize;
            for (i, &v) in px.iter().enumerate() {
                let (s, code) = (bitmap::slot(v), bitmap::code(v));
                if s == bitmap::SKIP {
                    continue;
                }
                g.set(
                    x as i32 + (i % w) as i32,
                    y as i32 + (i / w) as i32,
                    Cell {
                        slot: s,
                        shade: code.saturating_sub(1),
                        explicit: code != 0,
                    },
                );
            }
        }
    }
}

/// Bresenham + pixel-perfect cleanup, exactly `FORMAT.md` 2.2.
pub fn line(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let (mut x, mut y) = (x0, y0);
    let mut p = Vec::new();
    loop {
        p.push((x, y));
        if (x, y) == (x1, y1) {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    let n = p.len();
    let mut out = Vec::with_capacity(n);
    out.push(p[0]);
    for i in 1..n.saturating_sub(1) {
        let (a, b, c) = (out[out.len() - 1], p[i], p[i + 1]);
        let a_c_diagonal = (a.0 - c.0).abs() == 1 && (a.1 - c.1).abs() == 1;
        let b_touches_a = a.0 == b.0 || a.1 == b.1;
        let b_touches_c = c.0 == b.0 || c.1 == b.1;
        if !(a_c_diagonal && b_touches_a && b_touches_c) {
            out.push(b);
        }
    }
    if n > 1 {
        out.push(p[n - 1]);
    }
    out
}

/// `FORMAT.md` 2.3, in doubled coordinates.
fn in_ellipse(px: i32, py: i32, x: u8, y: u8, w: u8, h: u8) -> bool {
    let (w, h) = (w as i64, h as i64);
    let cx = 2 * x as i64 + w;
    let cy = 2 * y as i64 + h;
    let dx = 2 * px as i64 + 1 - cx;
    let dy = 2 * py as i64 + 1 - cy;
    dx * dx * h * h + dy * dy * w * w <= w * w * h * h
}

/// `FORMAT.md` 2.5: even-odd, ray to the right, left edges inclusive.
fn in_poly(px: i32, py: i32, pts: &[(u8, u8); POLY_MAX as usize], n: u8) -> bool {
    let (px, py) = (2 * px as i64 + 1, 2 * py as i64 + 1);
    let n = n as usize;
    let mut inside = false;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let (ax, ay) = (2 * a.0 as i64, 2 * a.1 as i64);
        let (bx, by) = (2 * b.0 as i64, 2 * b.1 as i64);
        if (ay > py) != (by > py) {
            let num = (ax - px) * (by - ay) + (py - ay) * (bx - ax);
            if num * (by - ay).signum() > 0 {
                inside = !inside;
            }
        }
    }
    inside
}

/// `FORMAT.md` 2.6.
fn mirror(g: &mut Grid, axis: i32, horizontal: bool) {
    let snapshot = g.cells.clone();
    let w = g.width;
    for (i, cell) in snapshot.iter().enumerate() {
        if cell.slot == slot::ERASE {
            continue;
        }
        let (x, y) = ((i % w) as i32, (i / w) as i32);
        if horizontal {
            g.set(axis - 1 - x, y, *cell);
        } else {
            g.set(x, axis - 1 - y, *cell);
        }
    }
}

/// Slot used for the "own neighbor" test: on item layers `U` counts as `P`.
fn own_class(s: u8, body: bool) -> u8 {
    if !body && s == slot::RUNE {
        slot::PRIMARY
    } else {
        s
    }
}

/// `FORMAT.md` 2.8: a level per cell, stored as its ramp color (`slot::auto_color`).
fn shade(g: &mut Grid, body: bool) {
    let src = g.clone();
    for y in 0..g.height as i32 {
        for x in 0..g.width as i32 {
            let Some(c) = src.get(x, y) else { continue };
            if c.slot == slot::ERASE || c.explicit {
                continue;
            }
            let n = slot::levels(c.slot);
            if n <= 2 {
                g.set(x, y, Cell { shade: 0, ..c });
                continue;
            }
            let class = own_class(c.slot, body);
            let not_own = |dx: i32, dy: i32| {
                src.get(x + dx, y + dy)
                    .is_none_or(|o| own_class(o.slot, body) != class || o.slot == slot::ERASE)
            };
            let (l, t, r, b) = (not_own(-1, 0), not_own(0, -1), not_own(1, 0), not_own(0, 1));
            let s = if (l || t) && !(r || b) {
                if l && t {
                    n - 1
                } else {
                    2.min(n - 1)
                }
            } else if (r || b) && !(l || t) {
                0
            } else {
                1
            };
            g.set(
                x,
                y,
                Cell {
                    shade: slot::auto_color(c.slot, s),
                    ..c
                },
            );
        }
    }
}

/// `FORMAT.md` 2.9: empty cells with a filled 4-neighbor.
pub fn outline_mask(g: &Grid) -> Vec<bool> {
    let mut mask = vec![false; g.cells.len()];
    for y in 0..g.height as i32 {
        for x in 0..g.width as i32 {
            if g.filled(x, y) {
                continue;
            }
            if g.filled(x - 1, y) || g.filled(x + 1, y) || g.filled(x, y - 1) || g.filled(x, y + 1)
            {
                mask[y as usize * g.width + x as usize] = true;
            }
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sprite::Header;
    use alloc::string::String;

    fn grid(cmds: &[Cmd], w: u8, h: u8) -> Grid {
        let header = Header {
            width: w,
            height: h,
            layer_type: 16,
            flags: 0,
        };
        let mut g = Grid::new(w as usize, h as usize);
        for c in cmds {
            draw(&mut g, c, &[]);
        }
        let _ = header;
        g
    }

    /// `#` for filled, `.` for empty, rows joined by `|`.
    fn ascii(g: &Grid, w: usize, h: usize) -> String {
        let mut s = String::new();
        for y in 0..h {
            if y > 0 {
                s.push('|');
            }
            for x in 0..w {
                s.push(if g.cells[y * g.width + x].slot != 0 {
                    '#'
                } else {
                    '.'
                });
            }
        }
        s
    }

    #[test]
    fn line_cleanup_removes_l_corners() {
        // A 2:1 slope keeps one pixel per column.
        assert_eq!(
            line(0, 0, 4, 2),
            vec![(0, 0), (1, 1), (2, 1), (3, 2), (4, 2)]
        );
        // 45 degrees: pure diagonal.
        assert_eq!(line(0, 0, 3, 3), vec![(0, 0), (1, 1), (2, 2), (3, 3)]);
        // Single point and reversed direction.
        assert_eq!(line(5, 5, 5, 5), vec![(5, 5)]);
        assert_eq!(line(3, 0, 0, 0), vec![(3, 0), (2, 0), (1, 0), (0, 0)]);
        // A shallow slope where Bresenham makes an L step: the corner pixel is removed.
        let l = line(0, 0, 5, 1);
        for w in l.windows(3) {
            let (a, b, c) = (w[0], w[1], w[2]);
            let corner = (a.0 - c.0).abs() == 1
                && (a.1 - c.1).abs() == 1
                && (a.0 == b.0 || a.1 == b.1)
                && (c.0 == b.0 || c.1 == b.1);
            assert!(!corner, "{l:?}");
        }
    }

    #[test]
    fn ellipse_even_and_odd() {
        let g = grid(
            &[Cmd::Ellipse {
                x: 0,
                y: 0,
                w: 4,
                h: 4,
                slot: 2,
            }],
            16,
            16,
        );
        assert_eq!(ascii(&g, 4, 4), ".##.|####|####|.##.");
        let g = grid(
            &[Cmd::Ellipse {
                x: 0,
                y: 0,
                w: 3,
                h: 3,
                slot: 2,
            }],
            16,
            16,
        );
        assert_eq!(ascii(&g, 3, 3), "###|###|###");
        let g = grid(
            &[Cmd::Ellipse {
                x: 0,
                y: 0,
                w: 5,
                h: 5,
                slot: 2,
            }],
            16,
            16,
        );
        assert_eq!(ascii(&g, 5, 5), ".###.|#####|#####|#####|.###.");
        let g = grid(
            &[Cmd::Ellipse {
                x: 0,
                y: 0,
                w: 6,
                h: 2,
                slot: 2,
            }],
            16,
            16,
        );
        assert_eq!(ascii(&g, 6, 2), "######|######");
    }

    #[test]
    fn ring_is_the_border_of_the_ellipse() {
        let g = grid(
            &[Cmd::Ring {
                x: 0,
                y: 0,
                w: 5,
                h: 5,
                slot: 2,
            }],
            16,
            16,
        );
        assert_eq!(ascii(&g, 5, 5), ".###.|#...#|#...#|#...#|.###.");
    }

    #[test]
    fn poly_top_left_rule() {
        // Axis-aligned square 0..4: exactly the 16 pixels, like RECT.
        let mut pts = [(0, 0); 8];
        pts[..4].copy_from_slice(&[(0, 0), (4, 0), (4, 4), (0, 4)]);
        let g = grid(&[Cmd::Poly { slot: 2, n: 4, pts }], 16, 16);
        assert_eq!(ascii(&g, 5, 5), "####.|####.|####.|####.|.....");
        // Right triangle with a 45 degree hypotenuse through pixel centers.
        let mut pts = [(0, 0); 8];
        pts[..3].copy_from_slice(&[(0, 0), (4, 4), (0, 4)]);
        let g = grid(&[Cmd::Poly { slot: 2, n: 3, pts }], 16, 16);
        // Centers on the hypotenuse lie on a right edge: not filled.
        assert_eq!(ascii(&g, 4, 4), "....|#...|##..|###.");
        // The mirrored triangle: hypotenuse is now a left edge, so those centers are filled.
        let mut pts = [(0, 0); 8];
        pts[..3].copy_from_slice(&[(0, 0), (4, 0), (4, 4)]);
        let g = grid(&[Cmd::Poly { slot: 2, n: 3, pts }], 16, 16);
        assert_eq!(ascii(&g, 4, 4), "####|.###|..##|...#");
    }

    #[test]
    fn mirror_axis_between_and_through_pixels() {
        let g = grid(
            &[
                Cmd::Pixel {
                    x: 1,
                    y: 0,
                    slot: 2,
                },
                Cmd::MirrorX { axis: 8 },
            ],
            16,
            16,
        );
        assert_eq!(ascii(&g, 8, 1), ".#....#.");
        let g = grid(
            &[
                Cmd::Rect {
                    x: 0,
                    y: 0,
                    w: 2,
                    h: 1,
                    slot: 2,
                },
                Cmd::MirrorX { axis: 3 },
            ],
            16,
            16,
        );
        assert_eq!(ascii(&g, 4, 1), "###.");
        let g = grid(
            &[
                Cmd::Pixel {
                    x: 0,
                    y: 1,
                    slot: 2,
                },
                Cmd::MirrorY { axis: 8 },
            ],
            16,
            16,
        );
        assert_eq!(ascii(&g, 1, 8), ".|#|.|.|.|.|#|.");
        // Targets outside the canvas are skipped, no panic.
        let g = grid(
            &[
                Cmd::Pixel {
                    x: 0,
                    y: 0,
                    slot: 2,
                },
                Cmd::MirrorX { axis: 64 },
            ],
            16,
            16,
        );
        assert_eq!(g.cells.iter().filter(|c| c.slot != 0).count(), 1);
    }

    #[test]
    fn erase_and_painter_order() {
        let g = grid(
            &[
                Cmd::Rect {
                    x: 0,
                    y: 0,
                    w: 3,
                    h: 1,
                    slot: 2,
                },
                Cmd::Pixel {
                    x: 1,
                    y: 0,
                    slot: 0,
                },
            ],
            16,
            16,
        );
        assert_eq!(ascii(&g, 3, 1), "#.#");
    }

    #[test]
    fn bitmap_mixes_with_geometry() {
        // A 3x3 P rect, then a 3x1 bitmap over its middle row: skip, erase, explicit S color 5.
        let sprite = Sprite {
            header: Header {
                width: 32,
                height: 32,
                layer_type: 16,
                flags: 0,
            },
            cmds: vec![
                Cmd::Rect {
                    x: 1,
                    y: 1,
                    w: 3,
                    h: 3,
                    slot: 2,
                },
                Cmd::Bitmap {
                    x: 1,
                    y: 2,
                    w: 3,
                    h: 1,
                    index: 0,
                },
                Cmd::Bitmap {
                    x: 6,
                    y: 6,
                    w: 2,
                    h: 2,
                    index: 1,
                },
            ],
            bitmaps: vec![
                vec![
                    bitmap::value(bitmap::SKIP, 0),
                    bitmap::value(0, 0),
                    bitmap::value(3, 6),
                ],
                vec![bitmap::value(2, 0); 4],
            ],
        };
        let g = rasterize(&sprite);
        let c = |x: usize, y: usize| g.cells[y * 32 + x];
        assert_eq!(c(1, 2).slot, 2, "skip keeps the rect");
        assert_eq!(c(2, 2).slot, 0, "erase");
        assert_eq!(
            (c(3, 2).slot, c(3, 2).shade, c(3, 2).explicit),
            (3, 5, true)
        );
        // Auto pixels are shaded like geometry: a lone 2x2 square.
        assert_eq!(
            [c(6, 6).shade, c(7, 6).shade, c(6, 7).shade, c(7, 7).shade],
            [4, 2, 2, 1]
        );
        assert!(
            outline_mask(&g)[2 * 32 + 2],
            "the erased hole gets an outline"
        );
    }

    fn shaded(cmds: &[Cmd], body: bool) -> Grid {
        let mut g = grid(cmds, 16, 16);
        shade(&mut g, body);
        g
    }

    #[test]
    fn shading_light_from_top_left() {
        let g = shaded(
            &[Cmd::Rect {
                x: 1,
                y: 1,
                w: 3,
                h: 3,
                slot: 2,
            }],
            false,
        );
        let s = |x: usize, y: usize| g.cells[y * 16 + x].shade;
        assert_eq!(s(1, 1), 4); // corner highlight, L and T: level 3 -> color 4
        assert_eq!(s(2, 1), 3); // top edge
        assert_eq!(s(1, 2), 3); // left edge
        assert_eq!(s(2, 2), 2); // middle
        assert_eq!(s(3, 3), 1); // shadow corner (color 0 is the outline)
        assert_eq!(s(3, 1), 2); // top-right: T and R
                                // Secondary slot has 3 levels: the corner highlight is color 4.
        let g = shaded(
            &[Cmd::Rect {
                x: 1,
                y: 1,
                w: 3,
                h: 3,
                slot: 3,
            }],
            false,
        );
        assert_eq!(g.cells[16 + 1].shade, 4);
        // Explicit shade survives.
        let g = shaded(
            &[
                Cmd::Rect {
                    x: 1,
                    y: 1,
                    w: 3,
                    h: 3,
                    slot: 2,
                },
                Cmd::Shade {
                    x: 2,
                    y: 2,
                    slot: 2,
                    shade: 3,
                },
            ],
            false,
        );
        assert_eq!(g.cells[2 * 16 + 2].shade, 3);
    }

    #[test]
    fn rune_counts_as_primary_only_on_item_layers() {
        let cmds = [
            Cmd::Rect {
                x: 1,
                y: 1,
                w: 3,
                h: 3,
                slot: 2,
            },
            Cmd::Pixel {
                x: 2,
                y: 2,
                slot: 6,
            },
        ];
        assert_eq!(shaded(&cmds, false).cells[2 * 16 + 2].shade, 2); // surrounded by own
        assert_eq!(shaded(&cmds, true).cells[2 * 16 + 2].shade, 2); // all sides foreign -> middle
        let cmds = [
            Cmd::Rect {
                x: 1,
                y: 1,
                w: 3,
                h: 3,
                slot: 2,
            },
            Cmd::Pixel {
                x: 1,
                y: 1,
                slot: 6,
            },
        ];
        assert_eq!(shaded(&cmds, false).cells[16 + 1].shade, 4);
        // On a body layer R at the corner sees P on R and B: not own -> middle shade.
        assert_eq!(shaded(&cmds, true).cells[16 + 1].shade, 2);
    }

    #[test]
    fn outline_ring_around_shape() {
        let g = grid(
            &[Cmd::Pixel {
                x: 2,
                y: 2,
                slot: 2,
            }],
            16,
            16,
        );
        let m = outline_mask(&g);
        let on: Vec<usize> = (0..m.len()).filter(|&i| m[i]).collect();
        assert_eq!(on, vec![16 + 2, 2 * 16 + 1, 2 * 16 + 3, 3 * 16 + 2]);
    }
}
