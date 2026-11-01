use anchor_lang::prelude::*;
use deadroll_mapcheck::{
    loot::chest_cells,
    tiles::TileRules,
    wide::{self, CheckState, Row, STEP_BUDGET},
};
use deadroll_registry::state::{view::table_body, TableKind};
use deadroll_rules::tiles::tile_rules;
use deadroll_types::{seeds, Cell, MapStatus, MAP_ROW_WORDS};

use crate::{
    errors::MapError,
    events::MapChecked,
    instructions::finalize_map::TILESET_TABLE,
    state::{LayoutBytes, Map},
};

#[derive(Accounts)]
pub struct CheckStep<'info> {
    pub dm: Signer<'info>,
    #[account(
        seeds = [seeds::MAP, map.core.dm.as_ref(), &map.core.index.to_le_bytes()],
        bump = map.core.bump,
        constraint = map.core.dm == dm.key() @ MapError::NotDm,
        constraint = map.core.status == MapStatus::Draft @ MapError::NotDraft,
    )]
    pub map: Account<'info, Map>,
    /// CHECK: the map's `MapLayout` (seeds), checked by `LayoutBytes::load`; the check's state
    /// and bitsets are written in place.
    #[account(mut, seeds = [seeds::LAYOUT, map.key().as_ref()], bump)]
    pub layout: UncheckedAccount<'info>,
    /// CHECK: registry `["tileset"]` table: which tiles exist and can be entered.
    #[account(address = TILESET_TABLE @ MapError::InvalidRegistryTable)]
    pub tileset: UncheckedAccount<'info>,
}

/// The registry `Tileset` as the map check takes it.
pub fn tiles_of(tileset: &AccountInfo) -> Result<TileRules> {
    Ok(tile_rules(
        &table_body(tileset, TableKind::Tileset)
            .map_err(|_| error!(MapError::InvalidRegistryTable))?,
    ))
}

/// Patch D: runs the layout's reachability check (`deadroll_mapcheck::wide`) from where it
/// stopped for `budget` work units, from the entrance around the closed chests, and stores where
/// it got to. Returns the state.
pub fn advance(layout: &LayoutBytes, tiles: &TileRules, budget: u32) -> Result<CheckState> {
    let mut state = layout.check_state()?;
    if state.done() {
        return Ok(state);
    }
    let g = layout.geom;
    let h = g.height as usize;
    let mut pass: Vec<Row> = vec![[0; MAP_ROW_WORDS]; h];
    let mut open: Vec<Row> = vec![[0; MAP_ROW_WORDS]; h];
    let mut reach: Vec<Row> = vec![[0; MAP_ROW_WORDS]; h];
    layout.load_rows(&mut pass, &mut reach)?;
    let core = Box::new(g.core(&layout.data()?));
    let (chests, count) = chest_cells(&core);
    let start = core.entrance().unwrap_or(Cell {
        x: u8::MAX,
        y: u8::MAX,
    });
    let mut left = budget;
    {
        let data = layout.data()?;
        let board = g.board(&data);
        wide::step(
            &board,
            tiles,
            start,
            &chests[..count],
            &mut pass,
            &mut open,
            &mut reach,
            &mut state,
            &mut left,
        )
        .map_err(MapError::from)?;
    }
    layout.store_check(state, &pass, &reach)?;
    Ok(state)
}

/// Patch D (docs/MAP_DESCRIPTION.md 10.3): one transaction's share of a big map's reachability
/// check. The website counts the calls a map needs (`wide::steps_needed`) and sends them before
/// `finalize_map`; a call after the check is done changes nothing. Any write to the layout starts
/// the check over.
pub fn handler(ctx: Context<CheckStep>) -> Result<()> {
    let tiles = tiles_of(&ctx.accounts.tileset)?;
    let layout = LayoutBytes::load(&ctx.accounts.layout)?;
    let state = advance(&layout, &tiles, STEP_BUDGET)?;
    emit!(MapChecked {
        map: ctx.accounts.map.key(),
        phase: state.phase,
        row: state.row,
    });
    Ok(())
}
