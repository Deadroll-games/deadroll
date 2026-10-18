use anchor_lang::prelude::*;
use deadroll_character::state::SKILLS_MAX;
use deadroll_combat::{chill_slow_bp, speed_ap};
use deadroll_types::{seeds, SessionStatus};

use crate::{
    cpi_out::{character::DeathAccounts, SessionSigner},
    errors::SessionError,
    events::{ModsTicked, TurnSkipped, TurnTicked},
    outcomes,
    rules::{monster_ap, Unit},
    state::{mods::from_combat, GameSession, Monsters, Participation, MONSTER_SKILLS_MAX},
    turns::{self, TurnStart},
    vrf,
};

#[derive(Accounts)]
pub struct StartTurn<'info> {
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place).
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
    /// Required on a player's turn: the participation whose turn it is.
    #[account(
        mut,
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            participation.core.character.as_ref(),
        ],
        bump = participation.core.bump,
    )]
    pub participation: Option<Box<Account<'info, Participation>>>,
    /// CHECK: required on a player's turn: that participation's `Character` (address checked in
    /// the handler); written by `character::mark_dead` when over-time damage kills.
    #[account(mut)]
    pub character: Option<UncheckedAccount<'info>>,
    /// CHECK: session `["authority"]` PDA, address checked (signs `mark_dead`).
    #[account(address = vrf::SESSION_AUTHORITY)]
    pub session_authority: UncheckedAccount<'info>,
    /// CHECK: registry `GameConfig`, address checked; the character program checks its content.
    #[account(address = vrf::GAME_CONFIG)]
    pub config: UncheckedAccount<'info>,
    /// CHECK: the character program.
    #[account(address = deadroll_character::ID)]
    pub character_program: UncheckedAccount<'info>,
    /// CHECK: required on a player's turn when over-time damage kills: that character's
    /// `SkillSet`, burned by `character::mark_dead` (which checks its seeds).
    #[account(mut)]
    pub skill_set: Option<UncheckedAccount<'info>>,
    /// CHECK: as `skill_set`, the character's `Bag`.
    #[account(mut)]
    pub bag: Option<UncheckedAccount<'info>>,
}

/// The start of the current unit's turn, before anything else on it. Permissionless: it holds no
/// choice, so any participant (the unit's own tab, the DM for monsters) can send it.
/// Over-time damage takes the death path of hits; Freeze / Stun, or a death, end the turn here.
pub fn handler(ctx: Context<StartTurn>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let session_key = session.key();
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    let current = session.current_unit().ok_or(SessionError::NotActive)?;
    require!(!session.turn_started(), SessionError::TurnAlreadyStarted);
    let monsters = Monsters::load(&accounts.monsters, &session_key)?;
    let turn_seq = session.turn_seq;

    let mut cooldowns = [0u8; SKILLS_MAX];
    let (start, states, mods, action_points): (TurnStart, _, _, u8) = match current {
        Unit::Player(slot) => {
            let participation: &mut Participation = accounts
                .participation
                .as_mut()
                .ok_or(SessionError::ParticipationMissing)?;
            require!(participation.core.slot == slot, SessionError::NotYourTurn);
            let character = accounts
                .character
                .as_ref()
                .ok_or(SessionError::InvalidCharacter)?;
            require_keys_eq!(
                character.key(),
                participation.core.character,
                SessionError::InvalidCharacter
            );
            let mut mods = participation.combat_mods();
            let start = turns::start_of_turn(
                &mut participation.core.states,
                &mut mods,
                &mut participation.cooldowns,
                participation.core.hp,
            );
            participation.core.hp = start.hp;
            participation.set_combat_mods(&mods);
            let mut accuracy = deadroll_combat::AccuracyBuff::from(participation.accuracy);
            accuracy.tick();
            participation.accuracy = accuracy.into();
            // After the tick: this turn's AP see the buffs and debuffs that still cover it.
            participation.refresh_ap(turn_seq);
            if start.died {
                let (Some(skill_set), Some(bag)) = (&accounts.skill_set, &accounts.bag) else {
                    return err!(SessionError::DeathAccountsMissing);
                };
                outcomes::player_killed(
                    session,
                    session_key,
                    slot,
                    start.dot_source,
                    &SessionSigner {
                        session_authority: &accounts.session_authority,
                        config: &accounts.config,
                        bump: vrf::SESSION_AUTHORITY_BUMP,
                    },
                    &DeathAccounts {
                        character,
                        skill_set,
                        bag,
                    },
                )?;
            }
            cooldowns = participation.cooldowns;
            (
                start,
                participation.core.states,
                mods,
                participation.core.action_points,
            )
        }
        Unit::Monster(index) => {
            // A player's accounts on a monster's turn: the caller mistook whose turn it is.
            require!(accounts.participation.is_none(), SessionError::NotYourTurn);
            let mut monster = monsters.get(index)?;
            let mut own = monsters.cooldowns(index)?;
            let mut mods = monsters.mods(index)?;
            let start = turns::start_of_turn(&mut monster.states, &mut mods, &mut own, monster.hp);
            monster.hp = start.hp;
            // `advance` granted the base AP; Haste and Slow apply after the tick.
            let chill = chill_slow_bp(&monster.states, &crate::rules::BALANCE.combat);
            monster.action_points = speed_ap(
                monster_ap(session.monster_speed[index as usize]),
                &mods,
                chill,
            );
            if start.died {
                // No VRF value of its own: the reward seed of a boss killed this way is the
                // latest one, already public but out of anyone's hands by now.
                let seed = session.core.last_seed;
                outcomes::monster_killed(
                    session,
                    session_key,
                    &mut monster,
                    index,
                    start.dot_source,
                    seed,
                );
            }
            monsters.set(index, &monster)?;
            monsters.set_cooldowns(index, &own)?;
            monsters.set_mods(index, &mods)?;
            cooldowns[..MONSTER_SKILLS_MAX].copy_from_slice(&own);
            (start, monster.states, mods, monster.action_points)
        }
    };
    session.started_seq = turn_seq;

    let unit = current.id();
    emit!(TurnTicked {
        session: session_key,
        unit,
        turn_seq,
        dot_damage: start.dot_damage,
        hp: start.hp,
        died: start.died,
        skipped: start.skip.is_some(),
        expired: start.expired,
        states,
        cooldowns,
        thaw_damage: start.thaw_damage,
    });
    emit!(ModsTicked {
        session: session_key,
        unit,
        turn_seq,
        expired: start.expired_mods,
        mods: from_combat(&mods),
        action_points,
    });
    if outcomes::emit_if_ended(session, session_key) {
        return Ok(());
    }
    if let Some(state) = start.skip {
        emit!(TurnSkipped {
            session: session_key,
            unit,
            turn_seq,
            state,
        });
    }
    // Hard control or death: the turn ends on its own (the escape intents stay, a skip is not
    // an action).
    if start.died || start.skip.is_some() {
        turns::advance(session, session_key, &monsters)?;
    }
    Ok(())
}
