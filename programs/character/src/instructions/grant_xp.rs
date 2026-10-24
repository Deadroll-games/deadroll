use anchor_lang::prelude::*;

use crate::{errors::CharacterError, events::XpGranted, rules, state::Character, trust};

/// CPI from session when the DM settles a victory (patch 11, `settle_party`) or a player claims
/// after the DM's deadline: the experience of the party's cache (level-ups give stat points and
/// HP). Resources do not go into the bag here: the owner lays them out with "Done" (`apply_bag`).
/// The character is free once a party ends, so it may already be in another party: only a living
/// one is required.
#[derive(Accounts)]
pub struct GrantXp<'info> {
    #[account(mut)]
    pub character: Box<Account<'info, Character>>,
    /// The session program's `["authority"]` PDA, checked against `GameConfig`.
    pub session_authority: Signer<'info>,
    /// CHECK: registry `GameConfig`: owner, address and layout checked in `trust::read_config`.
    pub config: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<GrantXp>, xp: u32) -> Result<()> {
    trust::check_session_caller(&ctx.accounts.config, ctx.accounts.session_authority.key)?;
    let key = ctx.accounts.character.key();
    let character: &mut Character = &mut ctx.accounts.character;
    let c = &mut character.core;
    require!(c.alive, CharacterError::CharacterDead);

    c.xp = c.xp.checked_add(xp).ok_or(CharacterError::MathOverflow)?;
    let level = rules::level_for(c.xp);
    if level > c.level {
        character.unspent_points = rules::level_up_points(c.level, level)
            .and_then(|p| character.unspent_points.checked_add(p))
            .ok_or(CharacterError::MathOverflow)?;
        c.level = level;
        c.hp_max = rules::hp_max(c.class, &c.stats, level);
        c.state_version = c
            .state_version
            .checked_add(1)
            .ok_or(CharacterError::MathOverflow)?;
    }

    emit!(XpGranted {
        character: key,
        xp,
        level: c.level,
        total_xp: c.xp,
    });
    Ok(())
}
