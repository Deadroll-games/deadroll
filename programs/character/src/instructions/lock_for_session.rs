use anchor_lang::prelude::*;
use deadroll_skillcheck::{check_map_selection, Category};
use deadroll_types::seeds;

use crate::{
    errors::CharacterError,
    events::{CharacterLocked, SkillsSelected},
    state::{Character, SkillSet, MAP_SKILLS, SKILLS_MAX},
    trust,
};

/// CPI from session at `set_ready(true)`: checks the character and the skills it takes onto the
/// map, and locks it into `session` (docs/ARCHITECTURE.md, 4.1). The lock freezes the bag and the
/// gear until the party ends (`unlock`). Accounts start with `[character, session_authority,
/// config]` like every session-only instruction here.
#[derive(Accounts)]
pub struct LockForSession<'info> {
    #[account(mut)]
    pub character: Box<Account<'info, Character>>,
    /// The session program's `["authority"]` PDA, checked against `GameConfig`.
    pub session_authority: Signer<'info>,
    /// CHECK: registry `GameConfig`: owner, address and layout checked in `trust::read_config`.
    pub config: UncheckedAccount<'info>,
    /// The player, signer of `set_ready`: a session cannot lock someone else's character.
    #[account(address = character.core.owner @ CharacterError::NotOwner)]
    pub owner: Signer<'info>,
    #[account(
        seeds = [seeds::SKILLS, character.key().as_ref()],
        bump = skill_set.bump,
    )]
    pub skill_set: Box<Account<'info, SkillSet>>,
}

/// `selection`: `SkillSet` indexes, 4 active, 1 reactive, 1 passive
/// (`deadroll_skillcheck::check_map_selection`).
pub fn handler(
    ctx: Context<LockForSession>,
    session: Pubkey,
    selection: [u8; MAP_SKILLS],
) -> Result<()> {
    trust::check_session_caller(&ctx.accounts.config, ctx.accounts.session_authority.key)?;
    let set = &ctx.accounts.skill_set;
    let count = (set.count as usize).min(SKILLS_MAX);
    let mut owned = [Category::default(); SKILLS_MAX];
    for (c, s) in owned.iter_mut().zip(&set.skills[..count]) {
        *c = s.blocks.category;
    }
    if let Err(e) = check_map_selection(&owned[..count], &selection) {
        msg!("selection: {:?}", e);
        return err!(CharacterError::InvalidSkillSelection);
    }

    let character = ctx.accounts.character.key();
    let c = &mut ctx.accounts.character.core;
    require!(c.alive, CharacterError::CharacterDead);
    require!(c.in_session.is_none(), CharacterError::AlreadyLocked);
    c.in_session = Some(session);
    emit!(CharacterLocked { character, session });
    emit!(SkillsSelected {
        character,
        session,
        selection,
    });
    Ok(())
}
