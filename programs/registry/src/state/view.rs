//! Reading a registry `Table` from another program, without CPI (docs/ARCHITECTURE.md 4.2): owner,
//! discriminator and kind are checked, and a record must lie inside the written length. The
//! caller pins the table's address (its PDA `[kind.seed()]`).
use anchor_lang::prelude::*;
use anchor_lang::Discriminator;

use super::{Table, TableKind, TABLE_CAP};

/// Byte offsets inside a `Table` account (README.md).
pub mod off {
    pub const KIND: usize = 8 + 1;
    pub const LEN: usize = 8 + 4;
    pub const BODY: usize = 8 + 8;
}

/// The table account is not this program's `Table` of the expected kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotATable;

/// The body of the registry table `kind` held by `info`, as far as it is written.
pub fn table_body<'a>(
    info: &'a AccountInfo,
    kind: TableKind,
) -> core::result::Result<core::cell::Ref<'a, [u8]>, NotATable> {
    if info.owner != &crate::ID {
        return Err(NotATable);
    }
    let data = info.try_borrow_data().map_err(|_| NotATable)?;
    if data.len() < off::BODY + TABLE_CAP
        || data[..8] != *Table::DISCRIMINATOR
        || data[off::KIND] != kind as u8
    {
        return Err(NotATable);
    }
    let mut len = [0u8; 4];
    len.copy_from_slice(&data[off::LEN..off::LEN + 4]);
    let len = (u32::from_le_bytes(len) as usize).min(TABLE_CAP);
    Ok(core::cell::Ref::map(data, |d| {
        &d[off::BODY..off::BODY + len]
    }))
}

/// Record `index` of `body` (records of `kind`), `None` past the written length.
pub fn record(body: &[u8], kind: TableKind, index: usize) -> Option<&[u8]> {
    let size = kind.record_size();
    body.get(index * size..(index + 1) * size)
}
