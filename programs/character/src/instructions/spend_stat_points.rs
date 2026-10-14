use anchor_lang::prelude::*;
use deadroll_rules::SpendError;
use deadroll_types::{seeds, STATS_COUNT};

use crate::{errors::CharacterError, events::StatPointsSpent, rules, state::Character};

/// The owner spends level-up stat points (docs/CHARACTER.md, section 5), all chosen at once.
/// Main wallet only, no session key (docs/GAME_COMMISSIONS.md, section 4: it changes the
/// character outside a party). Not while the character is in a session.
#[derive(Accounts)]
pub struct SpendStatPoints<'info> {
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::CHARACTER, character.core.owner.as_ref(), &[character.core.index]],
        bump = character.core.bump,
    )]
    pub character: Box<Account<'info, Character>>,
}

pub fn handler(ctx: Context<SpendStatPoints>, points: [u16; STATS_COUNT]) -> Result<()> {
    let key = ctx.accounts.character.key();
    let character: &mut Character = &mut ctx.accounts.character;
    require_keys_eq!(
        ctx.accounts.owner.key(),
        character.core.owner,
        CharacterError::NotOwner
    );
    let c = &mut character.core;
    require!(c.alive, CharacterError::CharacterDead);
    require!(c.in_session.is_none(), CharacterError::AlreadyLocked);

    let (stats, left) =
        rules::spend_points(&c.stats, character.unspent_points, &points).map_err(|e| match e {
            SpendError::Nothing => error!(CharacterError::NoPointsSpent),
            SpendError::NotEnough => error!(CharacterError::NotEnoughStatPoints),
            SpendError::Overflow => error!(CharacterError::MathOverflow),
        })?;
    c.stats = stats;
    // Endurance raises the stored maximum HP.
    c.hp_max = rules::hp_max(c.class, &c.stats, c.level);
    c.state_version = c
        .state_version
        .checked_add(1)
        .ok_or(CharacterError::MathOverflow)?;
    character.unspent_points = left;

    emit!(StatPointsSpent {
        character: key,
        points,
        stats,
        unspent_points: left,
        hp_max: c.hp_max,
    });
    Ok(())
}
