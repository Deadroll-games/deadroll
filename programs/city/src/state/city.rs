use anchor_lang::prelude::*;
use deadroll_types::{Cell, CityData, CITY_W_MAX};

/// `["city"]`: the one city (docs/CITY_DESCRIPTION.md 3.1).
#[account]
#[derive(InitSpace)]
pub struct City {
    pub core: CityData,
}

impl City {
    /// `cell` is on the city map.
    pub fn contains(core: &CityData, cell: Cell) -> bool {
        cell.x < core.width && cell.y < core.height
    }

    /// Index of `cell` in `cells` (`CITY_W_MAX` per row).
    pub const fn index(cell: Cell) -> usize {
        cell.y as usize * CITY_W_MAX + cell.x as usize
    }
}
