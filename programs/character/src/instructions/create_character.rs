use anchor_lang::prelude::*;
use deadroll_skillcheck::{check_loadout, derive, validate_starting, SkillBlocks, STARTING_MAX};
use deadroll_types::{
    seeds, Aspect, CharacterData, Class, LayerRef, Race, BAG_SLOTS, LAYERS_MAX, STATS_COUNT,
};

#[cfg(feature = "nft")]
use crate::cpi_out::nft;
use crate::{
    cpi_out::nft::MPL_CORE_ID,
    errors::CharacterError,
    events::CharacterCreated,
    rules::{self, ACCOUNT_VERSION, START_TIER},
    state::{
        Bag, Character, PlayerProfile, SkillRecord, SkillSet, NAME_LEN, RESOURCE_KINDS, SKILLS_MAX,
        SKILL_NAME_LEN,
    },
    trust,
};

/// A starting skill: the blocks (usually a preset from `deadroll_skillcheck::presets`) and the
/// player's name for it.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct NewSkill {
    pub blocks: SkillBlocks,
    pub name: [u8; SKILL_NAME_LEN],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CreateCharacterArgs {
    pub name: [u8; NAME_LEN],
    pub race: Race,
    pub class: Class,
    /// Human: Piercing, Blunt or Blood. Forest elf: Fire, Cold or Lightning.
    pub predisposition: Aspect,
    /// Indexed by `Stat`: `STAT_BASE` each plus exactly `STAT_POINTS_START` spent.
    pub stats: [u16; STATS_COUNT],
    pub appearance: [LayerRef; LAYERS_MAX],
    /// Up to 6: at most 4 active, 1 reactive, 1 passive, limited by class.
    pub skills: Vec<NewSkill>,
}

#[derive(Accounts)]
pub struct CreateCharacter<'info> {
    /// The player: pays rent and owns the character and its NFT.
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::PROFILE, owner.key().as_ref()],
        bump = profile.bump,
    )]
    pub profile: Account<'info, PlayerProfile>,
    #[account(
        init,
        payer = owner,
        space = 8 + Character::INIT_SPACE,
        seeds = [seeds::CHARACTER, owner.key().as_ref(), &[profile.character_count]],
        bump,
    )]
    pub character: Box<Account<'info, Character>>,
    #[account(
        init,
        payer = owner,
        space = 8 + SkillSet::INIT_SPACE,
        seeds = [seeds::SKILLS, character.key().as_ref()],
        bump,
    )]
    pub skill_set: Box<Account<'info, SkillSet>>,
    #[account(
        init,
        payer = owner,
        space = 8 + Bag::INIT_SPACE,
        seeds = [seeds::BAG, character.key().as_ref()],
        bump,
    )]
    pub bag: Box<Account<'info, Bag>>,
    /// CHECK: registry `GameConfig`: owner, address and layout checked in `trust::read_config`.
    pub config: UncheckedAccount<'info>,
    /// New keypair for the Core asset. Required with the `nft` feature.
    #[account(mut)]
    pub asset: Option<Signer<'info>>,
    /// CHECK: mpl-core program, address checked. Required with the `nft` feature.
    #[account(address = MPL_CORE_ID)]
    pub mpl_core_program: Option<UncheckedAccount<'info>>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreateCharacter>, args: CreateCharacterArgs) -> Result<()> {
    require!(rules::name_valid(&args.name), CharacterError::InvalidName);
    require!(
        rules::predisposition_allowed(args.race, args.predisposition),
        CharacterError::PredispositionNotAllowed
    );
    require!(
        rules::stats_valid(&args.stats),
        CharacterError::InvalidStats
    );
    require!(
        args.skills.len() <= STARTING_MAX,
        CharacterError::TooManySkills
    );

    let mut skills = [SkillRecord::default(); SKILLS_MAX];
    for (i, s) in args.skills.iter().enumerate() {
        if let Err(e) = validate_starting(&s.blocks, args.class) {
            msg!("skill {}: {:?}", i, e);
            return err!(CharacterError::InvalidSkill);
        }
        require!(rules::name_valid(&s.name), CharacterError::InvalidName);
        let d = derive(&s.blocks);
        skills[i] = SkillRecord {
            blocks: s.blocks,
            shares_bp: d.shares_bp,
            power_bp: d.power_bp,
            fusion: d.fusion,
            cooldown: d.cooldown,
            name: s.name,
        };
    }
    if let Err(e) = check_loadout(args.skills.iter().map(|s| &s.blocks)) {
        msg!("loadout: {:?}", e);
        return err!(CharacterError::InvalidSkill);
    }

    let config = trust::read_config(&ctx.accounts.config)?;
    let owner = ctx.accounts.owner.key();
    let character_key = ctx.accounts.character.key();
    let profile = &mut ctx.accounts.profile;
    let index = profile.character_count;
    profile.character_count = index
        .checked_add(1)
        .ok_or(CharacterError::TooManyCharacters)?;

    let nft = mint_nft(&ctx, &args, &config.uri_base)?;

    ctx.accounts.character.set_inner(Character {
        core: CharacterData {
            version: ACCOUNT_VERSION,
            owner,
            index,
            nft,
            race: args.race,
            class: args.class,
            level: 1,
            xp: 0,
            stats: args.stats,
            predisposition: args.predisposition,
            predisposition_tier: START_TIER,
            hp_max: rules::hp_max(args.class, &args.stats, 1),
            alive: true,
            in_session: None,
            appearance: args.appearance,
            state_version: 0,
            bump: ctx.bumps.character,
        },
        name: args.name,
        unspent_points: 0,
        nft_frozen: false,
    });
    ctx.accounts.skill_set.set_inner(SkillSet {
        version: ACCOUNT_VERSION,
        character: character_key,
        count: args.skills.len() as u8,
        skills,
        bump: ctx.bumps.skill_set,
    });
    ctx.accounts.bag.set_inner(Bag {
        version: ACCOUNT_VERSION,
        character: character_key,
        resources: [0; RESOURCE_KINDS],
        items: [Pubkey::default(); BAG_SLOTS],
        item_count: 0,
        bump: ctx.bumps.bag,
    });

    emit!(CharacterCreated {
        character: character_key,
        owner,
        index,
        race: args.race,
        class: args.class,
        predisposition: args.predisposition,
        nft,
        skill_count: args.skills.len() as u8,
    });
    Ok(())
}

/// Mints the Core NFT; returns the asset address.
#[cfg(feature = "nft")]
fn mint_nft(
    ctx: &Context<CreateCharacter>,
    args: &CreateCharacterArgs,
    uri_base: &[u8],
) -> Result<Option<Pubkey>> {
    let a = &ctx.accounts;
    let (Some(asset), Some(mpl_core)) = (&a.asset, &a.mpl_core_program) else {
        return err!(CharacterError::NftAccountsMissing);
    };
    let character = a.character.to_account_info();
    let owner = a.owner.to_account_info();
    nft::mint(
        nft::NftAccounts {
            mpl_core: &mpl_core.to_account_info(),
            asset: &asset.to_account_info(),
            payer: &owner,
            system_program: &a.system_program.to_account_info(),
        },
        &owner,
        &character,
        rules::name_str(&args.name).into(),
        nft::character_uri(uri_base, character.key, 0),
    )?;
    Ok(Some(asset.key()))
}

/// FALLBACK(nft): built without the `nft` feature, the character is PDA only.
#[cfg(not(feature = "nft"))]
fn mint_nft(
    ctx: &Context<CreateCharacter>,
    _args: &CreateCharacterArgs,
    _uri_base: &[u8],
) -> Result<Option<Pubkey>> {
    require!(ctx.accounts.asset.is_none(), CharacterError::NftDisabled);
    Ok(None)
}
