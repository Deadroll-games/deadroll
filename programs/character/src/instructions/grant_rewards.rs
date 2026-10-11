use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::{
    errors::CharacterError,
    events::RewardsGranted,
    rules,
    state::{Bag, Character, RESOURCE_KINDS},
    trust,
};

/// CPI from session on victory or a peaceful exit: XP (level-ups give stat points and HP) and
/// resources into the bag.
#[derive(Accounts)]
pub struct GrantRewards<'info> {
    #[account(mut)]
    pub character: Box<Account<'info, Character>>,
    /// The session program's `["authority"]` PDA, checked against `GameConfig`.
    pub session_authority: Signer<'info>,
    /// CHECK: registry `GameConfig`: owner, address and layout checked in `trust::read_config`.
    pub config: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [seeds::BAG, character.key().as_ref()],
        bump = bag.bump,
    )]
    pub bag: Box<Account<'info, Bag>>,
}

pub fn handler(
    ctx: Context<GrantRewards>,
    xp: u32,
    resources: [u32; RESOURCE_KINDS],
) -> Result<()> {
    trust::check_session_caller(&ctx.accounts.config, ctx.accounts.session_authority.key)?;
    let key = ctx.accounts.character.key();
    let character: &mut Character = &mut ctx.accounts.character;
    let c = &mut character.core;
    require!(c.in_session.is_some(), CharacterError::NotLocked);
    require!(c.alive, CharacterError::CharacterDead);

    c.xp = c.xp.checked_add(xp).ok_or(CharacterError::MathOverflow)?;
    let level = rules::level_for(c.xp);
    if level > c.level {
        let gained = u8::try_from(level - c.level).map_err(|_| CharacterError::MathOverflow)?;
        character.unspent_points = gained
            .checked_mul(rules::STAT_POINTS_PER_LEVEL)
            .and_then(|p| character.unspent_points.checked_add(p))
            .ok_or(CharacterError::MathOverflow)?;
        c.level = level;
        c.hp_max = rules::hp_max(c.class, &c.stats, level);
        c.state_version = c
            .state_version
            .checked_add(1)
            .ok_or(CharacterError::MathOverflow)?;
    }

    let bag = &mut ctx.accounts.bag;
    for (have, add) in bag.resources.iter_mut().zip(resources) {
        *have = have.saturating_add(add);
    }

    emit!(RewardsGranted {
        character: key,
        xp,
        resources,
        level: c.level,
        total_xp: c.xp,
    });
    Ok(())
}
