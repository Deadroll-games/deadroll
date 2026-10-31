//! Patch C: the DM's reveals (docs/MAP_DESCRIPTION.md 8.2-8.5, 17.2-17.5; PARTY_AND_TURNS.md 3.2).
//!
//! The map commits to every cell of the map and of each arena, and to each pack's composition,
//! under `Map.secrets_root` (`deadroll_mapcheck::secrets`). A player's step onto a cell, or a look
//! at the cell next to them (`examine`), obliges the DM to reveal it; until every obligation is
//! met no other action goes through (`GameSession::require_no_reveals`). The DM's client sends the
//! reveals with the DM's session key (`reveal_cell`); a pack's composition is revealed when the
//! party engages it (`reveal_pack`). After the party ends the DM reveals every declared secret and
//! every pack left (the audit), or the escrow is not paid (`settle_party`).
use anchor_lang::prelude::*;
use deadroll_mapcheck::{
    design::occupied_cell,
    secrets::{cell_leaf, pack_leaf, verify, Board, SecretPlace},
    tiles::TileRules,
    MapCheckError,
};
use deadroll_types::{
    Cell, CellSecret, PackRoster, SessionStatus, SECRET_DEPTH_MAX, SECRET_SALT_LEN,
};

use crate::{
    errors::SessionError,
    events::RevealsOwed,
    state::{external::MapLayoutView, GameSession},
};

/// The DM reveals during the party (`false`) or in the final audit after it ended (`true`).
/// Before the start, or once the escrow is settled, there is nothing to reveal.
pub fn audit_mode(session: &GameSession) -> Result<bool> {
    match session.core.status {
        SessionStatus::Active => Ok(false),
        SessionStatus::Victory | SessionStatus::Wiped | SessionStatus::Fled => Ok(true),
        _ => err!(SessionError::NotActive),
    }
}

/// Every cell of `cells` (on the ground the units stand on) not revealed yet becomes an
/// obligation of the DM, uncovered by player `slot` (MAP 8.4: a character stepped on the cell, or
/// examined it).
pub fn oblige_cells(
    session: &mut GameSession,
    session_key: Pubkey,
    slot: u8,
    cells: &[Cell],
) -> Result<()> {
    let before = session.obligation_count;
    for &c in cells {
        let leaf = session.leaf_here(c).ok_or(SessionError::OutOfBounds)?;
        require!(session.oblige(leaf), SessionError::TooManyObligations);
    }
    if session.obligation_count != before {
        session.obliged_by = slot;
        emit!(RevealsOwed {
            session: session_key,
            slot,
            count: session.obligation_count,
        });
    }
    Ok(())
}

/// `secret` with `salt` and `proof` is cell leaf `leaf` of the map's tree.
pub fn verify_cell(
    session: &GameSession,
    leaf: u16,
    secret: &CellSecret,
    salt: &[u8; SECRET_SALT_LEN],
    proof: &[[u8; 32]],
) -> Result<()> {
    require!(proof.len() <= SECRET_DEPTH_MAX, SessionError::BadProof);
    let map = session.core.map.to_bytes();
    let hash = cell_leaf(&map, leaf, secret, salt);
    require!(
        verify(&session.secrets_root, hash, leaf, session.leaf_count, proof),
        SessionError::BadProof
    );
    Ok(())
}

/// `roster` with `salt` and `proof` is pack `pack`'s leaf of the map's tree.
pub fn verify_pack(
    session: &GameSession,
    pack: u8,
    roster: &PackRoster,
    salt: &[u8; SECRET_SALT_LEN],
    proof: &[[u8; 32]],
) -> Result<()> {
    require!(proof.len() <= SECRET_DEPTH_MAX, SessionError::BadProof);
    let map = session.core.map.to_bytes();
    let leaf = pack as u16;
    let hash = pack_leaf(&map, leaf, roster, salt);
    require!(
        verify(&session.secrets_root, hash, leaf, session.leaf_count, proof),
        SessionError::BadProof
    );
    Ok(())
}

/// The session's copy of the `Tileset`: every type it lacks has cost 0.
pub fn tiles(session: &GameSession) -> TileRules {
    TileRules {
        cost: session.tile_cost,
        known: u32::MAX,
        blocks_vision: 0,
    }
}

/// Where leaf `board` lies, for `deadroll_mapcheck::secrets::check_secret`: its cell byte and,
/// on the map, whether a point, a chest of the map or a pack's monster takes the cell.
pub fn place_of(
    session: &GameSession,
    layout: &MapLayoutView,
    board: Board,
) -> Result<SecretPlace> {
    Ok(match board {
        Board::Map(c) => {
            let chests = &session.chest_list()
                [..(session.visible_chests as usize).min(session.chest_list().len())];
            SecretPlace {
                board,
                byte: layout.cell_of(None, c.x, c.y)?,
                occupied: occupied_cell(
                    c,
                    [session.entrance, layout.exit()?],
                    chests.iter().map(|p| p.cell()),
                    &session.packs,
                ),
            }
        }
        Board::Arena(n, c) => SecretPlace {
            board,
            byte: layout.cell_of(Some(n), c.x, c.y)?,
            occupied: false,
        },
        Board::Pack(_) => return err!(SessionError::SecretRejected),
    })
}

/// A map rule the revealed contents broke: the program error, the code in the log.
pub fn rejected(e: MapCheckError) -> Error {
    msg!("map rule {}", e.code());
    match e {
        MapCheckError::ThreatMismatch => error!(SessionError::ThreatMismatch),
        MapCheckError::TooManySecrets => error!(SessionError::TooManySecrets),
        _ => error!(SessionError::SecretRejected),
    }
}
