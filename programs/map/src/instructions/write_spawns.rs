use anchor_lang::prelude::*;
use deadroll_types::{Spawn, SPAWNS_MAX};

use crate::{errors::MapError, events::SpawnsWritten, instructions::WriteLayout};

/// Replaces the whole spawn list. Rules are checked at `finalize_map`, so the builder can save
/// a half-done map. Patch 13: the entrance and the exit are points (`write_points`).
pub fn handler(ctx: Context<WriteLayout>, spawns: Vec<Spawn>) -> Result<()> {
    require!(spawns.len() <= SPAWNS_MAX, MapError::TooManySpawns);
    let layout = &mut ctx.accounts.layout.core;
    layout.spawns = [Spawn::default(); SPAWNS_MAX];
    layout.spawns[..spawns.len()].copy_from_slice(&spawns);
    layout.spawn_count = spawns.len() as u8;

    emit!(SpawnsWritten {
        map: ctx.accounts.map.key(),
        spawn_count: layout.spawn_count,
    });
    Ok(())
}
