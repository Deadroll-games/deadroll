//! Palette body (`FORMAT.md` 3.2), format 2: ramps of 6 colors, material flags.

use crate::error::{Error, Result};
use crate::sprite::RAMP_COLORS;

/// Palette format 2 (format 1, 4-color ramps, is frozen with palette 1 for rasterizer 1).
pub const FORMAT_VERSION: u8 = 2;
pub const RAMP_LEN: usize = RAMP_COLORS as usize * 3;
pub const MATERIAL_LEN: usize = 3;
/// Header: format, version (2), outline (3), dark, curse, cold ramps, ramp_count.
pub const HEADER_LEN: usize = 10;
/// Material flag bit 0: ordered dithering on shade borders (`FORMAT.md` 2.10).
pub const MATERIAL_DITHER: u8 = 1;
/// Largest palette body: 255 ramps and 255 materials (the `Palette` account holds 8 KiB).
pub const MAX_LEN: usize = HEADER_LEN + 255 * RAMP_LEN + 1 + 255 * MATERIAL_LEN;
const _: () = assert!(
    MAX_LEN <= 8 * 1024,
    "a palette must fit the 8 KiB Palette account"
);

pub type Rgb = [u8; 3];

#[derive(Clone, Copy, Debug)]
pub struct Palette<'a> {
    pub palette_version: u16,
    pub outline: Rgb,
    pub dark_ramp: u8,
    /// Ramp of the `CURSE` render flag: outline and cast on every ramp.
    pub curse_ramp: u8,
    /// Ramp of the `COLD` render flag: outline.
    pub cold_ramp: u8,
    ramps: &'a [u8],
    materials: &'a [u8],
}

impl<'a> Palette<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < HEADER_LEN || bytes[0] != FORMAT_VERSION {
            return Err(Error::Palette);
        }
        let palette_version = u16::from_le_bytes([bytes[1], bytes[2]]);
        let outline = [bytes[3], bytes[4], bytes[5]];
        let (dark_ramp, curse_ramp, cold_ramp) = (bytes[6], bytes[7], bytes[8]);
        let ramp_count = bytes[9] as usize;
        let ramps_end = HEADER_LEN + ramp_count * RAMP_LEN;
        if ramp_count == 0 || bytes.len() < ramps_end + 1 {
            return Err(Error::Palette);
        }
        let ramps = &bytes[HEADER_LEN..ramps_end];
        let material_count = bytes[ramps_end] as usize;
        let materials = &bytes[ramps_end + 1..];
        if material_count == 0 || materials.len() != material_count * MATERIAL_LEN {
            return Err(Error::Palette);
        }
        let in_range = |r: u8| (r as usize) < ramp_count;
        if !in_range(dark_ramp) || !in_range(curse_ramp) || !in_range(cold_ramp) {
            return Err(Error::Palette);
        }
        for m in materials.chunks_exact(MATERIAL_LEN) {
            if !in_range(m[0]) || !in_range(m[1]) || m[2] & !MATERIAL_DITHER != 0 {
                return Err(Error::Palette);
            }
        }
        Ok(Self {
            palette_version,
            outline,
            dark_ramp,
            curse_ramp,
            cold_ramp,
            ramps,
            materials,
        })
    }

    pub fn ramp_count(&self) -> usize {
        self.ramps.len() / RAMP_LEN
    }

    pub fn material_count(&self) -> usize {
        self.materials.len() / MATERIAL_LEN
    }

    /// Color `i` (0..6) of a ramp. Indices are validated by the caller.
    pub fn color(&self, ramp: u8, i: u8) -> Rgb {
        let o = ramp as usize * RAMP_LEN + i as usize * 3;
        [self.ramps[o], self.ramps[o + 1], self.ramps[o + 2]]
    }

    /// `(primary_ramp, secondary_ramp)` of a material. Index validated by the caller.
    pub fn material(&self, m: u8) -> (u8, u8) {
        let o = m as usize * MATERIAL_LEN;
        (self.materials[o], self.materials[o + 1])
    }

    /// Whether a material dithers its shade borders.
    pub fn dithers(&self, m: u8) -> bool {
        self.materials[m as usize * MATERIAL_LEN + 2] & MATERIAL_DITHER != 0
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// Ramp `r` color `i` of [`sample`]: `[b + i, b + 10 + i, b + 20 + i]` with `b = 10 + 40 r`.
    pub fn rc(r: u8, i: u8) -> Rgb {
        let b = 10 + 40 * r;
        [b + i, b + 10 + i, b + 20 + i]
    }

    /// Ramps gray(0) steel(1) leather(2) gold(3) dark(4), curse = 4, cold = 3.
    /// Materials: 0 = steel/leather, 1 = leather/leather with dithering.
    pub fn sample() -> Vec<u8> {
        let mut p = alloc::vec![FORMAT_VERSION, 1, 0, 0x12, 0x0c, 0x16, 4, 4, 3, 5];
        for r in 0..5u8 {
            for i in 0..RAMP_COLORS {
                p.extend_from_slice(&rc(r, i));
            }
        }
        p.extend_from_slice(&[2, 1, 2, 0, 2, 2, MATERIAL_DITHER]);
        p
    }

    #[test]
    fn parses_sample() {
        let bytes = sample();
        let p = Palette::parse(&bytes).unwrap();
        assert_eq!(p.palette_version, 1);
        assert_eq!(p.outline, [0x12, 0x0c, 0x16]);
        assert_eq!((p.dark_ramp, p.curse_ramp, p.cold_ramp), (4, 4, 3));
        assert_eq!(p.ramp_count(), 5);
        assert_eq!(p.material_count(), 2);
        assert_eq!(p.material(0), (1, 2));
        assert_eq!((p.dithers(0), p.dithers(1)), (false, true));
        assert_eq!(p.color(1, 5), rc(1, 5));
    }

    #[test]
    fn rejections() {
        let ok = sample();
        for n in 0..ok.len() {
            assert_eq!(
                Palette::parse(&ok[..n]).err(),
                Some(Error::Palette),
                "prefix {n}"
            );
        }
        let bad = |i: usize, b: u8| {
            let mut v = ok.clone();
            v[i] = b;
            Palette::parse(&v).is_err()
        };
        let mut v = ok.clone();
        v.push(0);
        assert!(Palette::parse(&v).is_err(), "trailing");
        assert!(bad(0, 1), "format 1 belongs to rasterizer 1");
        assert!(bad(6, 5), "dark ramp");
        assert!(bad(7, 5), "curse ramp");
        assert!(bad(8, 5), "cold ramp");
        assert!(bad(9, 0), "no ramps");
        let last = ok.len() - 1;
        assert!(bad(last - 2, 9), "material ramp");
        assert!(bad(last, 2), "material flag bits");
        let m = HEADER_LEN + 5 * RAMP_LEN;
        assert!(bad(m, 0), "no materials");
    }
}
