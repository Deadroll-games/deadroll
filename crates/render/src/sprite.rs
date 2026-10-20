//! Binary sprite decoder (`FORMAT.md` 3.1). Validates everything, never panics.

use alloc::vec::Vec;

use crate::error::{Error, Result};

pub const FORMAT_VERSION: u8 = 2;
pub const HEADER_LEN: usize = 6;
pub const FLAG_NO_OUTLINE: u8 = 1;
pub const POLY_MIN: u8 = 3;
pub const POLY_MAX: u8 = 8;

/// Slot values (`FORMAT.md` 1.3). Items and body share the numbers.
pub mod slot {
    pub const ERASE: u8 = 0;
    pub const OUTLINE: u8 = 1;
    /// `P` / `K`.
    pub const PRIMARY: u8 = 2;
    /// `S` / `H`.
    pub const SECONDARY: u8 = 3;
    /// `A` / `Y`.
    pub const ACCENT: u8 = 4;
    /// `E` / `D`.
    pub const ELEMENT: u8 = 5;
    /// `U` / `R`.
    pub const RUNE: u8 = 6;
    pub const MAX: u8 = 6;

    /// Number of shades of a slot (`FORMAT.md` 1.3). 0 for erase.
    pub const fn shades(slot: u8) -> u8 {
        match slot {
            OUTLINE => 1,
            PRIMARY | ELEMENT | RUNE => 4,
            SECONDARY | ACCENT => 3,
            _ => 0,
        }
    }
}

/// Layer types (`FORMAT.md` 1.4).
pub mod layer {
    pub const SHADOW: u8 = 0;
    pub const ARM_BACK: u8 = 2;
    pub const LEGS: u8 = 5;
    pub const TORSO: u8 = 8;
    pub const HEAD: u8 = 11;
    pub const HAIR: u8 = 12;
    pub const ARM_FRONT: u8 = 14;
    pub const TILE: u8 = 20;
    pub const MONSTER: u8 = 21;
    pub const ICON: u8 = 22;
    pub const PORTRAIT: u8 = 23;
    pub const MAX: u8 = PORTRAIT;

    /// Names in byte order, as written after `layer` in the DSL.
    pub const NAMES: [&str; 24] = [
        "shadow",
        "back_item",
        "arm_back",
        "glove_back",
        "offhand",
        "legs",
        "pants",
        "boots",
        "torso",
        "armor_body",
        "necklace",
        "head",
        "hair",
        "helmet",
        "arm_front",
        "glove_front",
        "mainhand",
        "overlay_wounds",
        "overlay_element",
        "fx",
        "tile",
        "monster",
        "icon",
        "portrait",
    ];

    /// Body layers use the body meaning of slots 2..6 (`FORMAT.md` 1.3).
    pub const fn is_body(layer_type: u8) -> bool {
        matches!(
            layer_type,
            ARM_BACK | LEGS | TORSO | HEAD | HAIR | ARM_FRONT | MONSTER
        )
    }
}

/// Opcodes (`FORMAT.md` 1.2).
pub mod op {
    pub const RECT: u8 = 0x1;
    pub const PIXEL: u8 = 0x2;
    pub const LINE: u8 = 0x3;
    pub const ELLIPSE: u8 = 0x4;
    pub const RING: u8 = 0x5;
    pub const POLY: u8 = 0x6;
    pub const MIRROR_X: u8 = 0x7;
    pub const MIRROR_Y: u8 = 0x8;
    pub const SHADE: u8 = 0x9;
    pub const SOCKET: u8 = 0xA;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub width: u8,
    pub height: u8,
    pub layer_type: u8,
    pub flags: u8,
}

impl Header {
    pub const fn no_outline(&self) -> bool {
        self.flags & FLAG_NO_OUTLINE != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    Rect {
        x: u8,
        y: u8,
        w: u8,
        h: u8,
        slot: u8,
    },
    Pixel {
        x: u8,
        y: u8,
        slot: u8,
    },
    Line {
        x0: u8,
        y0: u8,
        x1: u8,
        y1: u8,
        slot: u8,
    },
    Ellipse {
        x: u8,
        y: u8,
        w: u8,
        h: u8,
        slot: u8,
    },
    Ring {
        x: u8,
        y: u8,
        w: u8,
        h: u8,
        slot: u8,
    },
    Poly {
        slot: u8,
        n: u8,
        pts: [(u8, u8); POLY_MAX as usize],
    },
    MirrorX {
        axis: u8,
    },
    MirrorY {
        axis: u8,
    },
    Shade {
        x: u8,
        y: u8,
        slot: u8,
        shade: u8,
    },
    Socket {
        x: u8,
        y: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sprite {
    pub header: Header,
    pub cmds: Vec<Cmd>,
}

/// Allowed canvas sides (`FORMAT.md` 1.1). Format 2 doubled the format 1 sizes.
pub const fn valid_size(v: u8) -> bool {
    matches!(v, 32 | 64 | 96 | 128)
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> Result<u8> {
        let b = *self.bytes.get(self.pos).ok_or(Error::Truncated)?;
        self.pos += 1;
        Ok(b)
    }
}

/// Decodes and validates one sprite. The input must end exactly after the last command.
pub fn decode(bytes: &[u8]) -> Result<Sprite> {
    let mut r = Reader { bytes, pos: 0 };
    if r.u8()? != FORMAT_VERSION {
        return Err(Error::Version);
    }
    let header = Header {
        width: r.u8()?,
        height: r.u8()?,
        layer_type: r.u8()?,
        flags: r.u8()?,
    };
    if !valid_size(header.width)
        || !valid_size(header.height)
        || header.layer_type > layer::MAX
        || header.flags & !FLAG_NO_OUTLINE != 0
    {
        return Err(Error::Header);
    }
    let count = r.u8()?;
    let (w, h) = (header.width, header.height);
    let mut cmds = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let b = r.u8()?;
        let (opcode, s) = (b >> 4, b & 0x0F);
        let cmd = match opcode {
            op::RECT | op::ELLIPSE | op::RING => {
                let (x, y, cw, ch) = (r.u8()?, r.u8()?, r.u8()?, r.u8()?);
                check_slot(s)?;
                check_rect(x, y, cw, ch, w, h)?;
                match opcode {
                    op::RECT => Cmd::Rect {
                        x,
                        y,
                        w: cw,
                        h: ch,
                        slot: s,
                    },
                    op::ELLIPSE => Cmd::Ellipse {
                        x,
                        y,
                        w: cw,
                        h: ch,
                        slot: s,
                    },
                    _ => Cmd::Ring {
                        x,
                        y,
                        w: cw,
                        h: ch,
                        slot: s,
                    },
                }
            }
            op::PIXEL => {
                let (x, y) = (r.u8()?, r.u8()?);
                check_slot(s)?;
                check_point(x, y, w, h)?;
                Cmd::Pixel { x, y, slot: s }
            }
            op::LINE => {
                let (x0, y0, x1, y1) = (r.u8()?, r.u8()?, r.u8()?, r.u8()?);
                check_slot(s)?;
                check_point(x0, y0, w, h)?;
                check_point(x1, y1, w, h)?;
                Cmd::Line {
                    x0,
                    y0,
                    x1,
                    y1,
                    slot: s,
                }
            }
            op::POLY => {
                let n = r.u8()?;
                check_slot(s)?;
                if !(POLY_MIN..=POLY_MAX).contains(&n) {
                    return Err(Error::Arg);
                }
                let mut pts = [(0u8, 0u8); POLY_MAX as usize];
                for p in pts.iter_mut().take(n as usize) {
                    let (x, y) = (r.u8()?, r.u8()?);
                    if x > w || y > h {
                        return Err(Error::Arg);
                    }
                    *p = (x, y);
                }
                Cmd::Poly { slot: s, n, pts }
            }
            op::MIRROR_X | op::MIRROR_Y => {
                let axis = r.u8()?;
                if s != 0 {
                    return Err(Error::Slot);
                }
                let size = if opcode == op::MIRROR_X { w } else { h };
                if axis as u16 > 2 * size as u16 {
                    return Err(Error::Arg);
                }
                if opcode == op::MIRROR_X {
                    Cmd::MirrorX { axis }
                } else {
                    Cmd::MirrorY { axis }
                }
            }
            op::SHADE => {
                let (x, y, shade) = (r.u8()?, r.u8()?, r.u8()?);
                if s == slot::ERASE || s > slot::MAX {
                    return Err(Error::Slot);
                }
                check_point(x, y, w, h)?;
                if shade >= slot::shades(s) {
                    return Err(Error::Arg);
                }
                Cmd::Shade {
                    x,
                    y,
                    slot: s,
                    shade,
                }
            }
            op::SOCKET => {
                let (x, y) = (r.u8()?, r.u8()?);
                if s != 0 {
                    return Err(Error::Slot);
                }
                if x as u16 + 2 > w as u16 || y as u16 + 2 > h as u16 {
                    return Err(Error::Arg);
                }
                Cmd::Socket { x, y }
            }
            _ => return Err(Error::Opcode),
        };
        cmds.push(cmd);
    }
    if r.pos != bytes.len() {
        return Err(Error::Truncated);
    }
    Ok(Sprite { header, cmds })
}

fn check_slot(s: u8) -> Result<()> {
    if s > slot::MAX {
        Err(Error::Slot)
    } else {
        Ok(())
    }
}

fn check_point(x: u8, y: u8, w: u8, h: u8) -> Result<()> {
    if x < w && y < h {
        Ok(())
    } else {
        Err(Error::Arg)
    }
}

fn check_rect(x: u8, y: u8, cw: u8, ch: u8, w: u8, h: u8) -> Result<()> {
    if cw == 0 || ch == 0 || x as u16 + cw as u16 > w as u16 || y as u16 + ch as u16 > h as u16 {
        Err(Error::Arg)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn sprite(cmds: &[u8], count: u8) -> Vec<u8> {
        let mut v = vec![FORMAT_VERSION, 32, 32, 16, 0, count];
        v.extend_from_slice(cmds);
        v
    }

    #[test]
    fn decodes_every_command() {
        let bytes = sprite(
            &[
                0x12, 1, 2, 3, 4, // RECT P
                0x23, 5, 6, // PIXEL S
                0x34, 0, 0, 31, 31, // LINE A
                0x45, 1, 1, 30, 30, // ELLIPSE E
                0x56, 2, 2, 4, 4, // RING U
                0x60, 3, 0, 0, 32, 0, 0, 32, // POLY erase
                0x70, 32, // MIRROR_X
                0x80, 64, // MIRROR_Y
                0x91, 4, 4, 0, // SHADE O
                0xA0, 30, 30, // SOCKET
            ],
            10,
        );
        let s = decode(&bytes).unwrap();
        assert_eq!(s.cmds.len(), 10);
        assert_eq!(
            s.cmds[0],
            Cmd::Rect {
                x: 1,
                y: 2,
                w: 3,
                h: 4,
                slot: 2
            }
        );
        assert_eq!(s.cmds[9], Cmd::Socket { x: 30, y: 30 });
    }

    #[test]
    fn rejections() {
        let ok = sprite(&[0x12, 0, 0, 1, 1], 1);
        assert!(decode(&ok).is_ok());
        // truncated header, truncated command, trailing byte
        assert_eq!(decode(&[FORMAT_VERSION, 32, 32]), Err(Error::Truncated));
        assert_eq!(decode(&ok[..ok.len() - 1]), Err(Error::Truncated));
        let mut trailing = ok.clone();
        trailing.push(0);
        assert_eq!(decode(&trailing), Err(Error::Truncated));
        // version: format 1 sprites belong to rasterizer version 1
        for version in [1, 3] {
            let mut v = ok.clone();
            v[0] = version;
            assert_eq!(decode(&v), Err(Error::Version), "version {version}");
        }
        // size (format 1 sizes 16 and 48 too), layer type, flags
        for (i, b) in [
            (1, 33u8),
            (1, 16),
            (2, 48),
            (1, 129),
            (2, 0),
            (3, 24),
            (4, 2),
        ] {
            let mut v = ok.clone();
            v[i] = b;
            assert_eq!(decode(&v), Err(Error::Header), "byte {i}");
        }
        // opcodes 0 and 0xB..=0xF
        for o in [0x0u8, 0xB, 0xF] {
            assert_eq!(
                decode(&sprite(&[o << 4, 0, 0, 1, 1], 1)),
                Err(Error::Opcode)
            );
        }
        // slots
        assert_eq!(decode(&sprite(&[0x17, 0, 0, 1, 1], 1)), Err(Error::Slot));
        assert_eq!(decode(&sprite(&[0x71, 32], 1)), Err(Error::Slot));
        assert_eq!(decode(&sprite(&[0xA1, 0, 0], 1)), Err(Error::Slot));
        assert_eq!(decode(&sprite(&[0x90, 0, 0, 0], 1)), Err(Error::Slot));
        // args
        for cmd in [
            &[0x12, 0, 0, 0, 1][..],       // zero width
            &[0x12, 30, 0, 3, 1],          // past right edge
            &[0x42, 0, 31, 1, 2],          // ellipse past bottom
            &[0x22, 32, 0],                // pixel outside
            &[0x32, 0, 0, 0, 32],          // line end outside
            &[0x62, 2, 0, 0, 1, 1],        // poly n = 2
            &[0x62, 3, 0, 0, 33, 0, 0, 1], // poly vertex past w
            &[0x70, 65],                   // mirror axis
            &[0x93, 0, 0, 3],              // shade >= n (S has 3)
            &[0xA0, 31, 0],                // socket square outside
        ] {
            assert_eq!(decode(&sprite(cmd, 1)), Err(Error::Arg), "{cmd:?}");
        }
    }

    #[test]
    fn largest_canvas() {
        // 128x128: coordinates up to 127, a rect to the edge, the widest mirror axis.
        let mut v = vec![FORMAT_VERSION, 128, 128, 21, 0, 4];
        v.extend_from_slice(&[
            0x12, 0, 0, 128, 128, 0x22, 127, 127, 0x70, 255, 0xA0, 126, 126,
        ]);
        assert_eq!(decode(&v).unwrap().cmds.len(), 4);
        v.extend_from_slice(&[0x12, 1, 0, 128, 1]);
        v[5] = 5;
        assert_eq!(decode(&v), Err(Error::Arg), "past the right edge");
    }

    #[test]
    fn never_panics_on_any_short_input() {
        // Every prefix of a valid sprite and some garbage: errors only.
        let full = sprite(&[0x62, 3, 0, 0, 4, 0, 0, 4, 0x93, 1, 1, 2], 2);
        for n in 0..full.len() {
            assert!(decode(&full[..n]).is_err());
        }
        for b in 0..=255u8 {
            let _ = decode(&sprite(&[b, b, b, b, b, b, b, b], 1));
        }
    }
}
