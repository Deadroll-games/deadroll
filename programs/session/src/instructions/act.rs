use anchor_lang::prelude::*;
use deadroll_skillcheck::Target;
use deadroll_types::{seeds, Cell, SessionStatus};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    combat_input::{action_ap, action_range, skill_use, target_side, Side, SkillUse},
    errors::SessionError,
    events::{ActionRequested, SkillUsed},
    line::skill_line,
    party::party_keys,
    rules::{cooldown_after_use, in_ally_range, in_range, Unit, BASE_ATTACK},
    state::{
        external::MapLayoutView, GameSession, MonsterState, Monsters, Participation, PendingAction,
    },
    vrf::{self, CallbackAccounts, RequestAccounts},
};

#[derive(Accounts, Session)]
pub struct Act<'info> {
    /// Main wallet or the participation's session key; pays the VRF fee.
    #[account(mut)]
    pub signer: Signer<'info>,
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
    /// CHECK: the participation's `Character`, decoded by the callback (passed on to it here).
    #[account(address = participation.core.character @ SessionError::InvalidCharacter)]
    pub character: UncheckedAccount<'info>,
    /// CHECK: the character's `SkillSet` (address from the seeds); its skills are read by
    /// `read_skill`, and the callback burns it if the character dies.
    #[account(
        seeds = [seeds::SKILLS, participation.core.character.as_ref()],
        bump,
        seeds::program = deadroll_character::ID,
    )]
    pub skill_set: UncheckedAccount<'info>,
    /// CHECK: the character's `Bag` (address from the seeds), passed on to the callback.
    #[account(
        seeds = [seeds::BAG, participation.core.character.as_ref()],
        bump,
        seeds::program = deadroll_character::ID,
    )]
    pub bag: UncheckedAccount<'info>,
    /// Read-only, never Borsh-decoded: owner and discriminator checked here, session in `Monsters`.
    pub monsters: LazyAccount<'info, MonsterState>,
    /// MagicBlock `SessionTokenV2` of the session key; none when the main wallet signs.
    #[session(signer = signer, authority = participation.player)]
    pub session_token: Option<Account<'info, SessionTokenV2>>,
    /// CHECK: our `["identity"]` PDA, signs the VRF request.
    #[account(seeds = [vrf::PROGRAM_IDENTITY_SEED], bump)]
    pub program_identity: UncheckedAccount<'info>,
    /// CHECK: VRF oracle queue, address checked.
    #[account(mut, address = vrf::oracle_queue())]
    pub oracle_queue: UncheckedAccount<'info>,
    /// CHECK: MagicBlock VRF program, address checked (not `Program<_>`: localnet has none).
    #[account(address = vrf::vrf_program_id())]
    pub vrf_program: UncheckedAccount<'info>,
    /// CHECK: SlotHashes sysvar, address checked.
    #[account(address = vrf::slot_hashes())]
    pub slot_hashes: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// The ally a heal or buff targets when it is not the caster; `None` for every other action
    /// (a self-cast, an attack, a debuff). Any `Participation` of this session; the handler
    /// checks its slot.
    #[account(
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            target_participation.core.character.as_ref(),
        ],
        bump = target_participation.core.bump,
    )]
    pub target_participation: Option<Box<Account<'info, Participation>>>,
    /// CHECK: the session's `MapLayout` (patch 5): a wall cuts a line target. Address pinned at
    /// `open_session`, read by `MapLayoutView`.
    #[account(address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
}

/// `skill_slot`: a slot of the character's `SkillSet`, or `BASE_ATTACK` for the weapon.
/// `target`: by the skill's side (`combat_input::target_side`): a monster index for the weapon,
/// damage and debuff skills, a player slot (the caster's own included) for heal and buff skills.
#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.participation.player,
    SessionError::Unauthorized
)]
pub fn handler(ctx: Context<Act>, skill_slot: u8, target: u8) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let participation = &mut accounts.participation;
    auth::check_bound(
        accounts.signer.key,
        &participation.player,
        &participation.core.session_key,
    )?;
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    // Patch 6: a player down in the last callback is finalized first (`start_turn`).
    require!(session.pending_deaths == 0, SessionError::DeathsPending);
    let slot = participation.core.slot;
    require!(
        session.current_unit() == Some(Unit::Player(slot)),
        SessionError::NotYourTurn
    );
    require!(session.turn_started(), SessionError::TurnNotStarted);
    require!(
        session.player_alive(slot) && participation.core.hp > 0,
        SessionError::UnitDead
    );
    // One attack or active skill per turn; movement spends what AP are left (PARTY_AND_TURNS 5.1).
    require!(!session.acted(), SessionError::AlreadyActed);

    let skill = if skill_slot == BASE_ATTACK {
        None
    } else {
        // Only the active skills taken onto this map (docs/SKILL_BUILDER.md, 3.4), as copied
        // at `set_ready(true)`.
        require!(
            participation.active_selected(skill_slot),
            SessionError::SkillNotSelected
        );
        let skill = participation
            .map_skill(skill_slot)
            .ok_or(SessionError::SkillNotSelected)?;
        require!(
            skill_use(&skill) == SkillUse::Ok,
            SessionError::InvalidSkill
        );
        // `read_skill` checked `skill_slot < SKILLS_MAX`. The weapon has no cooldown.
        require!(
            participation.cooldowns[skill_slot as usize] == 0,
            SessionError::SkillOnCooldown
        );
        Some(skill)
    };
    // Checked for every action: the callback loads it too, and a callback that fails would leave
    // the roll pending.
    let monsters = Monsters::load(accounts.monsters.as_ref(), &session.key())?;
    let from = Cell {
        x: participation.core.x,
        y: participation.core.y,
    };

    // The target must be on the skill's side: an enemy for the weapon, damage and debuffs, an
    // ally (or the caster) for heals and buffs (docs/SKILL_BUILDER.md, section 2).
    let (target_unit, to) = match target_side(skill.as_ref()) {
        Side::Enemy => {
            require!(
                accounts.target_participation.is_none(),
                SessionError::UnexpectedAllyAccount
            );
            require!(target < monsters.count()?, SessionError::InvalidTarget);
            let monster = monsters.get(target)?;
            require!(monster.alive, SessionError::InvalidTarget);
            let to = Cell {
                x: monster.x,
                y: monster.y,
            };
            (Unit::Monster(target), Some((to, false)))
        }
        Side::Ally if target == slot => {
            // A self-cast resolves on the caster's own participation: no second account.
            require!(
                accounts.target_participation.is_none(),
                SessionError::UnexpectedAllyAccount
            );
            (Unit::Player(slot), None)
        }
        Side::Ally => {
            require!(
                session.player_alive(target),
                SessionError::InvalidAllyTarget
            );
            let ally = accounts
                .target_participation
                .as_ref()
                .ok_or(SessionError::AllyAccountMissing)?;
            require!(
                ally.core.slot == target && ally.core.hp > 0,
                SessionError::InvalidAllyTarget
            );
            let to = Cell {
                x: ally.core.x,
                y: ally.core.y,
            };
            (Unit::Player(target), Some((to, true)))
        }
    };
    // The range tag of the skill, or the weapon's for the basic attack (patch 5).
    let range = action_range(&participation.equipment, skill.as_ref());
    if let Some((to, ally)) = to {
        let reaches = if ally {
            in_ally_range(range, from, to)
        } else {
            in_range(range, from, to)
        };
        require!(reaches, SessionError::OutOfRange);
        // A line runs from the caster through the target (docs/SKILL_BUILDER.md, 2 and 5.3);
        // a wall before the target cuts it.
        if skill.is_some_and(|s| s.blocks.target == Target::Line) {
            let layout = MapLayoutView::load(&accounts.layout)?;
            let line = skill_line(session, &layout, from, to, range);
            require!(line.index_of(to).is_some(), SessionError::LineBlocked);
        }
    }

    // Patch 6: a monster's debuff aura over the player counts too.
    let field = crate::monster_cb::Field::load(session, &monsters)?;
    participation.refresh_ap(session.turn_seq, &field.view());
    participation.core.action_points = participation
        .core
        .action_points
        .checked_sub(action_ap(skill.as_ref()))
        .ok_or(SessionError::NotEnoughActionPoints)?;
    // Any action resets every escape intent (docs/PARTY_AND_TURNS.md, 7.3).
    session.flee_mask = 0;

    let attacker = Unit::Player(slot).id();
    let target_unit = target_unit.id();
    session.pending = PendingAction {
        attacker,
        target: target_unit,
        skill_slot,
        participation: participation.key(),
    };
    // Patch 5: an ally of a buff comes with the party accounts, not as `target_participation`.
    session.pending_target = Pubkey::default();
    session.core.pending_vrf = true;
    session.acted_seq = session.turn_seq;
    if let Some(skill) = &skill {
        participation.cooldowns[skill_slot as usize] = cooldown_after_use(skill.cooldown);
    }

    let session_key = session.key();
    let (party, party_len) = party_keys(session, slot);
    vrf::request(
        RequestAccounts {
            payer: &accounts.signer,
            program_identity: &accounts.program_identity,
            oracle_queue: &accounts.oracle_queue,
            system_program: &accounts.system_program,
            slot_hashes: &accounts.slot_hashes,
        },
        ctx.bumps.program_identity,
        vrf::caller_seed(&session_key, session.core.action_counter),
        &CallbackAccounts {
            session: session_key,
            monsters: accounts.monsters.key(),
            participation: participation.key(),
            character: accounts.character.key(),
            skill_set: accounts.skill_set.key(),
            bag: accounts.bag.key(),
            target_participation: None,
            layout: session.layout,
            party,
            party_len,
        },
    )?;

    if let Some(skill) = &skill {
        emit!(SkillUsed {
            session: session_key,
            unit: attacker,
            skill_slot,
            cooldown: skill.cooldown,
            cooldowns: participation.cooldowns,
        });
    }
    emit!(ActionRequested {
        session: session_key,
        action: session.core.action_counter,
        attacker,
        target: target_unit,
        skill_slot,
        action_points: participation.core.action_points,
    });
    Ok(())
}
