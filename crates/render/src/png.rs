//! Minimal PNG encoder (`ART_DESCRIPTION.md` 13, `FORMAT.md` 4): indexed color (type 3, 8 bits)
//! with `PLTE` and `tRNS`, rows filtered None / Up, one zlib stream of one fixed-Huffman deflate
//! block whose repeats use only two distances, 1 and the row length.

use alloc::vec::Vec;

use crate::compose::{Image, MAX_SCALE};
use crate::error::{Error, Result};

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
/// Colors an indexed PNG can hold.
pub const PALETTE_MAX: usize = 256;
const FILTER_NONE: u8 = 0;
const FILTER_UP: u8 = 2;
const MATCH_MIN: usize = 3;
const MATCH_MAX: usize = 258;
const WINDOW: usize = 32_768;
const END_OF_BLOCK: u16 = 256;

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

const CRC_TABLE: [u32; 256] = crc_table();

const fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        t[n] = c;
        n += 1;
    }
    t
}

pub fn crc32(parts: &[&[u8]]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for part in parts {
        for &b in *part {
            c = CRC_TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
        }
    }
    c ^ 0xFFFF_FFFF
}

pub fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let (mut a, mut b) = (1u32, 0u32);
    // 5552 is the largest n with no u32 overflow before the modulo.
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
}

/// Deflate bit writer: values least significant bit first, Huffman codes most significant first.
struct Bits {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits {
    fn put(&mut self, value: u32, bits: u32) {
        self.acc |= value << self.n;
        self.n += bits;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    fn huffman(&mut self, code: u32, bits: u32) {
        self.put(code.reverse_bits() >> (32 - bits), bits);
    }

    /// Fixed literal/length code (RFC 1951 3.2.6).
    fn symbol(&mut self, sym: u16) {
        let s = sym as u32;
        match sym {
            0..=143 => self.huffman(0x30 + s, 8),
            144..=255 => self.huffman(0x190 + s - 144, 9),
            256..=279 => self.huffman(s - 256, 7),
            _ => self.huffman(0xC0 + s - 280, 8),
        }
    }

    fn repeat(&mut self, len: usize, dist: usize) {
        let i = LEN_BASE
            .iter()
            .rposition(|&b| b as usize <= len)
            .unwrap_or(0);
        self.symbol(257 + i as u16);
        self.put((len - LEN_BASE[i] as usize) as u32, LEN_EXTRA[i] as u32);
        let d = DIST_BASE
            .iter()
            .rposition(|&b| b as usize <= dist)
            .unwrap_or(0);
        self.huffman(d as u32, 5);
        self.put((dist - DIST_BASE[d] as usize) as u32, DIST_EXTRA[d] as u32);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

fn match_len(data: &[u8], i: usize, dist: usize) -> usize {
    if dist == 0 || dist > i || dist > WINDOW {
        return 0;
    }
    let max = MATCH_MAX.min(data.len() - i);
    (0..max)
        .take_while(|&k| data[i + k] == data[i + k - dist])
        .count()
}

/// zlib stream: one final fixed-Huffman block, greedy longest repeat at distance 1 or `row`.
pub fn zlib(data: &[u8], row: usize) -> Vec<u8> {
    let mut bits = Bits {
        out: Vec::with_capacity(data.len() / 8 + 16),
        acc: 0,
        n: 0,
    };
    bits.out.extend_from_slice(&[0x78, 0x01]);
    bits.put(1, 1); // BFINAL
    bits.put(1, 2); // BTYPE = fixed Huffman
    let mut i = 0;
    while i < data.len() {
        let (mut len, mut dist) = (match_len(data, i, 1), 1);
        let by_row = match_len(data, i, row);
        if by_row > len {
            (len, dist) = (by_row, row);
        }
        if len >= MATCH_MIN {
            bits.repeat(len, dist);
            i += len;
        } else {
            bits.symbol(data[i] as u16);
            i += 1;
        }
    }
    bits.symbol(END_OF_BLOCK);
    let mut z = bits.finish();
    z.extend_from_slice(&adler32(data).to_be_bytes());
    z
}

/// Encodes `img` enlarged `scale` times by nearest neighbor. The colors of the image become the
/// `PLTE` in the order they first appear (row-major), alpha below 255 goes to `tRNS`.
/// More than 256 colors is [`Error::Colors`].
pub fn encode(img: &Image, scale: u32) -> Result<Vec<u8>> {
    if !(1..=MAX_SCALE).contains(&scale) {
        return Err(Error::Request);
    }
    let s = scale as usize;
    let mut colors: Vec<[u8; 4]> = Vec::new();
    let mut index = Vec::with_capacity(img.width * img.height);
    for px in img.rgba.chunks_exact(4) {
        let c = [px[0], px[1], px[2], px[3]];
        let i = match colors.iter().position(|&k| k == c) {
            Some(i) => i,
            None if colors.len() == PALETTE_MAX => return Err(Error::Colors),
            None => {
                colors.push(c);
                colors.len() - 1
            }
        };
        index.push(i as u8);
    }

    // Each image row becomes `s` identical rows: the first one filtered None, the others Up
    // (all zeros after the filter).
    let (width, height) = (img.width * s, img.height * s);
    let row = width + 1;
    let mut raw = Vec::with_capacity(row * height);
    for y in 0..img.height {
        raw.push(FILTER_NONE);
        for &i in &index[y * img.width..(y + 1) * img.width] {
            raw.extend(core::iter::repeat_n(i, s));
        }
        for _ in 1..s {
            raw.push(FILTER_UP);
            raw.extend(core::iter::repeat_n(0, width));
        }
    }

    let mut ihdr = [0u8; 13];
    ihdr[0..4].copy_from_slice(&(width as u32).to_be_bytes());
    ihdr[4..8].copy_from_slice(&(height as u32).to_be_bytes());
    ihdr[8] = 8; // bit depth
    ihdr[9] = 3; // color type: indexed
                 // compression 0, filter 0, interlace 0
    let plte: Vec<u8> = colors.iter().flat_map(|c| [c[0], c[1], c[2]]).collect();
    let alpha_len = colors
        .iter()
        .rposition(|c| c[3] != 255)
        .map_or(0, |i| i + 1);
    let trns: Vec<u8> = colors[..alpha_len].iter().map(|c| c[3]).collect();
    let z = zlib(&raw, row);

    let mut out = Vec::with_capacity(z.len() + plte.len() + trns.len() + 80);
    out.extend_from_slice(&SIGNATURE);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"PLTE", &plte);
    if !trns.is_empty() {
        chunk(&mut out, b"tRNS", &trns);
    }
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::scale as upscale;
    use alloc::vec;

    /// Decodes to RGBA8 with the `png` crate (palette and tRNS expanded).
    fn decode(bytes: &[u8]) -> (u32, u32, png::ColorType, Vec<u8>) {
        let mut dec = png::Decoder::new(bytes);
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
        let mut reader = dec.read_info().unwrap();
        let color = reader.info().color_type;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        buf.truncate(info.buffer_size());
        (info.width, info.height, color, buf)
    }

    fn image(width: usize, height: usize, f: impl Fn(usize, usize) -> [u8; 4]) -> Image {
        let mut rgba = Vec::new();
        for y in 0..height {
            for x in 0..width {
                rgba.extend_from_slice(&f(x, y));
            }
        }
        Image {
            width,
            height,
            rgba,
        }
    }

    /// Inflates a zlib stream with the `png` crate's decoder by wrapping it in a 1-row gray PNG.
    fn inflate(z: &[u8], len: usize) -> Vec<u8> {
        let mut ihdr = [0u8; 13];
        ihdr[0..4].copy_from_slice(&((len - 1) as u32).to_be_bytes());
        ihdr[4..8].copy_from_slice(&1u32.to_be_bytes());
        ihdr[8] = 8;
        let mut out = Vec::from(SIGNATURE);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", z);
        chunk(&mut out, b"IEND", &[]);
        let mut reader = png::Decoder::new(&out[..]).read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut buf).unwrap();
        buf
    }

    #[test]
    fn known_checksums() {
        assert_eq!(crc32(&[b"123456789"]), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(&[]), 1);
        // Long input exercises the deferred modulo.
        let long = vec![0xFFu8; 100_000];
        let (mut a, mut b) = (1u64, 0u64);
        for &x in &long {
            a = (a + x as u64) % 65_521;
            b = (b + a) % 65_521;
        }
        assert_eq!(adler32(&long), ((b << 16) | a) as u32);
    }

    #[test]
    fn deflate_round_trips() {
        // Filter byte 0 (None) first, then literals 0..=255, long runs, a row repeat and every
        // length code: the inflated row must be the input.
        let mut data = vec![0u8];
        data.extend(0..=255u8);
        data.extend(core::iter::repeat_n(7, 600));
        let row: Vec<u8> = (0..40u8).collect();
        data.extend_from_slice(&row);
        data.extend_from_slice(&row);
        for len in 3..=258 {
            data.push(200);
            data.extend(core::iter::repeat_n(9, len));
        }
        let z = zlib(&data, 41);
        assert_eq!(inflate(&z, data.len()), data[1..]);
        // Header 78 01, then the fixed block flags.
        assert_eq!(&z[..2], &[0x78, 0x01]);
        assert_eq!(z[2] & 0b111, 0b011);
    }

    #[test]
    fn indexed_png_matches_scaled_rgba() {
        let img = image(32, 32, |x, y| match (x / 8 + y / 8) % 3 {
            0 => [0, 0, 0, 0],
            1 => [0, 0, 0, 90],
            _ => [(x / 4) as u8 * 30, (y / 4) as u8 * 30, 40, 255],
        });
        for s in [1, 2, 5, 16] {
            let bytes = encode(&img, s).unwrap();
            let (w, h, color, px) = decode(&bytes);
            assert_eq!(color, png::ColorType::Indexed);
            assert_eq!((w, h), (32 * s, 32 * s));
            assert_eq!(px, upscale(&img, s).unwrap().rgba, "scale {s}");
        }
    }

    #[test]
    fn chunks_filters_and_size() {
        // 64x64 sprite-like image at x8: a 512x512 PNG of a few kilobytes.
        let img = image(64, 64, |x, y| {
            if (16..48).contains(&x) && (8..60).contains(&y) {
                [(x / 4 * 16) as u8, (y / 4 * 16) as u8, 90, 255]
            } else {
                [0, 0, 0, 0]
            }
        });
        let bytes = encode(&img, 8).unwrap();
        assert!(bytes.len() < 8 * 1024, "{} bytes", bytes.len());
        assert_eq!(&bytes[12..16], b"IHDR");
        assert_eq!(bytes[16 + 9], 3, "color type");
        assert_eq!(&bytes[33 + 4..33 + 8], b"PLTE");
        // Transparent black is the first color: one tRNS entry.
        let plte_len = u32::from_be_bytes(bytes[33..37].try_into().unwrap()) as usize;
        let trns = 33 + 12 + plte_len;
        assert_eq!(&bytes[trns + 4..trns + 8], b"tRNS");
        assert_eq!(&bytes[trns..trns + 4], &1u32.to_be_bytes());
        assert_eq!(bytes[trns + 8], 0);
        // Fully opaque image: no tRNS chunk.
        let opaque = encode(&image(32, 32, |_, _| [1, 2, 3, 255]), 1).unwrap();
        assert!(!opaque.windows(4).any(|w| w == b"tRNS"));
        // Deterministic.
        assert_eq!(encode(&img, 8).unwrap(), bytes);
    }

    #[test]
    fn rejections() {
        let many = image(32, 32, |x, y| [x as u8, y as u8, 0, 255]);
        assert_eq!(encode(&many, 1), Err(Error::Colors), "1024 colors");
        let few = image(32, 8, |x, y| [x as u8, y as u8, 0, 255]);
        assert!(encode(&few, 1).is_ok(), "exactly 256 colors");
        assert_eq!(encode(&few, 0), Err(Error::Request));
        assert_eq!(encode(&few, MAX_SCALE + 1), Err(Error::Request));
    }
}
