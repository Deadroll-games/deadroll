use anchor_lang::{
    prelude::*,
    system_program::{
        allocate, assign, create_account, transfer, Allocate, Assign, CreateAccount, Transfer,
    },
    Discriminator,
};
use deadroll_mapcheck::wide::{rows_from_bytes, rows_to_bytes, CheckState, Row};
use deadroll_types::{
    map_size::hdr, seeds, LayoutGeom, MapPoint, PackLayout, Spawn, LAYOUT_V5, NO_TABLE,
    SECRET_TYPES,
};

use crate::{errors::MapError, state::LAYOUT_VERSION};

/// `["layout", map]`. `finalize_map` hashes the public bytes into `Map.layout_hash`.
/// Patch 13: the core is `MapLayoutDataV2` (cells with variants, points), version 2.
/// Arena update: version 3, the packs and their arenas after the core (`write_arena`).
/// Patch C: version 4, the public part of the secrets (each pack's threat, the declared counts)
/// and the DM's encrypted copy after the arenas (`write_secrets`, `finalize_map`).
/// Patch D: version 5 (`deadroll_types::map_size`): as big as the map's size class and sides
/// need, the cells `width` per row after the class's packs and arenas, then the encrypted copy and
/// the reachability check's scratch (`check_step`). A Dungeon layout is ~37 KB: `create_map`
/// creates the first 10 KB and `grow_layout` the rest, 10 KB per instruction. An older layout can
/// neither be written nor open a party.
///
/// The account is never Borsh-decoded whole: the struct is its header (what the IDL shows); the
/// instructions go through [`LayoutBytes`] and `LayoutGeom`. Contexts take it as
/// `UncheckedAccount` (seeds checked).
#[account]
#[derive(InitSpace)]
pub struct MapLayout {
    pub version: u8,
    pub map: Pubkey,
    pub width: u8,
    pub height: u8,
    /// Patch D: the size class (`deadroll_rules::BalanceConfig.map_size`).
    pub class: u8,
    /// The reward table of the boss (registry `Rewards` id, `NO_TABLE` = none).
    pub boss_table: u8,
    pub bump: u8,
    pub spawn_count: u8,
    pub point_count: u8,
}

const _: () = assert!(8 + MapLayout::INIT_SPACE == hdr::BODY);

/// Most bytes one instruction may add to an account (the runtime's
/// `MAX_PERMITTED_DATA_INCREASE`).
pub const GROW_STEP: usize = 10_240;

/// In-place access to a version 5 `MapLayout` account.
pub struct LayoutBytes<'a, 'info> {
    info: &'a AccountInfo<'info>,
    pub geom: LayoutGeom,
}

impl<'a, 'info> LayoutBytes<'a, 'info> {
    /// A `MapLayout` of this program in the current format, whole or still growing (the seeds
    /// are the context's).
    pub fn load_header(info: &'a AccountInfo<'info>) -> Result<Self> {
        require_keys_eq!(*info.owner, crate::ID, MapError::InvalidLayout);
        let data = info.try_borrow_data()?;
        require!(
            data.len() >= hdr::BODY
                && data[..8] == *MapLayout::DISCRIMINATOR
                && data[hdr::VERSION] == LAYOUT_VERSION,
            MapError::InvalidLayout
        );
        let geom = LayoutGeom::new(data[hdr::WIDTH], data[hdr::HEIGHT], data[hdr::CLASS])
            .ok_or(MapError::InvalidLayout)?;
        drop(data);
        Ok(Self { info, geom })
    }

    /// [`Self::load_header`] of a layout grown to its full size.
    pub fn load(info: &'a AccountInfo<'info>) -> Result<Self> {
        let out = Self::load_header(info)?;
        require!(info.data_len() >= out.geom.end, MapError::LayoutNotGrown);
        Ok(out)
    }

    /// Creates the PDA `["layout", map]` with its first `GROW_STEP` bytes (all of a layout that
    /// small) and writes its header: no cells, spawns, points or packs, no boss table. Works like
    /// Anchor's `init`, including an address pre-funded by someone else.
    pub fn create(
        info: &'a AccountInfo<'info>,
        payer: &AccountInfo<'info>,
        system_program: &AccountInfo<'info>,
        map: &Pubkey,
        bump: u8,
        geom: LayoutGeom,
    ) -> Result<Self> {
        let space = geom.end.min(GROW_STEP);
        let signer: &[&[&[u8]]] = &[&[seeds::LAYOUT, map.as_ref(), &[bump]]];
        let rent = Rent::get()?.minimum_balance(space);
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
                space as u64,
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
                space as u64,
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
            geom.write_header(&mut data, &map.to_bytes(), NO_TABLE, bump);
        }
        Ok(Self { info, geom })
    }

    /// Patch D: adds up to `GROW_STEP` bytes towards the full size, the payer topping up the
    /// rent. Returns the new size.
    pub fn grow(
        &self,
        payer: &AccountInfo<'info>,
        system_program: &AccountInfo<'info>,
    ) -> Result<usize> {
        let len = self.info.data_len();
        require!(len < self.geom.end, MapError::LayoutGrown);
        let new_len = (len + GROW_STEP).min(self.geom.end);
        let missing = Rent::get()?
            .minimum_balance(new_len)
            .saturating_sub(self.info.lamports());
        if missing > 0 {
            transfer(
                CpiContext::new(
                    system_program.key(),
                    Transfer {
                        from: payer.clone(),
                        to: self.info.clone(),
                    },
                ),
                missing,
            )?;
        }
        self.info.resize(new_len)?;
        Ok(new_len)
    }

    /// Every write starts the reachability check over (`check_step`).
    fn reset_check(&self, data: &mut [u8]) {
        let at = self.geom.check_at;
        data[at..at + LayoutGeom::CHECK_LEN].fill(0);
    }

    /// Writes `cells` into the map from cell `start` (row-major, `width` per row).
    pub fn write_cells(&self, start: usize, cells: &[u8]) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        require!(
            self.geom.write_cells(&mut data, start, cells),
            MapError::ChunkOutOfRange
        );
        self.reset_check(&mut data);
        Ok(())
    }

    /// Replaces the spawn list (the slots past it are cleared).
    pub fn write_spawns(&self, spawns: &[Spawn]) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        require!(
            self.geom.write_spawns(&mut data, spawns),
            MapError::TooManySpawns
        );
        self.reset_check(&mut data);
        Ok(())
    }

    /// Replaces the points (the slots past them are cleared) and the boss table.
    pub fn write_points(&self, points: &[MapPoint], boss_table: u8) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        require!(
            self.geom.write_points(&mut data, points, boss_table),
            MapError::TooManyPoints
        );
        self.reset_check(&mut data);
        Ok(())
    }

    /// Arena update: replaces pack `pack`'s record and its arena, `cells` row by row
    /// (`record.width` per row).
    pub fn write_arena(&self, pack: u8, record: &PackLayout, cells: &[u8]) -> Result<()> {
        require!((pack as usize) < self.geom.packs, MapError::PackOutOfRange);
        let mut data = self.info.try_borrow_mut_data()?;
        require!(
            self.geom
                .write_arena(&mut data, pack as usize, record, cells),
            MapError::ArenaCellsMismatch
        );
        self.reset_check(&mut data);
        Ok(())
    }

    /// Patch C: writes `bytes` into the encrypted secrets from `offset` (`len`: the whole copy's
    /// length, the website sends it in chunks) and the public part: each pack's threat (one per
    /// pack number of the class) and the declared counts.
    pub fn write_secrets(
        &self,
        offset: usize,
        bytes: &[u8],
        len: usize,
        threat: &[u8],
        declared: &[u8; SECRET_TYPES],
    ) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        require!(
            self.geom
                .write_secrets(&mut data, offset, bytes, len, threat, declared),
            MapError::SecretsOutOfRange
        );
        self.reset_check(&mut data);
        Ok(())
    }

    /// The account bytes, to read.
    pub fn data(&self) -> Result<std::cell::Ref<'a, &'a mut [u8]>> {
        Ok(self.info.try_borrow_data()?)
    }

    pub fn spawn_count(&self) -> Result<u8> {
        Ok(self.info.try_borrow_data()?[hdr::SPAWN_COUNT])
    }

    pub fn point_count(&self) -> Result<u8> {
        Ok(self.info.try_borrow_data()?[hdr::POINT_COUNT])
    }

    /// Patch D: the reachability check's state.
    pub fn check_state(&self) -> Result<CheckState> {
        let data = self.info.try_borrow_data()?;
        let mut b = [0u8; 8];
        b.copy_from_slice(&data[self.geom.check_at..self.geom.check_at + 8]);
        Ok(CheckState::from_bytes(b))
    }

    /// The check's two bitsets into `pass` and `reach` (`height` rows each).
    pub fn load_rows(&self, pass: &mut [Row], reach: &mut [Row]) -> Result<()> {
        let data = self.info.try_borrow_data()?;
        let (g, n) = (&self.geom, self.geom.bits_len());
        rows_from_bytes(&data[g.pass_at..g.pass_at + n], g.row_words, pass);
        rows_from_bytes(&data[g.reach_at..g.reach_at + n], g.row_words, reach);
        Ok(())
    }

    /// Stores the check's state and bitsets.
    pub fn store_check(&self, state: CheckState, pass: &[Row], reach: &[Row]) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        let (g, n) = (&self.geom, self.geom.bits_len());
        data[g.check_at..g.check_at + 8].copy_from_slice(&state.to_bytes());
        rows_to_bytes(pass, g.row_words, &mut data[g.pass_at..g.pass_at + n]);
        rows_to_bytes(reach, g.row_words, &mut data[g.reach_at..g.reach_at + n]);
        Ok(())
    }
}

const _: () = assert!(LAYOUT_VERSION == LAYOUT_V5);

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
        fn new(geom: LayoutGeom) -> Self {
            let mut data = vec![0u8; geom.end];
            data[..8].copy_from_slice(MapLayout::DISCRIMINATOR);
            geom.write_header(&mut data, &[9; 32], NO_TABLE, 255);
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

    fn medium() -> LayoutGeom {
        LayoutGeom::new(40, 30, 1).unwrap()
    }

    #[test]
    fn writes_in_place_and_resets_the_check() {
        let mut buf = Buf::new(medium());
        let spawn = Spawn {
            bestiary_id: 2,
            x: 3,
            y: 4,
            level: 1,
            skills: 0x1f,
            flags: 0,
            aspects: 0,
            pack: 12,
        };
        let point = MapPoint {
            kind: PointKind::Exit,
            x: 39,
            y: 29,
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
        let g = medium();
        {
            let info = buf.info();
            let l = LayoutBytes::load(&info).unwrap();
            l.store_check(
                CheckState {
                    phase: 3,
                    row: 0,
                    changed: false,
                },
                &[[1, 0, 0]; 30],
                &[[1, 0, 0]; 30],
            )
            .unwrap();
            assert!(l.check_state().unwrap().done());
            l.write_cells(40 * 29, &[8; 40]).unwrap();
            assert_eq!(l.check_state().unwrap(), CheckState::default());
            l.write_spawns(&[spawn, spawn]).unwrap();
            l.write_spawns(&[spawn]).unwrap();
            l.write_points(&[point], 1).unwrap();
            l.write_arena(15, &record, &[1, 2, 3, 4, 5, 6]).unwrap();
            let mut threat = [0u8; 16];
            threat[15] = 3;
            l.write_secrets(0, &[1, 2], 2, &threat, &[1, 0, 0, 0])
                .unwrap();
            assert_eq!(
                code(l.write_cells(40 * 30 - 1, &[1, 2]).unwrap_err()),
                u32::from(MapError::ChunkOutOfRange)
            );
            assert_eq!(
                code(l.write_spawns(&[spawn; 17]).unwrap_err()),
                u32::from(MapError::TooManySpawns)
            );
            assert_eq!(
                code(l.write_points(&[point; 11], 0).unwrap_err()),
                u32::from(MapError::TooManyPoints)
            );
            assert_eq!(
                code(l.write_arena(16, &record, &[0; 6]).unwrap_err()),
                u32::from(MapError::PackOutOfRange)
            );
            assert_eq!(
                code(l.write_arena(3, &record, &[0; 5]).unwrap_err()),
                u32::from(MapError::ArenaCellsMismatch)
            );
            assert_eq!(
                code(l.write_secrets(0, &[], 1601, &threat, &[0; 4]).unwrap_err()),
                u32::from(MapError::SecretsOutOfRange)
            );
            assert_eq!(
                code(
                    l.write_secrets(0, &[], 2, &threat[..8], &[0; 4])
                        .unwrap_err()
                ),
                u32::from(MapError::SecretsOutOfRange)
            );
        }
        let header = MapLayout::try_deserialize(&mut &buf.data[..]).unwrap();
        assert_eq!(
            (header.width, header.height, header.class, header.boss_table),
            (40, 30, 1, 1)
        );
        assert_eq!((header.spawn_count, header.point_count), (1, 1));
        let core = g.core(&buf.data);
        assert_eq!(core.spawn_list(), &[spawn]);
        assert_eq!(core.exit(), Some(deadroll_types::Cell { x: 39, y: 29 }));
        assert_eq!(g.pack(&buf.data, 15), Some(record));
        assert_eq!(g.board(&buf.data).row(29), &[8; 40]);
        assert_eq!(g.secrets(&buf.data).threat[15], 3);
    }

    #[test]
    fn the_check_survives_its_bytes() {
        let mut buf = Buf::new(LayoutGeom::new(143, 20, 3).unwrap());
        let info = buf.info();
        let l = LayoutBytes::load(&info).unwrap();
        let pass: Vec<Row> = (0..20u64).map(|i| [i, i << 3, 7]).collect();
        let reach: Vec<Row> = (0..20u64).map(|i| [!i, 0, i]).collect();
        let state = CheckState {
            phase: 2,
            row: 17,
            changed: true,
        };
        l.store_check(state, &pass, &reach).unwrap();
        let (mut p, mut r) = (vec![[0u64; 3]; 20], vec![[0u64; 3]; 20]);
        l.load_rows(&mut p, &mut r).unwrap();
        assert_eq!((p, r), (pass, reach));
        assert_eq!(l.check_state().unwrap(), state);
    }

    #[test]
    fn refuses_another_owner_version_or_a_short_account() {
        let mut other = Buf::new(medium());
        other.owner = Pubkey::new_unique();
        assert!(LayoutBytes::load(&other.info()).is_err());
        let mut old = Buf::new(medium());
        old.data[hdr::VERSION] = 4;
        assert!(LayoutBytes::load(&old.info()).is_err());
        let mut wrong = Buf::new(medium());
        wrong.data[0] ^= 1;
        assert!(LayoutBytes::load(&wrong.info()).is_err());
        let mut short = Buf::new(medium());
        short.data.truncate(medium().end - 1);
        assert_eq!(
            code(LayoutBytes::load(&short.info()).err().unwrap()),
            u32::from(MapError::LayoutNotGrown)
        );
        assert!(LayoutBytes::load_header(&short.info()).is_ok());
    }
}
