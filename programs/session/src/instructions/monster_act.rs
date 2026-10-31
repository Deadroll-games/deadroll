use anchor_lang::prelude::*;
use deadroll_rules::{
    input::{skill_use, target_side, Side, SkillUse},
    line::line_ends,
    monster::{kin_reaches, record},
    Footprint, MonsterSkillStats,
};
use deadroll_skillcheck::Target;
use deadroll_types::{seeds, Cell, SessionStatus, MONSTER_SLOTS};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    errors::SessionError,
    events::{ActionRequested, SkillUsed},
    line::skill_line,
    party::party_keys,
    rules::{cooldown_after_use, in_ally_range, in_range, monster_range, Unit, BASIC_ATTACK_AP},
    state::{external::MapLayoutView, GameSession, Monsters, Participation, PendingAction},
    vrf::{self, CallbackAccounts, RequestAccounts},
};

#[derive(Accounts, Session)]
pub struct MonsterAct<'info> {
    /// DM main wallet or the DM's session key; pays the VRF fee.
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place).
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
    /// The target player's participation; for a heal or buff on a monster (patch 6), the
    /// participation of the lowest living player slot (the callback's account list is fixed).
    #[account(
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            participation.core.character.as_ref(),
        ],
        bump = participation.core.bump,
    )]
    pub participation: Box<Account<'info, Participation>>,
    /// CHECK: the target's `Character`; the callback marks it dead at 0 HP.
    #[account(address = participation.core.character @ SessionError::InvalidCharacter)]
    pub character: UncheckedAccount<'info>,
    /// CHECK: the target's `SkillSet` (address from the seeds); burned if the hit kills.
    #[account(
        seeds = [seeds::SKILLS, participation.core.character.as_ref()],
        bump,
        seeds::program = deadroll_character::ID,
    )]
    pub skill_set: UncheckedAccount<'info>,
    /// CHECK: the target's `Bag` (address from the seeds); burned if the hit kills.
    #[account(
        seeds = [seeds::BAG, participation.core.character.as_ref()],
        bump,
        seeds::program = deadroll_character::ID,
    )]
    pub bag: UncheckedAccount<'info>,
    /// MagicBlock `SessionTokenV2` of the session key; none when the main wallet signs.
    #[session(signer = signer, authority = session.core.dm)]
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
    /// CHECK: the session's `MapLayout` (patch 6): a wall cuts a line target. Address pinned at
    /// `open_session`, read by `MapLayoutView`.
    #[account(address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
}

/// Monster `monster` acts with `action`: `NATURAL_ATTACK`, or `1 + slot` for the active skill in
/// that monster slot (docs/BESTIARY.md 3.2; patch 6). `target`, by the skill's side: a player slot
/// for the natural attack, damage and debuff skills; a monster index (itself included) for heal
/// and buff skills, of its own kin for a kin-only skill.
#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.session.core.dm,
    SessionError::Unauthorized
)]
pub fn handler(ctx: Context<MonsterAct>, monster: u8, action: u8, target: u8) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    auth::check_bound(accounts.signer.key, &session.core.dm, &session.dm_actor)?;
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    // Patch 6: a player down in the last callback is finalized first (`start_turn`).
    require!(session.pending_deaths == 0, SessionError::DeathsPending);
    session.require_no_reveals()?;
    require!(
        action as usize <= MONSTER_SLOTS,
        SessionError::InvalidAction
    );
    let monsters = Monsters::load(&accounts.monsters, &session.key())?;
    require!(monster < monsters.count()?, SessionError::InvalidMonster);
    require!(
        session.current_unit() == Some(Unit::Monster(monster)),
        SessionError::NotYourTurn
    );
    require!(session.turn_started(), SessionError::TurnNotStarted);
    session.require_in_play()?;
    let mut state = monsters.get(monster)?;
    require!(state.alive, SessionError::UnitDead);
    // One attack per turn, like players (PARTY_AND_TURNS 5.1).
    require!(!session.acted(), SessionError::AlreadyActed);

    // The skill: an active one in that slot, off cooldown (the natural attack has none).
    let slot = action.checked_sub(1);
    let skill: Option<MonsterSkillStats> = match slot {
        None => None,
        Some(k) => {
            let skill = monsters
                .skill(monster, k)?
                .ok_or(SessionError::MonsterSkillMissing)?;
            require!(
                skill_use(&skill.stats) == SkillUse::Ok,
                SessionError::MonsterSkillMissing
            );
            // Triples: a silenced monster has only its natural attack.
            require!(
                !deadroll_combat::silenced(&monsters.mods(monster)?),
                SessionError::Silenced
            );
            require!(
                monsters.skill_cooldowns(monster)?[k as usize] == 0,
                SessionError::SkillOnCooldown
            );
            Some(skill)
        }
    };
    let profile = monsters.profile(monster, state.level)?;
    // Patch B: a large monster acts from its whole square.
    let from = monsters.footprint(monster)?;
    let range = skill.map_or(monster_range(profile.form), |k| k.stats.ext.range);
    let participation = &accounts.participation;
    let target_unit = match target_side(skill.as_ref().map(|k| &k.stats)) {
        Side::Enemy => {
            require!(
                participation.core.slot == target
                    && session.player_alive(target)
                    && participation.core.hp > 0,
                SessionError::InvalidTarget
            );
            let to = Cell {
                x: participation.core.x,
                y: participation.core.y,
            };
            require!(in_range(range, from, to), SessionError::OutOfRange);
            check_line(session, &accounts.layout, skill.as_ref(), from, to)?;
            Unit::Player(target)
        }
        Side::Ally => {
            // The callback's participation: the lowest living slot.
            let first = (0..session.core.player_count)
                .find(|&s| session.player_alive(s))
                .ok_or(SessionError::NotActive)?;
            require!(
                participation.core.slot == first,
                SessionError::InvalidPendingAccounts
            );
            require!(target < monsters.count()?, SessionError::InvalidAllyTarget);
            let ally = monsters.get(target)?;
            require!(ally.alive, SessionError::InvalidAllyTarget);
            let skill = skill.as_ref().ok_or(SessionError::MonsterSkillMissing)?;
            require!(
                kin_reaches(
                    skill.kin_only,
                    profile.kin,
                    monsters.record(target)?[record::KIN],
                    target == monster
                ),
                SessionError::NotSameKin
            );
            let to = monsters.footprint(target)?;
            require!(in_ally_range(range, from, to), SessionError::OutOfRange);
            if target != monster {
                check_line(session, &accounts.layout, Some(skill), from, to)?;
            }
            Unit::Monster(target)
        }
    };

    let ap = skill.map_or(BASIC_ATTACK_AP, |k| k.stats.ap_cost);
    state.action_points = state
        .action_points
        .checked_sub(ap)
        .ok_or(SessionError::NotEnoughActionPoints)?;
    monsters.set(monster, &state)?;
    let mut cooldowns = monsters.skill_cooldowns(monster)?;
    if let (Some(k), Some(skill)) = (slot, skill.as_ref()) {
        cooldowns[k as usize] = cooldown_after_use(skill.stats.cooldown);
        monsters.set_skill_cooldowns(monster, &cooldowns)?;
    }

    let attacker = Unit::Monster(monster).id();
    let target_unit = target_unit.id();
    session.pending = PendingAction {
        attacker,
        target: target_unit,
        skill_slot: action,
        participation: participation.key(),
    };
    session.core.pending_vrf = true;
    session.acted_seq = session.turn_seq;

    let session_key = session.key();
    let main = participation.core.slot;
    let (party, party_len) = party_keys(session, main);
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

    if let (Some(k), Some(skill)) = (slot, skill.as_ref()) {
        let mut all = [0u8; deadroll_character::state::SKILLS_MAX];
        all[..MONSTER_SLOTS].copy_from_slice(&cooldowns);
        emit!(SkillUsed {
            session: session_key,
            unit: attacker,
            skill_slot: k,
            cooldown: skill.stats.cooldown,
            cooldowns: all,
        });
    }
    emit!(ActionRequested {
        session: session_key,
        action: session.core.action_counter,
        attacker,
        target: target_unit,
        skill_slot: action,
        action_points: state.action_points,
    });
    Ok(())
}

/// A line runs from the monster through its target (docs/SKILL_BUILDER.md 2, 5.3): a wall before
/// the target cuts it. Patch B: between the nearest cells of large units (`line_ends`).
fn check_line(
    session: &GameSession,
    layout: &AccountInfo,
    skill: Option<&MonsterSkillStats>,
    from: Footprint,
    to: impl Into<Footprint>,
) -> Result<()> {
    if let Some(k) = skill.filter(|k| k.stats.blocks.target == Target::Line) {
        let layout = MapLayoutView::load(layout, session.ground())?;
        let (start, through) = line_ends(from, to);
        let line = skill_line(session, &layout, start, through, k.stats.ext.range);
        require!(line.index_of(through).is_some(), SessionError::LineBlocked);
    }
    Ok(())
}
