use anchor_lang::prelude::*;

use crate::state::records::TableKind;

/// Body capacity of every table account (the largest table is the prefab book).
pub const TABLE_CAP: usize = 4096;

/// `["aspects"]`, `["resonances"]`, `["bestiary"]`, `["tileset"]`, `["prefabs"]`.
/// Fixed-size body, written in chunks by the authority. Record layouts: `state/records.rs`.
#[account(zero_copy)]
#[repr(C)]
pub struct Table {
    pub version: u8,
    pub kind: u8,
    pub bump: u8,
    pub _pad: u8,
    /// Bytes written so far (highest end offset).
    pub len: u32,
    pub body: [u8; TABLE_CAP],
}

impl Table {
    pub fn table_kind(&self) -> Option<TableKind> {
        TableKind::from_u8(self.kind)
    }
}
