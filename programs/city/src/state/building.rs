use anchor_lang::prelude::*;
use deadroll_types::{
    seeds, BuildingData, BuildingKind, BuildingStatus, Cell, CityData, Requirement,
};

use super::City;
use crate::errors::CityError;

/// `["building", id: u16]`: one building of the city (docs/CITY_DESCRIPTION.md 3.2).
#[account]
#[derive(InitSpace)]
pub struct Building {
    pub core: BuildingData,
}

/// What `add_building` and `update_building` set: every field but the id.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildingSpec {
    pub kind: BuildingKind,
    pub prefab_id: u16,
    pub x: u8,
    pub y: u8,
    pub door: Cell,
    pub program: Pubkey,
    pub target: Pubkey,
    pub interior_id: u16,
    pub status: BuildingStatus,
    pub requirement: Requirement,
}

impl BuildingSpec {
    /// The rules of a building (docs/CITY_DESCRIPTION.md 4): on the map; its program is one of
    /// the game programs (`GameConfig.programs`), or none for a closed building; an open Tavern
    /// is a board of this program.
    pub fn check(&self, city: &CityData, programs: &[Pubkey], building: &Pubkey) -> Result<()> {
        let origin = Cell {
            x: self.x,
            y: self.y,
        };
        require!(
            City::contains(city, origin) && City::contains(city, self.door),
            CityError::OutOfCity
        );
        if self.program == Pubkey::default() {
            require!(
                self.status == BuildingStatus::Closed,
                CityError::ProgramNotAllowed
            );
        } else {
            require!(
                programs.contains(&self.program),
                CityError::ProgramNotAllowed
            );
        }
        if self.kind == BuildingKind::Tavern && self.status == BuildingStatus::Active {
            require_keys_eq!(self.program, crate::ID, CityError::BadTarget);
            require_keys_eq!(self.target, board_address(building), CityError::BadTarget);
        }
        Ok(())
    }

    pub fn data(&self, id: u16, version: u8, bump: u8) -> BuildingData {
        BuildingData {
            version,
            id,
            kind: self.kind,
            prefab_id: self.prefab_id,
            x: self.x,
            y: self.y,
            door: self.door,
            program: self.program,
            target: self.target,
            interior_id: self.interior_id,
            status: self.status,
            requirement: self.requirement,
            bump,
        }
    }
}

/// The `TavernBoard` PDA of `building`.
pub fn board_address(building: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[seeds::TAVERN_BOARD, building.as_ref()], &crate::ID).0
}

/// The access condition of a building (docs/CITY_DESCRIPTION.md 5.3), checked by every
/// building instruction with a condition.
pub fn check_requirement(requirement: Requirement) -> Result<()> {
    match requirement {
        Requirement::None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use deadroll_types::{CityObject, CITY_CELLS, CITY_OBJECTS_MAX};

    use super::*;

    fn city() -> CityData {
        CityData {
            version: 1,
            authority: Pubkey::new_unique(),
            width: 20,
            height: 12,
            cells: [0; CITY_CELLS],
            objects: [CityObject::default(); CITY_OBJECTS_MAX],
            object_count: 0,
            spawn: Cell { x: 1, y: 1 },
            building_count: 0,
            bump: 255,
        }
    }

    fn tavern(building: &Pubkey) -> BuildingSpec {
        BuildingSpec {
            kind: BuildingKind::Tavern,
            prefab_id: 1,
            x: 2,
            y: 3,
            door: Cell { x: 4, y: 6 },
            program: crate::ID,
            target: board_address(building),
            interior_id: 0,
            status: BuildingStatus::Active,
            requirement: Requirement::None,
        }
    }

    #[test]
    fn open_tavern_is_a_board_of_this_program() {
        let key = Pubkey::new_unique();
        let programs = [Pubkey::new_unique(), crate::ID];
        assert_eq!(tavern(&key).check(&city(), &programs, &key), Ok(()));
        let mut other_target = tavern(&key);
        other_target.target = Pubkey::new_unique();
        assert_eq!(
            other_target.check(&city(), &programs, &key),
            Err(CityError::BadTarget.into())
        );
        let mut other_program = tavern(&key);
        other_program.program = programs[0];
        assert_eq!(
            other_program.check(&city(), &programs, &key),
            Err(CityError::BadTarget.into())
        );
    }

    #[test]
    fn program_must_be_a_game_program() {
        let key = Pubkey::new_unique();
        let not_allowed = Err(CityError::ProgramNotAllowed.into());
        // City id missing from the allowlist.
        assert_eq!(tavern(&key).check(&city(), &[], &key), not_allowed);
        // No program: only a closed building.
        let mut none = tavern(&key);
        none.program = Pubkey::default();
        assert_eq!(none.check(&city(), &[crate::ID], &key), not_allowed);
        none.status = BuildingStatus::Closed;
        none.kind = BuildingKind::Storage;
        assert_eq!(none.check(&city(), &[crate::ID], &key), Ok(()));
    }

    #[test]
    fn building_stays_on_the_map() {
        let key = Pubkey::new_unique();
        let mut b = tavern(&key);
        b.x = 20;
        assert_eq!(
            b.check(&city(), &[crate::ID], &key),
            Err(CityError::OutOfCity.into())
        );
        let mut b = tavern(&key);
        b.door = Cell { x: 0, y: 12 };
        assert_eq!(
            b.check(&city(), &[crate::ID], &key),
            Err(CityError::OutOfCity.into())
        );
    }
}
