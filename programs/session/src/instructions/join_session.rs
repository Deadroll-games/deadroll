use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};
use deadroll_character::state::MAP_SKILLS;
use deadroll_skillcheck::NO_SKILL;
use deadroll_types::{
    seeds, ActiveState, CacheStatus, ParticipationData, SessionStatus, EMPTY_ITEM, EQUIP_SLOTS,
    PARTY_MAX, STATES_MAX,
};

use crate::{
    errors::SessionError,
    events::PlayerJoined,
    settlement,
    state::{
        external::read_character, CacheEntry, Escrow, GameSession, Participation, RewardCache,
        ACCOUNT_VERSION, CACHE_PARTIES,
    },
};

#[derive(Accounts)]
pub struct JoinSession<'info> {
    /// Main wallet that owns the character. Pays the contribution and the account rent.
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: `deadroll_character` `Character`, owner and fields checked in the handler.
    pub character: UncheckedAccount<'info>,
    #[account(
        init,
        payer = player,
        space = 8 + Participation::INIT_SPACE,
        seeds = [seeds::PARTICIPATION, session.key().as_ref(), character.key().as_ref()],
        bump,
    )]
    pub participation: Box<Account<'info, Participation>>,
    /// Patch 11: the character's reward caches (`RewardCache`), created on its first party; this
    /// party takes a free entry, and what is no longer safe to keep burns (see the handler).
    #[account(
        init_if_needed,
        payer = player,
        space = 8 + RewardCache::INIT_SPACE,
        seeds = [seeds::CACHE, character.key().as_ref()],
        bump,
    )]
    pub reward_cache: Box<Account<'info, RewardCache>>,
    #[account(
        mut,
        seeds = [seeds::ESCROW, session.key().as_ref()],
        bump = escrow.bump,
    )]
    pub escrow: Box<Account<'info, Escrow>>,
    pub system_program: Program<'info, System>,
}

/// `contribution` is the player's share of the party price; only what is still missing is taken.
/// `actor` stays in the signature (agents/CONTRACTS.md 8) but must be `Pubkey::default()`: the
/// session key and its fee prepayment come with Ready, where the main wallet binds the key with
/// `set_session_key` in the `set_ready(true)` transaction (docs/PARTY_AND_TURNS.md, 2.1).
///
/// Joining takes a place and pays; the character is not locked yet. `set_ready(true)` takes the
/// skills onto the map, locks the character (its bag and gear freeze) and copies its stats
/// (docs/PARTY_AND_TURNS.md, 2.1).
///
/// Patch 11, the character's reward caches (decisions of patch 11): a `Claimed` entry (the owner
/// did not press "Done") burns; an `Open` entry burns if its party ended and the DM's deadline
/// passed (the player could have claimed it), else it waits; then this party takes a free entry
/// (`TooManyPendingParties` if none). `remaining_accounts`: the `GameSession` of every `Open`
/// entry, in entry order.
pub fn handler<'info>(
    ctx: Context<'info, JoinSession<'info>>,
    contribution: u64,
    actor: Pubkey,
) -> Result<()> {
    require_keys_eq!(actor, Pubkey::default(), SessionError::SessionKeyAtReady);
    let accounts = ctx.accounts;
    let game_session = accounts.session.key();
    let character_key = accounts.character.key();
    let character = read_character(&accounts.character)?;
    require_keys_eq!(
        character.owner,
        accounts.player.key(),
        SessionError::NotCharacterOwner
    );
    require!(character.alive, SessionError::CharacterDead);
    require!(character.in_session.is_none(), SessionError::CharacterBusy);

    let session = &mut accounts.session;
    require!(
        session.core.status == SessionStatus::Lobby,
        SessionError::NotInLobby
    );
    let slot = session.core.player_count;
    require!((slot as usize) < PARTY_MAX, SessionError::PartyFull);

    let remaining = session
        .core
        .price_lamports
        .saturating_sub(accounts.escrow.collected);
    let paid = contribution.min(remaining);
    if paid > 0 {
        transfer(
            CpiContext::new(
                accounts.system_program.key(),
                Transfer {
                    from: accounts.player.to_account_info(),
                    to: accounts.escrow.to_account_info(),
                },
            ),
            paid,
        )?;
        accounts.escrow.collected = accounts
            .escrow
            .collected
            .checked_add(paid)
            .ok_or(SessionError::Overflow)?;
    }

    session.core.players[slot as usize] = character_key;
    session.participations[slot as usize] = accounts.participation.key();
    session.core.player_count = slot + 1;
    session.player_pos[slot as usize] = session.entrance;
    session.alive_mask |= 1 << slot;

    // HP, AP, initiative and gear come with `set_ready(true)`.
    let participation = &mut accounts.participation;
    participation.core = ParticipationData {
        version: ACCOUNT_VERSION,
        game_session,
        character: character_key,
        // Bound at Ready (`set_session_key`); until then only the main wallet acts.
        session_key: Pubkey::default(),
        slot,
        x: session.entrance.x,
        y: session.entrance.y,
        hp: 0,
        action_points: 0,
        states: [ActiveState::default(); STATES_MAX],
        ready: false,
        bump: ctx.bumps.participation,
    };
    participation.player = accounts.player.key();
    participation.ap_seq = 0;
    participation.contribution = paid;
    participation.equipment = [EMPTY_ITEM; EQUIP_SLOTS];
    // Copied from the character at `set_ready(true)`, with the gear.
    participation.class_tier = 0;
    participation.skills = [NO_SKILL; MAP_SKILLS];

    // The cache of the party lives in `participation` (zero from `init`) until the settlement.
    let cache = &mut accounts.reward_cache;
    if cache.version == 0 {
        cache.version = ACCOUNT_VERSION;
        cache.character = character_key;
        cache.entries = [CacheEntry::EMPTY; CACHE_PARTIES];
        cache.bump = ctx.bumps.reward_cache;
    }
    let burned = refresh_caches(cache, ctx.remaining_accounts, Clock::get()?.unix_timestamp)?;
    let free = cache
        .entries
        .iter_mut()
        .find(|e| e.status == CacheStatus::Empty)
        .ok_or(SessionError::TooManyPendingParties)?;
    *free = CacheEntry {
        session: game_session,
        status: CacheStatus::Open,
        ..CacheEntry::EMPTY
    };

    emit!(PlayerJoined {
        session: game_session,
        character: character_key,
        player: accounts.player.key(),
        slot,
        contribution: paid,
        collected: accounts.escrow.collected,
        cache_burned: burned,
    });
    Ok(())
}

/// Burns the entries that are no longer safe to keep (see [`handler`]); returns them, bit per
/// entry. `sessions`: the `GameSession` of every `Open` entry, in entry order.
fn refresh_caches<'info>(
    cache: &mut RewardCache,
    sessions: &'info [AccountInfo<'info>],
    now: i64,
) -> Result<u8> {
    let open = cache
        .entries
        .iter()
        .filter(|e| e.status == CacheStatus::Open)
        .count();
    require!(sessions.len() == open, SessionError::InvalidCacheSessions);
    let mut sessions = sessions.iter();
    let mut burned = 0u8;
    for (i, e) in cache.entries.iter_mut().enumerate() {
        let burn = match e.status {
            CacheStatus::Empty => false,
            CacheStatus::Claimed => true,
            CacheStatus::Open => {
                let info = sessions.next().ok_or(SessionError::InvalidCacheSessions)?;
                require_keys_eq!(*info.key, e.session, SessionError::InvalidCacheSessions);
                let party = Box::new(Account::<GameSession>::try_from(info)?);
                settlement::require_ended(&party).is_ok() && now > settlement::deadline(&party)
            }
        };
        if burn {
            *e = CacheEntry::EMPTY;
            burned |= 1 << i;
        }
    }
    Ok(burned)
}
