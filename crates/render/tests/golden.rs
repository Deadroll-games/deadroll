//! Golden test: the reference warrior (pixel-sprite skill) with the demo palette.
//! Also writes `target/render-golden/{state,sprites,palette,book}.bin`, `expected.sha256` and
//! `warrior.png` for the WASM parity check (`crates/render/wasm/golden.mjs`), the app and the
//! validator tests.

use deadroll_render::text::{compile_palette, compile_sprites, PaletteSrc};
use deadroll_render::{
    book, encode_books, png_scale, render, scale, LayerRef, OutputMode, RenderRequest, ASSET_LEN,
};
use sha2::{Digest, Sha256};

/// SHA-256 of the PNG (the canvas enlarged to at most 512 px). Changing it means the rendering rules changed: new rasterizer version.
const WARRIOR_PNG_SHA256: &str = "ed76d4bf42d112220ad546d2bfa83ac9d8be1bd4a593b502a3cd07d579f7f861";
const ASSET: [u8; ASSET_LEN] = [0; ASSET_LEN];

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn palette() -> PaletteSrc {
    let src = std::fs::read_to_string(root().join("assets/palette/palette_2.pal")).unwrap();
    compile_palette(&src).unwrap()
}

/// `(sprite id, layer index, material, accent ramp, element ramp)`.
const LAYERS: &[(u16, usize, &str, &str, &str)] = &[
    (1, 0, "stone", "gray", "gray"),
    (100, 1, "cape", "gold", "gray"),
    (10, 2, "human", "eyes", "gray"),
    (102, 4, "leather", "gold", "gray"),
    (103, 5, "leather", "gold", "gray"),
    (104, 6, "steel", "gold", "gray"),
    (11, 8, "human", "eyes", "gray"),
    (101, 9, "steel", "gold", "gray"),
    (105, 10, "steel", "gold", "gray"),
    (106, 11, "steel", "gold", "gray"),
    (14, 12, "human", "eyes", "gray"),
];

/// The render arguments of the warrior: `(sprites, palette, state)`.
struct Args {
    sprites: Vec<u8>,
    book: Vec<u8>,
    palette: Vec<u8>,
    state: Vec<u8>,
}

fn warrior(flags: u32) -> Args {
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
    Args {
        sprites: encode_books(&[&book]),
        book,
        palette: pal.bytes,
        state: req.encode(),
    }
}

fn run(a: &Args, mode: OutputMode) -> deadroll_render::Result<deadroll_render::Output> {
    render(&a.sprites, &a.palette, &a.state, &ASSET, mode)
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn warrior_png_is_stable() {
    let a = warrior(0);
    let out = run(&a, OutputMode::Png).unwrap();
    // The canvas enlarged by the integer scale that fits 512 px.
    let raw = run(&a, OutputMode::RawRgba).unwrap();
    let k = png_scale(raw.width as usize, raw.height as usize);
    assert_eq!((out.width, out.height), (raw.width * k, raw.height * k));
    assert!(out.width <= 512 && out.width + raw.width > 512);
    let sha = hex(&Sha256::digest(&out.bytes));

    let dir = root().join("target/render-golden");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in [
        ("state.bin", &a.state),
        ("sprites.bin", &a.sprites),
        // Palette and sprite book bodies, as uploaded on-chain (tools/scripts/upload-*.ts dry runs).
        ("palette.bin", &a.palette),
        ("book.bin", &a.book),
        ("warrior.png", &out.bytes),
    ] {
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    std::fs::write(dir.join("expected.sha256"), &sha).unwrap();
    let _ = std::fs::remove_file(dir.join("input.bin"));

    // Deterministic: same bytes on a second run.
    assert_eq!(run(&a, OutputMode::Png).unwrap(), out);
    assert_eq!(
        sha, WARRIOR_PNG_SHA256,
        "rendering changed; see target/render-golden/warrior.png"
    );
}

#[test]
fn warrior_png_pixels_are_the_scaled_rgba() {
    let a = warrior(0);
    let rgba = run(&a, OutputMode::RawRgba).unwrap();
    let (w, h) = (rgba.width as usize, rgba.height as usize);
    let k = png_scale(w, h);
    let png_bytes = run(&a, OutputMode::Png).unwrap().bytes;
    let mut dec = png::Decoder::new(&png_bytes[..]);
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = dec.read_info().unwrap();
    assert_eq!(reader.info().color_type, png::ColorType::Indexed);
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    let img = deadroll_render::Image {
        width: w,
        height: h,
        rgba: rgba.bytes,
    };
    assert_eq!(buf, scale(&img, k).unwrap().rgba);
    // Wallet-sized and small: a few kilobytes.
    assert!(png_bytes.len() < 8 * 1024, "{} bytes", png_bytes.len());
}

#[test]
fn dead_warrior_has_gray_ramps() {
    let pal = palette();
    let outline = deadroll_render::Palette::parse(&pal.bytes).unwrap().outline;
    let alive = run(&warrior(0), OutputMode::RawRgba).unwrap().bytes;
    let dead = run(&warrior(1), OutputMode::RawRgba).unwrap().bytes;
    for (a, d) in alive.chunks_exact(4).zip(dead.chunks_exact(4)) {
        assert_eq!(a[3], d[3], "same alpha");
        if a[..3] == outline {
            assert_eq!(a, d, "the palette outline is not a ramp");
        } else {
            assert!(d[0] == d[1] && d[1] == d[2], "{d:?}");
        }
    }
}

#[test]
fn rejections() {
    let a = warrior(0);
    let r = |sprites: &[u8], palette: &[u8], state: &[u8]| {
        render(sprites, palette, state, &ASSET, OutputMode::RawRgba).map_err(|e| e.code())
    };
    assert_eq!(
        OutputMode::from_u32(2),
        Err(deadroll_render::Error::Request)
    );
    assert_eq!(OutputMode::from_u32(1), Ok(OutputMode::Png));
    assert_eq!(
        r(&a.sprites[..a.sprites.len() - 1], &a.palette, &a.state),
        Err(1),
        "truncated book"
    );
    assert_eq!(
        r(&a.sprites[..3], &a.palette, &a.state),
        Err(1),
        "truncated length"
    );
    assert_eq!(r(&[], &a.palette, &a.state), Err(9), "no books");
    assert_eq!(
        r(&a.sprites, &a.palette, &a.state[..10]),
        Err(9),
        "short state"
    );
    assert_eq!(r(&a.sprites, &a.palette[..10], &a.state), Err(8), "palette");
    // Two books: an id is found in the second one.
    let two = encode_books(&[&book::pack(9, &[]).unwrap(), &a.book]);
    assert!(r(&two, &a.palette, &a.state).is_ok());
}
