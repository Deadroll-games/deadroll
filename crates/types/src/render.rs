//! Sprite render input, shared with `crates/render` (agents/CONTRACTS.md, section 3).

use crate::LAYERS_MAX;

shared! {
    /// One sprite layer. `sprite_id == 0` means an empty layer.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct LayerRef {
        pub sprite_id: u16,
        pub material_ramp: u8,
        pub accent_ramp: u8,
        pub element_ramp: u8,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct RenderRequest {
        pub palette_version: u16,
        pub layers: [LayerRef; LAYERS_MAX],
        pub flags: u32,
    }
}

impl LayerRef {
    pub const SIZE: usize = 2 + 1 + 1 + 1;
    pub const EMPTY: LayerRef = LayerRef {
        sprite_id: 0,
        material_ramp: 0,
        accent_ramp: 0,
        element_ramp: 0,
    };

    pub const fn is_empty(&self) -> bool {
        self.sprite_id == 0
    }
}

impl RenderRequest {
    pub const SIZE: usize = 2 + LAYERS_MAX * LayerRef::SIZE + 4;
}
