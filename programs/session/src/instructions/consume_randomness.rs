use anchor_lang::prelude::*;
use deadroll_combat::{resolve_hit_into, Balance, HitInput, HitResult};
use deadroll_types::{seeds, SessionStatus};

use crate::{
    combat_input::{
        monster_attacker, monster_defender, player_attacker, player_defender, take_damage,
    },
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::{HitResolved, MonsterKilled, PlayerDied, SessionEnded},
    rules::{is_boss, monster_profile, Unit, BASE_ATTACK},
    state::{
        external::{read_character, read_skill},
        GameSession, Monsters, Participation, PendingAction,
    },
    vrf::{self, SyscallRoll},
};

/// The VRF callback (CONTRACTS.md, section 6). Account order is fixed: `vrf::callback_metas`
/// builds the same list for the request, and a unit test checks they match.
#[derive(Accounts)]
pub struct ConsumeRandomness<'info> {
    /// The VRF program's identity for this program (`mock-vrf`: the fixed test identity).
    #[account(address = vrf::identity())]
    pub vrf_program_identity: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place).
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
    /// The player side of the hit, the one recorded by `act` / `monster_act`.
    #[account(mut, address = session.pending.participation @ SessionError::InvalidPendingAccounts)]
    pub participation: Box<Account<'info, Participation>>,
    /// CHECK: that player's `Character`, decoded with owner and discriminator checks; written by
    /// `character::mark_dead`.
    #[account(mut, address = participation.core.character @ SessionError::InvalidPendingAccounts)]
    pub character: UncheckedAccount<'info>,
    /// CHECK: the attacking player's `SkillSet`, checked by `read_skill` (unused for monsters).
    pub skill_set: UncheckedAccount<'info>,
    /// CHECK: session `["authority"]` PDA, address checked (signs `mark_dead`).
    #[account(address = vrf::SESSION_AUTHORITY)]
    pub session_authority: UncheckedAccount<'info>,
    /// CHECK: registry `GameConfig`, address checked; the character program checks its content.
    #[account(address = vrf::GAME_CONFIG)]
    pub config: UncheckedAccount<'info>,
    /// CHECK: the character program.
    #[account(address = deadroll_character::ID)]
    pub character_program: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<ConsumeRandomness>, randomness: [u8; 32]) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let session_key = session.key();
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(session.core.pending_vrf, SessionError::NoRollPending);
    let pending = session.pending;
    let monsters = Monsters::load(&accounts.monsters, &session_key)?;
    let participation = &mut accounts.participation;
    let character = read_character(&accounts.character)?;

    // Off the 4 KB stack frame: `HitResult` alone is over 1 KB.
    let mut input = Box::<HitInput>::default();
    let mut result = Box::<HitResult>::default();
    let target_hp;
    let mut killed = false;

    match (
        Unit::from_id(pending.attacker),
        Unit::from_id(pending.target),
    ) {
        (Some(Unit::Player(slot)), Some(Unit::Monster(index))) => {
            require!(
                participation.core.slot == slot,
                SessionError::InvalidPendingAccounts
            );
            let skill = if pending.skill_slot == BASE_ATTACK {
                None
            } else {
                Some(read_skill(
                    &accounts.skill_set,
                    &participation.core.character,
                    pending.skill_slot,
                )?)
            };
            let mut monster = monsters.get(index)?;
            input.attacker = player_attacker(pending.attacker, &character, skill.as_ref());
            input.defender = monster_defender(&monster).ok_or(SessionError::InvalidMonster)?;
            resolve_hit_into(
                &input,
                &Balance::DEFAULT,
                &mut SyscallRoll::new(randomness),
                &mut result,
            );

            let (hp, died) = take_damage(monster.hp, result.damage);
            monster.hp = hp;
            monster.states = result.states;
            if died {
                monster.alive = false;
                killed = true;
            }
            monsters.set(index, &monster)?;
            target_hp = hp;

            if died {
                let xp = monster_profile(monster.bestiary_id, monster.level).map_or(0, |p| p.xp);
                session.xp_pool = session.xp_pool.saturating_add(xp);
                let boss = is_boss(monster.bestiary_id);
                emit!(MonsterKilled {
                    session: session_key,
                    monster: index,
                    bestiary_id: monster.bestiary_id,
                    by: pending.attacker,
                    xp,
                    boss,
                });
                if boss {
                    session.core.status = SessionStatus::Victory;
                    session.victory_seed = randomness;
                }
            }
        }
        (Some(Unit::Monster(index)), Some(Unit::Player(slot))) => {
            require!(
                participation.core.slot == slot,
                SessionError::InvalidPendingAccounts
            );
            let monster = monsters.get(index)?;
            input.attacker =
                monster_attacker(pending.attacker, &monster).ok_or(SessionError::InvalidMonster)?;
            input.defender = player_defender(&character, &participation.core.states);
            resolve_hit_into(
                &input,
                &Balance::DEFAULT,
                &mut SyscallRoll::new(randomness),
                &mut result,
            );

            let (hp, died) = take_damage(participation.core.hp, result.damage);
            participation.core.hp = hp;
            participation.core.states = result.states;
            target_hp = hp;
            if died {
                killed = true;
                session.alive_mask &= !(1 << slot);
                cpi_out::character::mark_dead(
                    &SessionSigner {
                        session_authority: &accounts.session_authority,
                        config: &accounts.config,
                        bump: vrf::SESSION_AUTHORITY_BUMP,
                    },
                    &accounts.character,
                )?;
                emit!(PlayerDied {
                    session: session_key,
                    character: participation.core.character,
                    slot,
                    by: pending.attacker,
                });
                if session.alive_mask == 0 {
                    session.core.status = SessionStatus::Wiped;
                }
            }
        }
        _ => return err!(SessionError::InvalidPendingAccounts),
    }

    let action = session.core.action_counter;
    session.core.pending_vrf = false;
    session.pending = PendingAction::default();
    session.core.last_seed = randomness;
    session.core.action_counter = action.checked_add(1).ok_or(SessionError::Overflow)?;

    let mut resonance = [0xFF; 3];
    for (out, res) in resonance.iter_mut().zip(result.resonances.iter()) {
        if let Some(r) = res.filter(|r| r.active) {
            *out = r.id;
        }
    }
    emit!(HitResolved {
        session: session_key,
        action,
        attacker: pending.attacker,
        target: pending.target,
        skill_slot: pending.skill_slot,
        seed: randomness,
        outcome: result.outcome as u8,
        d20: result.d20,
        crit: result.crit,
        d100: result.d100,
        damage: result.damage,
        resonance,
        states: result.states,
        target_hp,
        killed,
        breakdown: borsh::to_vec(&*result)?,
    });
    if session.core.status != SessionStatus::Active {
        emit!(SessionEnded {
            session: session_key,
            status: session.core.status,
            round: session.core.round,
        });
    }
    Ok(())
}
