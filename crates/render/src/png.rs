//! Minimal PNG encoder: RGBA8, one IDAT, zlib with stored deflate blocks (`FORMAT.md` 4).
//! TODO(post-demo): indexed PNG with fixed Huffman (ART_DESCRIPTION 13) for the wallet worker.

use alloc::vec::Vec;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const STORED_MAX: usize = 65_535;

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

/// Encodes `width x height` RGBA pixels. `rgba.len()` must be `width * height * 4`.
pub fn encode(width: usize, height: usize, rgba: &[u8]) -> Vec<u8> {
    let row = width * 4;
    let mut raw = Vec::with_capacity(height * (row + 1));
    for y in 0..height {
        raw.push(0); // filter None
        raw.extend_from_slice(&rgba[y * row..(y + 1) * row]);
    }

    let blocks = raw.len().div_ceil(STORED_MAX).max(1);
    let mut z = Vec::with_capacity(2 + raw.len() + blocks * 5 + 4);
    z.extend_from_slice(&[0x78, 0x01]);
    let mut chunks = raw.chunks(STORED_MAX).peekable();
    if raw.is_empty() {
        z.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(block) = chunks.next() {
        let last = chunks.peek().is_none();
        let len = block.len() as u16;
        z.push(last as u8);
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut ihdr = [0u8; 13];
    ihdr[0..4].copy_from_slice(&(width as u32).to_be_bytes());
    ihdr[4..8].copy_from_slice(&(height as u32).to_be_bytes());
    ihdr[8] = 8; // bit depth
    ihdr[9] = 6; // color type RGBA
                 // compression 0, filter 0, interlace 0

    let mut out = Vec::with_capacity(z.len() + 64);
    out.extend_from_slice(&SIGNATURE);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let dec = png::Decoder::new(bytes);
        let mut reader = dec.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);
        buf.truncate(info.buffer_size());
        (info.width, info.height, buf)
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
    fn png_crate_reads_small_image() {
        let rgba: Vec<u8> = (0..16 * 16 * 4).map(|i| (i * 7 % 256) as u8).collect();
        let (w, h, px) = decode(&encode(16, 16, &rgba));
        assert_eq!((w, h), (16, 16));
        assert_eq!(px, rgba);
    }

    #[test]
    fn png_crate_reads_multi_block_image() {
        // 512x512 RGBA = 1 MiB raw, 17 stored blocks.
        let rgba: Vec<u8> = (0..512 * 512 * 4).map(|i| (i % 251) as u8).collect();
        let (w, h, px) = decode(&encode(512, 512, &rgba));
        assert_eq!((w, h), (512, 512));
        assert_eq!(px, rgba);
    }
}
