//! Deadroll city: the city, its buildings, the Tavern board and its quests
//! (docs/CITY_DESCRIPTION.md).
use anchor_lang::prelude::*;

// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod reads;
pub mod state;

use deadroll_types::{Cell, CityObject};
use instructions::*;
use state::BuildingSpec;

declare_id!("2Cr5vB5awqDVx6XRcxtFFYQEG6Zy3aSAcKcqzh1xbVjS");

#[program]
pub mod deadroll_city {
    use super::*;

    /// The game authority creates the one city.
    pub fn init_city(ctx: Context<InitCity>, width: u8, height: u8, spawn: Cell) -> Result<()> {
        init_city::handler(ctx, width, height, spawn)
    }

    /// City authority writes a chunk of city cells (site tile ids).
    pub fn write_city(ctx: Context<WriteCity>, offset: u16, cells: Vec<u8>) -> Result<()> {
        write_city::handler(ctx, offset, cells)
    }

    /// City authority replaces the decor and the spawn cell.
    pub fn set_city_objects(
        ctx: Context<WriteCity>,
        objects: Vec<CityObject>,
        spawn: Cell,
    ) -> Result<()> {
        set_city_objects::handler(ctx, objects, spawn)
    }

    /// City authority adds a building calling one of the game programs.
    pub fn add_building(ctx: Context<AddBuilding>, spec: BuildingSpec) -> Result<()> {
        add_building::handler(ctx, spec)
    }

    /// City authority changes a building.
    pub fn update_building(ctx: Context<UpdateBuilding>, spec: BuildingSpec) -> Result<()> {
        update_building::handler(ctx, spec)
    }

    /// City authority closes a building (boarded-up door).
    pub fn close_building(ctx: Context<CloseBuilding>) -> Result<()> {
        close_building::handler(ctx)
    }

    /// City authority creates the order board of a Tavern.
    pub fn init_tavern_board(ctx: Context<InitTavernBoard>) -> Result<()> {
        init_tavern_board::handler(ctx)
    }

    /// DM posts their party in the lobby as a quest on a Tavern board.
    pub fn post_quest(ctx: Context<PostQuest>) -> Result<()> {
        post_quest::handler(ctx)
    }

    /// DM takes a quest off the board.
    pub fn close_quest(ctx: Context<CloseQuest>) -> Result<()> {
        close_quest::handler(ctx)
    }
}
