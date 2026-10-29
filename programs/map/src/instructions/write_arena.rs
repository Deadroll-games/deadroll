use anchor_lang::prelude::*;
use deadroll_types::{MapArenasData, PackLayout, ARENA_CELLS, ARENA_SIDE_MAX, PACKS_MAX};

use crate::{errors::MapError, events::ArenaWritten, instructions::WriteLayout};

/// Arena update (docs/MAP_DESCRIPTION.md 17.2, 17.3): replaces pack `pack`'s record (the monster
/// shown on the map, its map cell, the facing, the arena size) and its arena, `cells` row by row
/// (`record.width` cells per row, `cell_byte`). A record of width 0 clears the pack. The rules are
/// checked at `finalize_map`, so the builder can save a half-done map.
pub fn handler(
    ctx: Context<WriteLayout>,
    pack: u8,
    record: PackLayout,
    cells: Vec<u8>,
) -> Result<()> {
    store(&mut ctx.accounts.layout.arenas, pack, record, &cells)?;
    emit!(ArenaWritten {
        map: ctx.accounts.map.key(),
        pack,
        width: record.width,
        height: record.height,
    });
    Ok(())
}

/// Writes the record and the rows (`ARENA_SIDE_MAX` cells per stored row, the rest cleared).
fn store(
    arenas: &mut MapArenasData,
    pack: u8,
    record: PackLayout,
    cells: &[u8],
) -> std::result::Result<(), MapError> {
    if pack as usize >= PACKS_MAX {
        return Err(MapError::PackOutOfRange);
    }
    let (w, h) = (record.width as usize, record.height as usize);
    if w > ARENA_SIDE_MAX || h > ARENA_SIDE_MAX || cells.len() != w * h {
        return Err(MapError::ArenaCellsMismatch);
    }
    arenas.packs[pack as usize] = record;
    let stored = &mut arenas.cells[pack as usize];
    *stored = [0; ARENA_CELLS];
    for (y, row) in cells.chunks(w.max(1)).take(h).enumerate() {
        stored[y * ARENA_SIDE_MAX..y * ARENA_SIDE_MAX + row.len()].copy_from_slice(row);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_types::Facing;

    fn store(
        arenas: &mut MapArenasData,
        pack: u8,
        record: PackLayout,
        cells: &[u8],
    ) -> std::result::Result<(), u32> {
        super::store(arenas, pack, record, cells).map_err(|e| e as u32)
    }

    fn record(width: u8, height: u8) -> PackLayout {
        PackLayout {
            visible: 1,
            x: 4,
            y: 5,
            facing: Facing::SW,
            width,
            height,
        }
    }

    #[test]
    fn rows_are_stored_with_the_full_stride() {
        let mut arenas = MapArenasData::EMPTY;
        arenas.cells[2] = [9; ARENA_CELLS];
        let cells: Vec<u8> = (1..=6).collect();
        assert_eq!(store(&mut arenas, 2, record(3, 2), &cells), Ok(()));
        assert_eq!(arenas.packs[2], record(3, 2));
        assert_eq!(&arenas.cells[2][..4], &[1, 2, 3, 0]);
        assert_eq!(
            &arenas.cells[2][ARENA_SIDE_MAX..ARENA_SIDE_MAX + 4],
            &[4, 5, 6, 0]
        );
        // The old cells are gone.
        assert_eq!(arenas.cell(2, 11, 11), Some(0));
        // The largest arena fits.
        let full = vec![7; ARENA_CELLS];
        let side = ARENA_SIDE_MAX as u8;
        assert_eq!(store(&mut arenas, 7, record(side, side), &full), Ok(()));
        assert_eq!(arenas.cells[7], [7; ARENA_CELLS]);
        // Width 0 clears the pack.
        assert_eq!(store(&mut arenas, 2, PackLayout::EMPTY, &[]), Ok(()));
        assert!(!arenas.packs[2].is_used());
        assert_eq!(arenas.cells[2], [0; ARENA_CELLS]);
    }

    #[test]
    fn rejects_a_pack_number_past_the_cap() {
        let mut arenas = MapArenasData::EMPTY;
        assert_eq!(
            store(&mut arenas, PACKS_MAX as u8, record(1, 1), &[1]),
            Err(MapError::PackOutOfRange as u32)
        );
    }

    #[test]
    fn rejects_cells_that_do_not_match_the_size() {
        let mut arenas = MapArenasData::EMPTY;
        assert_eq!(
            store(&mut arenas, 1, record(3, 2), &[1; 5]),
            Err(MapError::ArenaCellsMismatch as u32)
        );
        let side = ARENA_SIDE_MAX as u8 + 1;
        let cells = vec![1; side as usize];
        assert_eq!(
            store(&mut arenas, 1, record(side, 1), &cells),
            Err(MapError::ArenaCellsMismatch as u32)
        );
        assert_eq!(arenas, MapArenasData::EMPTY);
    }
}
