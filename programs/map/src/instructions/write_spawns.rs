use anchor_lang::prelude::*;
use deadroll_types::Spawn;

use crate::{events::SpawnsWritten, instructions::WriteLayout, state::LayoutBytes};

/// Replaces the whole spawn list. Rules are checked at `finalize_map`, so the builder can save
/// a half-done map. Patch 13: the entrance and the exit are points (`write_points`).
pub fn handler(ctx: Context<WriteLayout>, spawns: Vec<Spawn>) -> Result<()> {
    let layout = LayoutBytes::load(&ctx.accounts.layout)?;
    layout.write_spawns(&spawns)?;
    emit!(SpawnsWritten {
        map: ctx.accounts.map.key(),
        spawn_count: layout.spawn_count()?,
    });
    Ok(())
}
