//! Render request (`FORMAT.md` 3.4). A local copy of `deadroll_types::{RenderRequest, LayerRef}`
//! with the same Borsh layout, so the WASM build does not pull in `deadroll-types`
//! (and `solana-pubkey`). `tests/types_layout.rs` asserts the bytes are identical.

use alloc::vec::Vec;

use crate::error::{Error, Result};

/// Sprite layers in a request (`deadroll_types::LAYERS_MAX`).
pub const LAYERS_MAX: usize = 20;
/// `RenderRequest.flags` bit 0: dead character, all colors gray.
pub const FLAG_GRAY: u32 = 1;

/// One sprite layer. `sprite_id == 0` means an empty layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayerRef {
    pub sprite_id: u16,
    pub material_ramp: u8,
    pub accent_ramp: u8,
    pub element_ramp: u8,
}

impl LayerRef {
    pub const SIZE: usize = 2 + 1 + 1 + 1;

    pub const fn is_empty(&self) -> bool {
        self.sprite_id == 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderRequest {
    pub palette_version: u16,
    pub layers: [LayerRef; LAYERS_MAX],
    pub flags: u32,
}

impl RenderRequest {
    /// Borsh size in bytes.
    pub const SIZE: usize = 2 + LAYERS_MAX * LayerRef::SIZE + 4;

    /// Borsh decode. The input must be exactly [`Self::SIZE`] bytes.
    pub fn decode(b: &[u8]) -> Result<Self> {
        if b.len() != Self::SIZE {
            return Err(Error::Request);
        }
        let mut req = Self {
            palette_version: u16::from_le_bytes([b[0], b[1]]),
            ..Default::default()
        };
        for (i, l) in req.layers.iter_mut().enumerate() {
            let o = 2 + i * LayerRef::SIZE;
            *l = LayerRef {
                sprite_id: u16::from_le_bytes([b[o], b[o + 1]]),
                material_ramp: b[o + 2],
                accent_ramp: b[o + 3],
                element_ramp: b[o + 4],
            };
        }
        let o = 2 + LAYERS_MAX * LayerRef::SIZE;
        req.flags = u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        Ok(req)
    }

    /// Borsh encode.
    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(Self::SIZE);
        v.extend_from_slice(&self.palette_version.to_le_bytes());
        for l in &self.layers {
            v.extend_from_slice(&l.sprite_id.to_le_bytes());
            v.extend_from_slice(&[l.material_ramp, l.accent_ramp, l.element_ramp]);
        }
        v.extend_from_slice(&self.flags.to_le_bytes());
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_length() {
        let mut r = RenderRequest {
            palette_version: 0x0102,
            flags: 1,
            ..Default::default()
        };
        r.layers[3] = LayerRef {
            sprite_id: 7,
            material_ramp: 1,
            accent_ramp: 2,
            element_ramp: 3,
        };
        r.layers[19].sprite_id = 513;
        let b = r.encode();
        assert_eq!(b.len(), 106);
        assert_eq!(&b[..2], &[2, 1]);
        assert_eq!(RenderRequest::decode(&b), Ok(r));
        assert_eq!(RenderRequest::decode(&b[1..]), Err(Error::Request));
        let mut long = b.clone();
        long.push(0);
        assert_eq!(RenderRequest::decode(&long), Err(Error::Request));
    }
}
