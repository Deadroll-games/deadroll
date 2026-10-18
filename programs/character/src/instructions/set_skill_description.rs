use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::{
    errors::CharacterError,
    events::SkillDescribed,
    rules,
    state::{Character, SkillExtras, SkillSet, SKILL_DESCRIPTION_LEN},
};

/// The owner writes the description of one of the character's skills (docs/SKILL_BUILDER.md,
/// sections 2 and 12: up to 200 characters, the player's own words). Once: a written
/// description never changes, like the name. Main wallet only, outside a session, while the
/// character lives.
#[derive(Accounts)]
pub struct SetSkillDescription<'info> {
    pub owner: Signer<'info>,
    #[account(
        seeds = [seeds::CHARACTER, character.core.owner.as_ref(), &[character.core.index]],
        bump = character.core.bump,
    )]
    pub character: Box<Account<'info, Character>>,
    #[account(
        seeds = [seeds::SKILLS, character.key().as_ref()],
        bump = skill_set.bump,
    )]
    pub skill_set: Box<Account<'info, SkillSet>>,
    #[account(
        mut,
        seeds = [seeds::SKILL_EXTRAS, character.key().as_ref()],
        bump,
    )]
    pub skill_extras: AccountLoader<'info, SkillExtras>,
}

pub fn handler(
    ctx: Context<SetSkillDescription>,
    skill_index: u8,
    description: [u8; SKILL_DESCRIPTION_LEN],
) -> Result<()> {
    let character = &ctx.accounts.character;
    require_keys_eq!(
        ctx.accounts.owner.key(),
        character.core.owner,
        CharacterError::NotOwner
    );
    require!(character.core.alive, CharacterError::CharacterDead);
    require!(
        character.core.in_session.is_none(),
        CharacterError::AlreadyLocked
    );
    require!(
        skill_index < ctx.accounts.skill_set.count,
        CharacterError::NoSuchSkill
    );
    require!(
        rules::name_valid(&description),
        CharacterError::InvalidDescription
    );

    let mut extras = ctx.accounts.skill_extras.load_mut()?;
    let extra = &mut extras.extras[skill_index as usize];
    require!(
        !extra.has_description(),
        CharacterError::DescriptionAlreadySet
    );
    extra.description = description;

    emit!(SkillDescribed {
        character: character.key(),
        skill_index,
    });
    Ok(())
}
