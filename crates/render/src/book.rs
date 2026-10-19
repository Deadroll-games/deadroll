//! Sprite book body (`FORMAT.md` 3.3).

use alloc::vec::Vec;

use crate::error::{Error, Result};

pub const FORMAT_VERSION: u8 = 1;
pub const HEADER_LEN: usize = 5;
pub const ENTRY_LEN: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct Book<'a> {
    pub book_id: u16,
    bytes: &'a [u8],
    count: usize,
}

impl<'a> Book<'a> {
    /// Checks the header and that the index is sorted and in bounds.
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < HEADER_LEN || bytes[0] != FORMAT_VERSION {
            return Err(Error::Sprite);
        }
        let book_id = u16::from_le_bytes([bytes[1], bytes[2]]);
        let count = u16::from_le_bytes([bytes[3], bytes[4]]) as usize;
        let data_start = HEADER_LEN + count * ENTRY_LEN;
        if bytes.len() < data_start {
            return Err(Error::Sprite);
        }
        let book = Self {
            book_id,
            bytes,
            count,
        };
        let mut prev: Option<u16> = None;
        for i in 0..count {
            let (id, off, len) = book.entry(i);
            if id == 0 || prev.is_some_and(|p| p >= id) {
                return Err(Error::Sprite);
            }
            if off < data_start || off.checked_add(len).is_none_or(|end| end > bytes.len()) {
                return Err(Error::Sprite);
            }
            prev = Some(id);
        }
        Ok(book)
    }

    fn entry(&self, i: usize) -> (u16, usize, usize) {
        let e = &self.bytes[HEADER_LEN + i * ENTRY_LEN..HEADER_LEN + (i + 1) * ENTRY_LEN];
        let id = u16::from_le_bytes([e[0], e[1]]);
        let off = u32::from_le_bytes([e[2], e[3], e[4], e[5]]) as usize;
        let len = u16::from_le_bytes([e[6], e[7]]) as usize;
        (id, off, len)
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The sprite bytes by id (binary search over the sorted index).
    pub fn get(&self, id: u16) -> Option<&'a [u8]> {
        let (mut lo, mut hi) = (0usize, self.count);
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (eid, off, len) = self.entry(mid);
            match eid.cmp(&id) {
                core::cmp::Ordering::Equal => return Some(&self.bytes[off..off + len]),
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
            }
        }
        None
    }
}

/// Packs sprites into a book body. Sprites are sorted by id; duplicate ids or a sprite
/// longer than `u16::MAX` bytes are an error. Sprites with identical bytes are stored once and
/// their index entries share the offset (an elf body that copies the human one costs no bytes).
pub fn pack(book_id: u16, sprites: &[(u16, &[u8])]) -> Result<Vec<u8>> {
    let mut sorted: Vec<(u16, &[u8])> = sprites.to_vec();
    sorted.sort_by_key(|s| s.0);
    if sorted.windows(2).any(|w| w[0].0 == w[1].0) || sorted.iter().any(|s| s.0 == 0) {
        return Err(Error::Sprite);
    }
    if sorted.len() > u16::MAX as usize {
        return Err(Error::Sprite);
    }
    let mut out = Vec::new();
    out.push(FORMAT_VERSION);
    out.extend_from_slice(&book_id.to_le_bytes());
    out.extend_from_slice(&(sorted.len() as u16).to_le_bytes());
    let mut off = HEADER_LEN + sorted.len() * ENTRY_LEN;
    let mut blobs: Vec<(&[u8], usize)> = Vec::new();
    for (id, bytes) in &sorted {
        let len = u16::try_from(bytes.len()).map_err(|_| Error::Sprite)?;
        let at = match blobs.iter().find(|b| b.0 == *bytes) {
            Some(b) => b.1,
            None => {
                blobs.push((bytes, off));
                off += bytes.len();
                off - bytes.len()
            }
        };
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&(at as u32).to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
    }
    for (bytes, _) in &blobs {
        out.extend_from_slice(bytes);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_and_lookup() {
        let b = pack(7, &[(30, &[3, 3, 3][..]), (2, &[1][..]), (11, &[2, 2][..])]).unwrap();
        let book = Book::parse(&b).unwrap();
        assert_eq!(book.book_id, 7);
        assert_eq!(book.len(), 3);
        assert_eq!(book.get(2), Some(&[1u8][..]));
        assert_eq!(book.get(11), Some(&[2u8, 2][..]));
        assert_eq!(book.get(30), Some(&[3u8, 3, 3][..]));
        assert_eq!(book.get(5), None);
    }

    #[test]
    fn identical_sprites_share_bytes() {
        let one = pack(1, &[(2, &[1, 2, 3][..]), (9, &[4][..])]).unwrap();
        let alias = pack(
            1,
            &[(2, &[1, 2, 3][..]), (9, &[4][..]), (20, &[1, 2, 3][..])],
        )
        .unwrap();
        assert_eq!(alias.len(), one.len() + ENTRY_LEN);
        let book = Book::parse(&alias).unwrap();
        assert_eq!(book.get(20), Some(&[1u8, 2, 3][..]));
        assert_eq!(book.get(2), book.get(20));
        assert_eq!(book.get(9), Some(&[4u8][..]));
    }

    #[test]
    fn rejections() {
        assert!(pack(1, &[(2, &[1][..]), (2, &[1][..])]).is_err());
        assert!(pack(1, &[(0, &[1][..])]).is_err());
        let ok = pack(1, &[(2, &[1][..]), (3, &[1][..])]).unwrap();
        for n in 0..ok.len() {
            assert!(Book::parse(&ok[..n]).is_err(), "prefix {n}");
        }
        let mut v = ok.clone();
        v[0] = 2;
        assert!(Book::parse(&v).is_err(), "format");
        let mut unsorted = ok.clone();
        unsorted[5..7].copy_from_slice(&3u16.to_le_bytes());
        assert!(Book::parse(&unsorted).is_err(), "not ascending");
        let mut oob = ok.clone();
        oob[7..11].copy_from_slice(&1000u32.to_le_bytes());
        assert!(Book::parse(&oob).is_err(), "offset out of bounds");
        let mut into_index = ok.clone();
        into_index[7..11].copy_from_slice(&0u32.to_le_bytes());
        assert!(Book::parse(&into_index).is_err(), "offset inside the index");
    }
}
