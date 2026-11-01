use anchor_lang::{
    prelude::*,
    system_program::{
        allocate, assign, create_account, transfer, Allocate, Assign, CreateAccount, Transfer,
    },
    Discriminator,
};
use deadroll_types::{
    seeds, MapArenasData, MapLayoutDataV2, MapPoint, PackLayout, Spawn, ARENA_CELLS,
    ARENA_SIDE_MAX, MAP_CELLS, NO_TABLE, PACKS_MAX, POINTS_MAX, SPAWNS_MAX,
};

use crate::{errors::MapError, state::LAYOUT_VERSION};

/// `["layout", map]`. `finalize_map` hashes the account bytes after the discriminator into
/// `Map.layout_hash`.
/// Patch 13: the core is `MapLayoutDataV2` (cells with variants, points), version 2.
/// Arena update: version 3, the packs and their arenas after the core (`write_arena`). A layout
/// from before it is smaller: it can neither be written nor open a party.
///
/// The account (~2.5 KB) is never Borsh-decoded whole on-chain: `Account<MapLayout>` would put two
/// copies of it on the 4 KB SBF stack frame. The instructions go through [`LayoutBytes`] (in place,
/// like the session's `MapLayoutView`); `finalize_map` decodes the core and the arenas into boxes
/// one at a time. Contexts take it as `UncheckedAccount` (seeds checked), `finalize_map` as
/// `LazyAccount` (owner, discriminator, IDL entry).
#[account]
#[derive(InitSpace)]
pub struct MapLayout {
    pub core: MapLayoutDataV2,
    pub arenas: MapArenasData,
}

/// Account size, discriminator included.
pub const LAYOUT_SPACE: usize = 8 + MapLayout::INIT_SPACE;
// `create_map` creates it in one CPI.
const _: () = assert!(LAYOUT_SPACE <= 10_240);

/// Byte offsets inside a `MapLayout` account (discriminator included).
pub mod off {
    use super::*;
    pub const VERSION: usize = 8;
    pub const MAP: usize = VERSION + 1;
    pub const CELLS: usize = MAP + 32;
    pub const SPAWNS: usize = CELLS + MAP_CELLS;
    pub const SPAWN_COUNT: usize = SPAWNS + SPAWNS_MAX * Spawn::SIZE;
    pub const POINTS: usize = SPAWN_COUNT + 1;
    pub const POINT_COUNT: usize = POINTS + POINTS_MAX * MapPoint::SIZE;
    pub const BOSS_TABLE: usize = POINT_COUNT + 1;
    pub const BUMP: usize = BOSS_TABLE + 1;
    pub const PACKS: usize = BUMP + 1;
    pub const ARENAS: usize = PACKS + PACKS_MAX * PackLayout::SIZE;
    pub const END: usize = ARENAS + PACKS_MAX * ARENA_CELLS;
}

const _: () = assert!(off::END == LAYOUT_SPACE);

/// In-place access to a `MapLayout` account.
pub struct LayoutBytes<'a, 'info> {
    info: &'a AccountInfo<'info>,
}

impl<'a, 'info> LayoutBytes<'a, 'info> {
    /// A `MapLayout` of this program in the current format (the seeds are the context's).
    pub fn load(info: &'a AccountInfo<'info>) -> Result<Self> {
        require_keys_eq!(*info.owner, crate::ID, MapError::InvalidLayout);
        let data = info.try_borrow_data()?;
        require!(
            data.len() >= LAYOUT_SPACE
                && data[..8] == *MapLayout::DISCRIMINATOR
                && data[off::VERSION] == LAYOUT_VERSION,
            MapError::InvalidLayout
        );
        drop(data);
        Ok(Self { info })
    }

    /// Creates the PDA `["layout", map]` and writes its header: no cells, spawns, points or packs,
    /// no boss table. Works like Anchor's `init`, including an address pre-funded by someone else.
    pub fn create(
        info: &'a AccountInfo<'info>,
        payer: &AccountInfo<'info>,
        system_program: &AccountInfo<'info>,
        map: &Pubkey,
        bump: u8,
    ) -> Result<Self> {
        let signer: &[&[&[u8]]] = &[&[seeds::LAYOUT, map.as_ref(), &[bump]]];
        let rent = Rent::get()?.minimum_balance(LAYOUT_SPACE);
        let program = system_program.key();
        if info.lamports() == 0 {
            create_account(
                CpiContext::new_with_signer(
                    program,
                    CreateAccount {
                        from: payer.clone(),
                        to: info.clone(),
                    },
                    signer,
                ),
                rent,
                LAYOUT_SPACE as u64,
                &crate::ID,
            )?;
        } else {
            let missing = rent.saturating_sub(info.lamports());
            if missing > 0 {
                transfer(
                    CpiContext::new(
                        program,
                        Transfer {
                            from: payer.clone(),
                            to: info.clone(),
                        },
                    ),
                    missing,
                )?;
            }
            allocate(
                CpiContext::new_with_signer(
                    program,
                    Allocate {
                        account_to_allocate: info.clone(),
                    },
                    signer,
                ),
                LAYOUT_SPACE as u64,
            )?;
            assign(
                CpiContext::new_with_signer(
                    program,
                    Assign {
                        account_to_assign: info.clone(),
                    },
                    signer,
                ),
                &crate::ID,
            )?;
        }
        {
            let mut data = info.try_borrow_mut_data()?;
            data[..8].copy_from_slice(MapLayout::DISCRIMINATOR);
            data[off::VERSION] = LAYOUT_VERSION;
            data[off::MAP..off::MAP + 32].copy_from_slice(map.as_ref());
            data[off::BOSS_TABLE] = NO_TABLE;
            data[off::BUMP] = bump;
        }
        Ok(Self { info })
    }

    /// Writes `cells` into the grid from cell `start` (row-major, `MAP_MAX` per row).
    pub fn write_cells(&self, start: usize, cells: &[u8]) -> Result<()> {
        let end = start
            .checked_add(cells.len())
            .ok_or(MapError::ChunkOutOfRange)?;
        require!(end <= MAP_CELLS, MapError::ChunkOutOfRange);
        self.info.try_borrow_mut_data()?[off::CELLS + start..off::CELLS + end]
            .copy_from_slice(cells);
        Ok(())
    }

    /// Replaces the spawn list (the slots past it are cleared).
    pub fn write_spawns(&self, spawns: &[Spawn]) -> Result<()> {
        require!(spawns.len() <= SPAWNS_MAX, MapError::TooManySpawns);
        let mut data = self.info.try_borrow_mut_data()?;
        data[off::SPAWNS..off::SPAWN_COUNT].fill(0);
        for (i, s) in spawns.iter().enumerate() {
            let at = off::SPAWNS + i * Spawn::SIZE;
            s.serialize(&mut &mut data[at..at + Spawn::SIZE])
                .map_err(|_| error!(MapError::InvalidLayout))?;
        }
        data[off::SPAWN_COUNT] = spawns.len() as u8;
        Ok(())
    }

    /// Replaces the points (the slots past them are cleared) and the boss table.
    pub fn write_points(&self, points: &[MapPoint], boss_table: u8) -> Result<()> {
        require!(points.len() <= POINTS_MAX, MapError::TooManyPoints);
        let mut data = self.info.try_borrow_mut_data()?;
        data[off::POINTS..off::POINT_COUNT].fill(0);
        for (i, p) in points.iter().enumerate() {
            let at = off::POINTS + i * MapPoint::SIZE;
            p.serialize(&mut &mut data[at..at + MapPoint::SIZE])
                .map_err(|_| error!(MapError::InvalidLayout))?;
        }
        data[off::POINT_COUNT] = points.len() as u8;
        data[off::BOSS_TABLE] = boss_table;
        Ok(())
    }

    /// Arena update: replaces pack `pack`'s record and its arena, `cells` row by row
    /// (`record.width` per row; `ARENA_SIDE_MAX` per stored row, the rest cleared).
    pub fn write_arena(&self, pack: u8, record: &PackLayout, cells: &[u8]) -> Result<()> {
        require!((pack as usize) < PACKS_MAX, MapError::PackOutOfRange);
        let (w, h) = (record.width as usize, record.height as usize);
        require!(
            w <= ARENA_SIDE_MAX && h <= ARENA_SIDE_MAX && cells.len() == w * h,
            MapError::ArenaCellsMismatch
        );
        let mut data = self.info.try_borrow_mut_data()?;
        let at = off::PACKS + pack as usize * PackLayout::SIZE;
        record
            .serialize(&mut &mut data[at..at + PackLayout::SIZE])
            .map_err(|_| error!(MapError::InvalidLayout))?;
        let arena = off::ARENAS + pack as usize * ARENA_CELLS;
        data[arena..arena + ARENA_CELLS].fill(0);
        for (y, row) in cells.chunks(w.max(1)).take(h).enumerate() {
            let at = arena + y * ARENA_SIDE_MAX;
            data[at..at + row.len()].copy_from_slice(row);
        }
        Ok(())
    }

    pub fn spawn_count(&self) -> Result<u8> {
        Ok(self.info.try_borrow_data()?[off::SPAWN_COUNT])
    }

    pub fn point_count(&self) -> Result<u8> {
        Ok(self.info.try_borrow_data()?[off::POINT_COUNT])
    }
}

/// The layout core, decoded into a box (`finalize_map`). Not inlined: its frame holds one copy.
#[inline(never)]
pub fn decode_core(data: &[u8]) -> Result<Box<MapLayoutDataV2>> {
    let mut body = data
        .get(off::VERSION..off::PACKS)
        .ok_or(error!(MapError::InvalidLayout))?;
    Ok(Box::new(
        MapLayoutDataV2::deserialize(&mut body).map_err(|_| error!(MapError::InvalidLayout))?,
    ))
}

/// Arena update: the packs and arenas, decoded into a box (`finalize_map`). Not inlined.
#[inline(never)]
pub fn decode_arenas(data: &[u8]) -> Result<Box<MapArenasData>> {
    let mut body = data
        .get(off::PACKS..off::END)
        .ok_or(error!(MapError::InvalidLayout))?;
    Ok(Box::new(
        MapArenasData::deserialize(&mut body).map_err(|_| error!(MapError::InvalidLayout))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::AccountDeserialize;
    use deadroll_types::{Facing, PointKind};

    fn code(e: Error) -> u32 {
        match e {
            Error::AnchorError(e) => e.error_code_number,
            other => panic!("unexpected {other:?}"),
        }
    }

    struct Buf {
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
    }

    impl Buf {
        fn new() -> Self {
            let mut data = vec![0u8; LAYOUT_SPACE];
            data[..8].copy_from_slice(MapLayout::DISCRIMINATOR);
            data[off::VERSION] = LAYOUT_VERSION;
            data[off::BOSS_TABLE] = NO_TABLE;
            Self {
                key: Pubkey::new_unique(),
                owner: crate::ID,
                lamports: 1,
                data,
            }
        }

        fn info(&mut self) -> AccountInfo<'_> {
            AccountInfo::new(
                &self.key,
                false,
                true,
                &mut self.lamports,
                &mut self.data,
                &self.owner,
                false,
            )
        }
    }

    #[test]
    fn writes_in_place_like_borsh() {
        let mut buf = Buf::new();
        let spawn = Spawn {
            bestiary_id: 2,
            x: 3,
            y: 4,
            level: 1,
            skills: 0x1f,
            flags: 0,
            aspects: 0,
            pack: 3,
        };
        let point = MapPoint {
            kind: PointKind::Exit,
            x: 9,
            y: 9,
            data: 0,
        };
        let record = PackLayout {
            visible: 0,
            x: 6,
            y: 7,
            facing: Facing::SW,
            width: 3,
            height: 2,
        };
        {
            let info = buf.info();
            let l = LayoutBytes::load(&info).unwrap();
            l.write_cells(40, &[8, 9]).unwrap();
            l.write_spawns(&[spawn, spawn]).unwrap();
            l.write_spawns(&[spawn]).unwrap();
            l.write_points(&[point], 1).unwrap();
            l.write_arena(3, &record, &[1, 2, 3, 4, 5, 6]).unwrap();
            assert_eq!(
                code(l.write_cells(MAP_CELLS - 1, &[1, 2]).unwrap_err()),
                u32::from(MapError::ChunkOutOfRange)
            );
            let many = [spawn; SPAWNS_MAX + 1];
            assert_eq!(
                code(l.write_spawns(&many).unwrap_err()),
                u32::from(MapError::TooManySpawns)
            );
            let points = [point; POINTS_MAX + 1];
            assert_eq!(
                code(l.write_points(&points, 0).unwrap_err()),
                u32::from(MapError::TooManyPoints)
            );
            assert_eq!(
                code(
                    l.write_arena(PACKS_MAX as u8, &record, &[0; 6])
                        .unwrap_err()
                ),
                u32::from(MapError::PackOutOfRange)
            );
            assert_eq!(
                code(l.write_arena(3, &record, &[0; 5]).unwrap_err()),
                u32::from(MapError::ArenaCellsMismatch)
            );
            let wide = PackLayout {
                width: ARENA_SIDE_MAX as u8 + 1,
                height: 1,
                ..record
            };
            assert_eq!(
                code(
                    l.write_arena(3, &wide, &[0; ARENA_SIDE_MAX + 1])
                        .unwrap_err()
                ),
                u32::from(MapError::ArenaCellsMismatch)
            );
        }
        let back = MapLayout::try_deserialize(&mut &buf.data[..]).unwrap();
        assert_eq!(&back.core.cells[40..42], &[8, 9]);
        assert_eq!(back.core.spawn_count, 1);
        assert_eq!(back.core.spawns[0], spawn);
        assert_eq!(back.core.spawns[1], Spawn::default(), "cleared");
        assert_eq!(
            (
                back.core.point_count,
                back.core.points[0],
                back.core.boss_table
            ),
            (1, point, 1)
        );
        assert_eq!(back.arenas.packs[3], record);
        assert_eq!(back.arenas.cell(3, 2, 1), Some(6));
        assert_eq!(back.arenas.cell(3, 3, 0), Some(0));
        // The boxed decoders read the same.
        assert_eq!(*decode_core(&buf.data).unwrap(), back.core);
        assert_eq!(*decode_arenas(&buf.data).unwrap(), back.arenas);
    }

    #[test]
    fn refuses_another_owner_discriminator_or_version() {
        let mut other = Buf::new();
        other.owner = Pubkey::new_unique();
        assert!(LayoutBytes::load(&other.info()).is_err());
        let mut old = Buf::new();
        old.data[off::VERSION] = 2;
        assert!(LayoutBytes::load(&old.info()).is_err());
        let mut wrong = Buf::new();
        wrong.data[0] ^= 1;
        assert!(LayoutBytes::load(&wrong.info()).is_err());
        let mut short = Buf::new();
        short.data.truncate(LAYOUT_SPACE - 1);
        assert!(LayoutBytes::load(&short.info()).is_err());
    }
}
