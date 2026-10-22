use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};
use deadroll_character::state::MAP_SKILLS;
use deadroll_skillcheck::NO_SKILL;
use deadroll_types::{
    seeds, ActiveState, ParticipationData, SessionStatus, EMPTY_ITEM, EQUIP_SLOTS, PARTY_MAX,
    STATES_MAX,
};

use crate::{
    errors::SessionError,
    events::PlayerJoined,
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
pub fn handler(ctx: Context<JoinSession>, contribution: u64, actor: Pubkey) -> Result<()> {
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

    let stash = &mut accounts.stash;
    stash.version = ACCOUNT_VERSION;
    stash.session = game_session;
    stash.character = character_key;
    stash.xp = 0;
    stash.resources = [0; STASH_RESOURCES];
    stash.bump = ctx.bumps.stash;

    emit!(PlayerJoined {
        session: game_session,
        character: character_key,
        player: accounts.player.key(),
        slot,
        contribution: paid,
        collected: accounts.escrow.collected,
    });
    Ok(())
}
