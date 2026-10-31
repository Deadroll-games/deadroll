use anchor_lang::prelude::*;
use deadroll_mapcheck::{
    limits::tier_limits,
    loot::{check_stash_budget, MapLoot},
    secrets::{ambush_mask, check_secret, Board},
};
use deadroll_registry::state::{view::table_body, TableKind};
use deadroll_rules::{loot::map_rules, map_tables};
use deadroll_types::{
    seeds, CellSecret, FoundSecret, MapPoint, PointKind, SecretKind, CHESTS_MAX, NO_TABLE,
    SECRETS_MAX, SECRET_SALT_LEN,
};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    errors::SessionError,
    events::{AmbushSprung, CellRevealed, TrapTriggered},
    outcomes::player_down,
    rules::{BALANCE, EMPTY_UNIT},
    secrets::{audit_mode, place_of, rejected, tiles, verify_cell},
    state::{
        external::{MapLayoutView, REWARDS_TABLE},
        ArenaMode, GameSession, Monsters, Participation,
    },
};

#[derive(Accounts, Session)]
pub struct RevealCell<'info> {
    /// The DM's main wallet or the DM's session key bound to this party.
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: the session's `MapLayout` (address pinned at `open_session`): the cell bytes, the exit.
    #[account(address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
    /// The player whose step uncovered the cell (`GameSession.obliged_by`): a trap hits them.
    /// Needed only for a trap during the party.
    #[account(
        mut,
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            participation.core.character.as_ref(),
        ],
        bump = participation.core.bump,
    )]
    pub participation: Option<Box<Account<'info, Participation>>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place): a
    /// hidden stash's reward table is copied there.
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
    /// CHECK: registry `["rewards"]` table, read in place: a hidden stash's table and the budget.
    #[account(address = REWARDS_TABLE @ SessionError::InvalidRegistryTable)]
    pub rewards: UncheckedAccount<'info>,
    #[session(signer = signer, authority = session.core.dm)]
    pub session_token: Option<Account<'info, SessionTokenV2>>,
}

/// Patch C (docs/MAP_DESCRIPTION.md 8.2-8.5, 17.4): the DM reveals what leaf `leaf` hides,
/// `secret` with its `salt`, and the Merkle `proof` to the map's `secrets_root`. During the party
/// only a leaf on the obligations (a step or a look uncovered it); after it, in the final audit,
/// any leaf not revealed yet, without effects (docs/PARTY_AND_TURNS.md 3.2). The contents must
/// follow the map rules (`deadroll_mapcheck::secrets::check_secret`) and stay within the declared
/// counts and, for a hidden stash, the reward budget. Then, during the party: a trap takes its
/// damage off the HP of the player who stepped on it (no dice, no resistances); an ambush takes
/// the party to its hidden pack, the monsters first; a hidden stash becomes a chest; a secret door
/// becomes a door.
#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.session.core.dm,
    SessionError::Unauthorized
)]
pub fn handler(
    ctx: Context<RevealCell>,
    leaf: u16,
    secret: CellSecret,
    salt: [u8; SECRET_SALT_LEN],
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let session_key = session.key();
    auth::check_bound(accounts.signer.key, &session.core.dm, &session.dm_actor)?;
    let audit = audit_mode(session)?;
    if !audit {
        require!(!session.core.pending_vrf, SessionError::RollPending);
        require!(
            session.obligation_list().contains(&leaf),
            SessionError::NotObligated
        );
    }
    require!(!session.is_revealed(leaf), SessionError::AlreadyRevealed);
    verify_cell(session, leaf, &secret, &salt, &proof)?;

    let board = session
        .leaves()
        .board(leaf)
        .ok_or(SessionError::SecretRejected)?;
    let layout = MapLayoutView::load(&accounts.layout, None)?;
    let place = place_of(session, &layout, board)?;
    let limits = BALANCE
        .secrets
        .limits(session.tier)
        .ok_or(SessionError::SecretRejected)?;
    let level_cap = tier_limits(session.tier)
        .ok_or(SessionError::SecretRejected)?
        .level_max;
    {
        let body = table_body(&accounts.rewards, TableKind::Rewards)
            .map_err(|_| error!(SessionError::InvalidRegistryTable))?;
        let loot = MapLoot {
            min_players: session.min_players,
            rules: map_rules(&BALANCE.loot, session.tier),
            table_of: |id: u8| map_tables::table_rules(&body, id),
        };
        check_secret(
            &place,
            &secret,
            &tiles(session),
            &limits,
            ambush_mask(&session.packs),
            |id| deadroll_mapcheck::loot::allowed_table(level_cap, id, &loot).map(|_| ()),
        )
        .map_err(rejected)?;
        if secret.kind == SecretKind::Stash {
            // The map's chests, the stashes found before and this one, with the boss table.
            let mut chests = [NO_TABLE; CHESTS_MAX];
            let visible = (session.visible_chests as usize).min(CHESTS_MAX);
            for (t, c) in chests.iter_mut().zip(session.chest_list()) {
                *t = c.data;
            }
            let mut stashes = [NO_TABLE; SECRETS_MAX];
            let mut stash_count = 0;
            for f in session.found_list() {
                if f.secret.kind == SecretKind::Stash && stash_count < SECRETS_MAX - 1 {
                    stashes[stash_count] = f.secret.a;
                    stash_count += 1;
                }
            }
            stashes[stash_count] = secret.a;
            check_stash_budget(
                level_cap,
                &chests[..visible],
                session.boss_table,
                &stashes[..stash_count + 1],
                &loot,
            )
            .map_err(rejected)?;
        }
    }
    drop(layout);
    if let Some(k) = secret.kind.counted() {
        let n = session.found_counts[k].saturating_add(1);
        require!(n <= session.declared[k], SessionError::TooManySecrets);
        session.found_counts[k] = n;
        let at = session.found_count as usize;
        require!(at < SECRETS_MAX, SessionError::TooManySecrets);
        session.found[at] = FoundSecret { leaf, secret };
        session.found_count += 1;
    }
    session.set_revealed(leaf);
    if !audit {
        session.fulfil(leaf);
        let slot = session.obliged_by;
        match (secret.kind, board) {
            (SecretKind::Trap, _) if session.player_alive(slot) => {
                let p = accounts
                    .participation
                    .as_mut()
                    .ok_or(SessionError::ParticipationMissing)?;
                require!(p.core.slot == slot, SessionError::ActorMismatch);
                p.core.hp = p.core.hp.saturating_sub(secret.a as u32);
                emit!(TrapTriggered {
                    session: session_key,
                    slot,
                    damage: secret.a,
                    hp: p.core.hp,
                });
                if p.core.hp == 0 {
                    player_down(session, session_key, slot, EMPTY_UNIT);
                }
            }
            (SecretKind::Ambush, _) if session.packs_alive & (1 << secret.a) != 0 => {
                // The trap springs where the player stands: their party goes into the pack's
                // arena, the monsters first (docs/MAP_DESCRIPTION.md 17.4). It replaces a pack
                // the same step engaged.
                session.mode = ArenaMode::Engaged;
                session.arena_pack = secret.a;
                session.engaged_by = slot;
                emit!(AmbushSprung {
                    session: session_key,
                    pack: secret.a,
                    slot,
                });
            }
            (SecretKind::Stash, Board::Map(c)) => {
                // It opens like a chest from now on (docs/PARTY_AND_TURNS.md 9.3).
                let n = session.chest_count as usize;
                require!(n < CHESTS_MAX, SessionError::TooManySecrets);
                session.chests[n] = MapPoint {
                    kind: PointKind::Chest,
                    x: c.x,
                    y: c.y,
                    data: secret.a,
                };
                session.chest_count += 1;
                let monsters = Monsters::load(&accounts.monsters, &session_key)?;
                crate::instructions::open_session::copy_loot_table(
                    &monsters,
                    &accounts.rewards,
                    secret.a,
                )?;
            }
            // A secret door is a door from now on (`GameSession::door_found_at`); nothing else
            // happens.
            _ => {}
        }
    }
    emit!(CellRevealed {
        session: session_key,
        leaf,
        kind: secret.kind as u8,
        a: secret.a,
        b: secret.b,
        c: secret.c,
        audit,
    });
    Ok(())
}
