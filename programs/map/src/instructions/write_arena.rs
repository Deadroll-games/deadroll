use anchor_lang::prelude::*;
use deadroll_types::PackLayout;

use crate::{events::ArenaWritten, instructions::WriteLayout, state::LayoutBytes};

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
    LayoutBytes::load(&ctx.accounts.layout)?.write_arena(pack, &record, &cells)?;
    emit!(ArenaWritten {
        map: ctx.accounts.map.key(),
        pack,
        width: record.width,
        height: record.height,
    });
    Ok(())
}
