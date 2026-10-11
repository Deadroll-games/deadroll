//! Golden test: the reference warrior (pixel-sprite skill) with the demo palette.
//! Also writes `target/render-golden/{input.bin,expected.sha256,warrior.png}` for the WASM
//! parity check (`crates/render/wasm/golden.mjs`).

use deadroll_render::text::{compile_palette, compile_sprites, PaletteSrc};
use deadroll_render::{book, encode_input, render, LayerRef, RenderRequest, OUT_PNG, OUT_RGBA};
use sha2::{Digest, Sha256};

/// SHA-256 of the x8 PNG. Changing it means the rendering rules changed: new rasterizer version.
const WARRIOR_PNG_SHA256: &str = "2e14ab1c59f6883c8e344e8738c5a67b46a744ebc127cdab50ca05b44b8a2e5b";

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn palette() -> PaletteSrc {
    let src = std::fs::read_to_string(root().join("assets/palette/palette_1.pal")).unwrap();
    compile_palette(&src).unwrap()
}

/// `(sprite id, layer index, material, accent ramp, element ramp)`.
const LAYERS: &[(u16, usize, &str, &str, &str)] = &[
    (1, 0, "stone", "gray", "gray"),
    (100, 1, "cape", "gold", "gray"),
    (2, 2, "human", "eyes", "gray"),
    (101, 4, "steel", "gold", "gray"),
    (3, 5, "human", "eyes", "gray"),
    (102, 6, "leather", "gold", "gray"),
    (103, 7, "leather", "gold", "gray"),
    (4, 8, "human", "eyes", "gray"),
    (104, 9, "steel", "gold", "gray"),
    (5, 11, "human", "eyes", "gray"),
    (6, 12, "human", "eyes", "gray"),
    (105, 13, "steel", "gold", "gray"),
    (7, 14, "human", "eyes", "gray"),
    (106, 16, "steel", "gold", "gray"),
];

fn warrior_input(flags: u32) -> Vec<u8> {
    let pal = palette();
    let src = std::fs::read_to_string(root().join("crates/render/tests/data/warrior.dsl")).unwrap();
    let sprites = compile_sprites(&src).unwrap();
    assert_eq!(sprites.len(), LAYERS.len());
    let list: Vec<(u16, &[u8])> = sprites.iter().map(|s| (s.id, &s.bytes[..])).collect();
    let book = book::pack(1, &list).unwrap();

    let mut req = RenderRequest {
        palette_version: pal.palette_version,
        flags,
        ..Default::default()
    };
    for &(id, ix, m, a, e) in LAYERS {
        // "gray" as element means none (ramp 0).
        let element = if e == "gray" { 0 } else { pal.ramp(e).unwrap() };
        req.layers[ix] = LayerRef {
            sprite_id: id,
            material_ramp: pal.material(m).unwrap(),
            accent_ramp: pal.ramp(a).unwrap(),
            element_ramp: element,
        };
    }
    encode_input(&req.encode(), &[0; 32], &pal.bytes, &[&book])
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn warrior_png_is_stable() {
    let input = warrior_input(0);
    let out = render(&input, OUT_PNG, 8).unwrap();
    assert_eq!((out.width, out.height), (256, 256));
    let sha = hex(&Sha256::digest(&out.bytes));

    let dir = root().join("target/render-golden");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("input.bin"), &input).unwrap();
    std::fs::write(dir.join("expected.sha256"), &sha).unwrap();
    std::fs::write(dir.join("warrior.png"), &out.bytes).unwrap();
    // Palette and sprite book bodies, as uploaded on-chain (tools/scripts/upload-*.ts dry runs).
    let parts = deadroll_render::parse_input(&input).unwrap();
    std::fs::write(dir.join("palette.bin"), parts.palette).unwrap();
    std::fs::write(dir.join("book.bin"), parts.books[0]).unwrap();

    // Deterministic: same bytes on a second run.
    assert_eq!(render(&input, OUT_PNG, 8).unwrap(), out);
    assert_eq!(
        sha, WARRIOR_PNG_SHA256,
        "rendering changed; see target/render-golden/warrior.png"
    );
}

#[test]
fn warrior_rgba_matches_png_pixels() {
    let input = warrior_input(0);
    let rgba = render(&input, OUT_RGBA, 2).unwrap();
    let png_bytes = render(&input, OUT_PNG, 2).unwrap().bytes;
    let mut reader = png::Decoder::new(&png_bytes[..]).read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    assert_eq!((info.width, info.height), (rgba.width, rgba.height));
    assert_eq!(buf, rgba.bytes);
}

#[test]
fn dead_warrior_is_gray() {
    let out = render(&warrior_input(1), OUT_RGBA, 1).unwrap();
    for px in out.bytes.chunks_exact(4).filter(|p| p[3] != 0) {
        assert!(px[0] == px[1] && px[1] == px[2], "{px:?}");
    }
}

#[test]
fn input_rejections() {
    let input = warrior_input(0);
    assert!(render(&input, 2, 1).is_err(), "out mode");
    assert!(render(&input, OUT_RGBA, 0).is_err(), "scale 0");
    assert!(render(&input, OUT_RGBA, 17).is_err(), "scale 17");
    assert!(
        render(&input[..input.len() - 1], OUT_RGBA, 1).is_err(),
        "truncated book"
    );
    assert!(
        render(&input[..10], OUT_RGBA, 1).is_err(),
        "truncated request"
    );
    let mut no_books = input.clone();
    let pal_len = palette().bytes.len();
    no_books.truncate(4 + 106 + 32 + 4 + pal_len);
    assert_eq!(
        render(&no_books, OUT_RGBA, 1).unwrap_err().code(),
        9,
        "no books"
    );
}
