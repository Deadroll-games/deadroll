//! Host-only: the local `RenderRequest` in `crates/render` must have exactly the Borsh bytes of
//! `deadroll_types::RenderRequest` (agents/CONTRACTS.md 3), which the character program stores.

use deadroll_render::{LayerRef, RenderRequest, LAYERS_MAX};
use deadroll_types as types;

fn pair(seed: u32) -> (RenderRequest, types::RenderRequest) {
    let mut ours = RenderRequest {
        palette_version: seed as u16 ^ 0xA55A,
        flags: seed.rotate_left(7),
        ..Default::default()
    };
    let mut theirs = types::RenderRequest {
        palette_version: ours.palette_version,
        flags: ours.flags,
        ..Default::default()
    };
    for i in 0..LAYERS_MAX {
        let v = seed
            .wrapping_mul(2_654_435_761)
            .wrapping_add(i as u32 * 40_503);
        let l = LayerRef {
            sprite_id: v as u16,
            material_ramp: (v >> 16) as u8,
            accent_ramp: (v >> 8) as u8,
            element_ramp: (v >> 24) as u8,
        };
        ours.layers[i] = l;
        theirs.layers[i] = types::LayerRef {
            sprite_id: l.sprite_id,
            material_ramp: l.material_ramp,
            accent_ramp: l.accent_ramp,
            element_ramp: l.element_ramp,
        };
    }
    (ours, theirs)
}

#[test]
fn same_constants() {
    assert_eq!(LAYERS_MAX, types::LAYERS_MAX);
    assert_eq!(LayerRef::SIZE, types::LayerRef::SIZE);
    assert_eq!(RenderRequest::SIZE, types::RenderRequest::SIZE);
}

#[test]
fn identical_borsh_bytes() {
    for seed in [0u32, 1, 0xFFFF_FFFF, 12_345, 0xDEAD_BEEF] {
        let (ours, theirs) = pair(seed);
        let theirs_bytes = borsh::to_vec(&theirs).unwrap();
        assert_eq!(ours.encode(), theirs_bytes, "seed {seed}");
        assert_eq!(
            RenderRequest::decode(&theirs_bytes).unwrap(),
            ours,
            "seed {seed}"
        );
    }
    assert_eq!(
        borsh::to_vec(&types::RenderRequest::default()).unwrap(),
        RenderRequest::default().encode()
    );
}
