use anchor_lang::prelude::*;
use deadroll_character::state::MAP_SKILLS;
use deadroll_rules::HeroStats;
use deadroll_skillcheck::NO_SKILL;
use deadroll_types::{seeds, SessionStatus};

use crate::{
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::PlayerReady,
    rules::{hero_ap, hero_initiative, party_hp_max},
    state::{external::read_character_gear, GameSession, Participation},
};

#[derive(Accounts)]
pub struct SetReady<'info> {
    /// Main wallet only: readiness is not a game move (docs/PARTY_AND_TURNS.md, section 2.1).
    pub player: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    #[account(
        mut,
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            participation.core.character.as_ref(),
        ],
        bump = participation.core.bump,
    )]
    pub participation: Box<Account<'info, Participation>>,
    /// CHECK: the participation's `Character`; written by the character CPI (lock / unlock).
    #[account(mut, address = participation.core.character @ SessionError::InvalidCharacter)]
    pub character: UncheckedAccount<'info>,
    /// CHECK: the character's `SkillSet`, checked by `character::lock_for_session` (seeds).
    pub skill_set: UncheckedAccount<'info>,
    /// CHECK: signer PDA for CPI into other game programs.
    #[account(seeds = [seeds::AUTHORITY], bump)]
    pub session_authority: UncheckedAccount<'info>,
    /// CHECK: registry `GameConfig`, forwarded to the callee, which checks the trust list.
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
    /// CHECK: the character program.
    #[account(address = deadroll_character::ID)]
    pub character_program: UncheckedAccount<'info>,
}

/// "Ready" in the lobby (docs/PARTY_AND_TURNS.md, 2.1), one main wallet transaction:
/// - `true`: takes `selection` onto the map (`SkillSet` indexes: 4 active, 1 reactive, 1
///   passive, docs/SKILL_BUILDER.md 3.4), the character program checks it and locks the
///   character (its bag and gear freeze until the party ends), and the participation copies the
///   character's HP, AP, initiative and gear for the whole party;
/// - `false`: clears the selection and unlocks the character (`selection` is ignored).
pub fn handler(ctx: Context<SetReady>, ready: bool, selection: [u8; MAP_SKILLS]) -> Result<()> {
    let accounts = ctx.accounts;
    let session_key = accounts.session.key();
    let session = &mut accounts.session;
    let participation = &mut accounts.participation;
    require_keys_eq!(
        participation.player,
        accounts.player.key(),
        SessionError::NotPlayer
    );
    require!(
        session.core.status == SessionStatus::Lobby,
        SessionError::NotInLobby
    );
    require!(
        participation.core.ready != ready,
        SessionError::ReadyUnchanged
    );
    let signer = SessionSigner {
        session_authority: &accounts.session_authority,
        config: &accounts.config,
        bump: ctx.bumps.session_authority,
    };

    let slot = participation.core.slot;
    if ready {
        cpi_out::character::lock_for_session(
            &signer,
            &accounts.character,
            &accounts.player,
            &accounts.skill_set,
            session_key,
            selection,
        )?;
        // The gear is frozen from here until the party ends, so one snapshot serves the whole
        // party.
        let (character, equipment, class_tier) = read_character_gear(&accounts.character)?;
        let hero = HeroStats::new(&character, class_tier).with_equipment(equipment);
        let hp_max = party_hp_max(character.hp_max, &hero);
        participation.core.hp = hp_max;
        participation.hp_max = hp_max;
        participation.max_ap = hero_ap(&hero);
        participation.equipment = equipment.0;
        participation.class_tier = class_tier;
        participation.skills = selection;
        session.player_initiative[slot as usize] = hero_initiative(&hero);
        session.ready_mask |= 1 << slot;
    } else {
        cpi_out::character::unlock(&signer, &accounts.character)?;
        participation.skills = [NO_SKILL; MAP_SKILLS];
        session.ready_mask &= !(1 << slot);
    }
    participation.core.ready = ready;

    emit!(PlayerReady {
        session: session_key,
        slot,
        ready,
        skills: participation.skills,
        hp: participation.core.hp,
    });
    Ok(())
}
