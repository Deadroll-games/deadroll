//! Palette body (`FORMAT.md` 3.2).

use crate::error::{Error, Result};

pub const FORMAT_VERSION: u8 = 1;
pub const RAMP_LEN: usize = 12;

pub type Rgb = [u8; 3];

#[derive(Clone, Copy, Debug)]
pub struct Palette<'a> {
    pub palette_version: u16,
    pub outline: Rgb,
    pub dark_ramp: u8,
    ramps: &'a [u8],
    materials: &'a [u8],
}

impl<'a> Palette<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        // format, version(2), outline(3), dark_ramp, ramp_count
        if bytes.len() < 8 {
            return Err(Error::Palette);
        }
        if bytes[0] != FORMAT_VERSION {
            return Err(Error::Palette);
        }
        let palette_version = u16::from_le_bytes([bytes[1], bytes[2]]);
        let outline = [bytes[3], bytes[4], bytes[5]];
        let dark_ramp = bytes[6];
        let ramp_count = bytes[7] as usize;
        let ramps_end = 8 + ramp_count * RAMP_LEN;
        if ramp_count == 0 || bytes.len() < ramps_end + 1 {
            return Err(Error::Palette);
        }
        let ramps = &bytes[8..ramps_end];
        let material_count = bytes[ramps_end] as usize;
        let materials = &bytes[ramps_end + 1..];
        if material_count == 0 || materials.len() != material_count * 2 {
            return Err(Error::Palette);
        }
        if dark_ramp as usize >= ramp_count || materials.iter().any(|&r| r as usize >= ramp_count) {
            return Err(Error::Palette);
        }
        Ok(Self {
            palette_version,
            outline,
            dark_ramp,
            ramps,
            materials,
        })
    }

    pub fn ramp_count(&self) -> usize {
        self.ramps.len() / RAMP_LEN
    }

    pub fn material_count(&self) -> usize {
        self.materials.len() / 2
    }

    /// Color `i` (0..4) of a ramp. Indices are validated by the caller.
    pub fn color(&self, ramp: u8, i: u8) -> Rgb {
        let o = ramp as usize * RAMP_LEN + i as usize * 3;
        [self.ramps[o], self.ramps[o + 1], self.ramps[o + 2]]
    }

    /// `(primary_ramp, secondary_ramp)` of a material. Index validated by the caller.
    pub fn material(&self, m: u8) -> (u8, u8) {
        let o = m as usize * 2;
        (self.materials[o], self.materials[o + 1])
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// gray, steel, leather, gold, dark; materials: steel/leather.
    pub fn sample() -> Vec<u8> {
        let mut p = alloc::vec![1, 1, 0, 0x12, 0x0c, 0x16, 4, 5];
        for base in [10u8, 50, 90, 130, 170] {
            for i in 0..4u8 {
                p.extend_from_slice(&[base + i, base + 10 + i, base + 20 + i]);
            }
        }
        p.extend_from_slice(&[1, 1, 2]);
        p
    }

    #[test]
    fn parses_sample() {
        let bytes = sample();
        let p = Palette::parse(&bytes).unwrap();
        assert_eq!(p.palette_version, 1);
        assert_eq!(p.outline, [0x12, 0x0c, 0x16]);
        assert_eq!(p.ramp_count(), 5);
        assert_eq!(p.material(0), (1, 2));
        assert_eq!(p.color(1, 3), [53, 63, 73]);
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
        let mut v = ok.clone();
        v.push(0);
        assert!(Palette::parse(&v).is_err(), "trailing");
        let mut v = ok.clone();
        v[0] = 2;
        assert!(Palette::parse(&v).is_err(), "format");
        let mut v = ok.clone();
        v[6] = 5;
        assert!(Palette::parse(&v).is_err(), "dark ramp");
        let mut v = ok.clone();
        let last = v.len() - 1;
        v[last] = 9;
        assert!(Palette::parse(&v).is_err(), "material ramp");
    }
}
