use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};
use deadroll_types::{
    seeds, ActiveState, ParticipationData, SessionStatus, Stat, PARTY_MAX, STATES_MAX,
};

use crate::{
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::PlayerJoined,
    rules::player_ap,
    state::{
        external::read_character, Escrow, GameSession, Participation, TempStash, ACCOUNT_VERSION,
        STASH_RESOURCES,
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
    /// CHECK: `deadroll_character` `Character`, owner and fields checked in the handler; written by
    /// the character CPI.
    #[account(mut)]
    pub character: UncheckedAccount<'info>,
    #[account(
        init,
        payer = player,
        space = 8 + Participation::INIT_SPACE,
        seeds = [seeds::PARTICIPATION, session.key().as_ref(), character.key().as_ref()],
        bump,
    )]
    pub participation: Box<Account<'info, Participation>>,
    #[account(
        init,
        payer = player,
        space = 8 + TempStash::INIT_SPACE,
        seeds = [seeds::STASH, session.key().as_ref(), character.key().as_ref()],
        bump,
    )]
    pub stash: Box<Account<'info, TempStash>>,
    #[account(
        mut,
        seeds = [seeds::ESCROW, session.key().as_ref()],
        bump = escrow.bump,
    )]
    pub escrow: Box<Account<'info, Escrow>>,
    /// CHECK: signer PDA for CPI into other game programs.
    #[account(seeds = [seeds::AUTHORITY], bump)]
    pub session_authority: UncheckedAccount<'info>,
    /// CHECK: registry `GameConfig`, forwarded to the callee, which checks the trust list.
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
    /// CHECK: the character program.
    #[account(address = deadroll_character::ID)]
    pub character_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

/// `contribution` is the player's share of the party price; only what is still missing is taken.
/// `actor` is the session key that will act for this character (`Pubkey::default()` = none).
pub fn handler(ctx: Context<JoinSession>, contribution: u64, actor: Pubkey) -> Result<()> {
    let accounts = ctx.accounts;
    let session_key = accounts.session.key();
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
    session.core.player_count = slot + 1;
    session.player_pos[slot as usize] = session.entrance;
    session.alive_mask |= 1 << slot;

    let participation = &mut accounts.participation;
    participation.core = ParticipationData {
        version: ACCOUNT_VERSION,
        session: session_key,
        character: character_key,
        actor,
        slot,
        x: session.entrance.x,
        y: session.entrance.y,
        hp: character.hp_max,
        action_points: 0,
        states: [ActiveState::default(); STATES_MAX],
        ready: false,
        bump: ctx.bumps.participation,
    };
    participation.player = accounts.player.key();
    participation.max_ap = player_ap(
        character.race,
        character.class,
        character.stats[Stat::Dexterity as usize],
    );
    participation.ap_seq = 0;
    participation.contribution = paid;

    let stash = &mut accounts.stash;
    stash.version = ACCOUNT_VERSION;
    stash.session = session_key;
    stash.character = character_key;
    stash.xp = 0;
    stash.resources = [0; STASH_RESOURCES];
    stash.bump = ctx.bumps.stash;

    cpi_out::character::lock_for_session(
        &SessionSigner {
            session_authority: &accounts.session_authority,
            config: &accounts.config,
            bump: ctx.bumps.session_authority,
        },
        &accounts.character,
        &accounts.player,
        session_key,
    )?;

    emit!(PlayerJoined {
        session: session_key,
        character: character_key,
        player: accounts.player.key(),
        slot,
        contribution: paid,
        collected: accounts.escrow.collected,
    });
    Ok(())
}
