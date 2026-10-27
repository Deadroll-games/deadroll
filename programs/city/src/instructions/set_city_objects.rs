use anchor_lang::prelude::*;
use deadroll_types::{Cell, CityObject, CITY_OBJECTS_MAX};

use crate::{errors::CityError, events::CityObjectsSet, instructions::WriteCity, state::City};

/// Replaces the decor and the spawn cell.
pub fn handler(ctx: Context<WriteCity>, objects: Vec<CityObject>, spawn: Cell) -> Result<()> {
    require!(objects.len() <= CITY_OBJECTS_MAX, CityError::TooManyObjects);
    let core = &mut ctx.accounts.city.core;
    let on_map = |cell: Cell| City::contains(core, cell);
    require!(
        on_map(spawn) && objects.iter().all(|o| on_map(Cell { x: o.x, y: o.y })),
        CityError::OutOfCity
    );
    core.objects = [CityObject::default(); CITY_OBJECTS_MAX];
    core.objects[..objects.len()].copy_from_slice(&objects);
    core.object_count = objects.len() as u8;
    core.spawn = spawn;
    emit!(CityObjectsSet {
        object_count: core.object_count,
        spawn_x: spawn.x,
        spawn_y: spawn.y,
    });
    Ok(())
}
