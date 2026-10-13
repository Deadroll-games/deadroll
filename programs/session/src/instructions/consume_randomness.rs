use anchor_lang::prelude::*;
use deadroll_character::state::SkillRecord;
use deadroll_combat::{
    resolve_hit_into, resolve_rider, resolve_support, Applied as CombatApplied, Balance,
    EffectRoll, HitInput, HitResult, Outcome, States, SupportBalance, TimedMods,
};
use deadroll_skillcheck::{Effect, Purpose};
use deadroll_types::{seeds, CharacterData, SessionStatus};

use crate::{
    combat_input::{
        effect_action, effect_range, heal, land_effect, monster_attacker, monster_defender,
        player_attacker, player_defender, support_input, take_damage, EffectAction,
    },
    cpi_out::SessionSigner,
    errors::SessionError,
    events::EffectResolved,
    hit_event::emit_hit_resolved,
    outcomes,
    rules::{Unit, BASE_ATTACK},
    state::{
        external::{read_character, read_skill},
        mods::from_combat,
        GameSession, Monsters, Participation, PendingAction,
    },
    turns,
    vrf::{self, SyscallRoll},
};

/// The VRF callback (CONTRACTS.md, section 6). Account order is fixed: `vrf::callback_metas`
/// builds the same list for the request, and a unit test checks they match.
#[derive(Accounts)]
pub struct ConsumeRandomness<'info> {
    /// The VRF program's identity for this program (`mock-vrf`: the fixed test identity).
    #[account(address = vrf::identity())]
    pub vrf_program_identity: Signer<'info>,
    /// State checks run here, before the pending-accounts checks below: once a roll is resolved
    /// `pending` is cleared, and a late callback must read `NoRollPending`, not a wrong account.
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
        constraint = session.core.status == SessionStatus::Active @ SessionError::NotActive,
        constraint = session.core.pending_vrf @ SessionError::NoRollPending,
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
    /// The ally of a heal or buff that is not the caster (`session.pending_target`); `None`
    /// for every other roll.
    #[account(mut, address = session.pending_target @ SessionError::InvalidPendingAccounts)]
    pub target_participation: Option<Box<Account<'info, Participation>>>,
}

pub fn handler(ctx: Context<ConsumeRandomness>, randomness: [u8; 32]) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let session_key = session.key();
    let pending = session.pending;
    let monsters = Monsters::load(&accounts.monsters, &session_key)?;

    // `start_session`'s roll: the turn order, nothing else.
    if pending.is_initiative() {
        turns::begin(session, session_key, &monsters, randomness)?;
        settle(session, randomness)?;
        return Ok(());
    }

    // The ally account is there exactly when `act` recorded one.
    let ally = accounts.target_participation.as_mut();
    require!(
        ally.as_ref().map(|a| a.key()).unwrap_or_default() == session.pending_target,
        SessionError::InvalidPendingAccounts
    );
    let participation = &mut accounts.participation;
    let character = read_character(&accounts.character)?;
    let roll = Roll {
        session_key,
        pending,
        seed: randomness,
        action: session.core.action_counter,
    };

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
            match skill
                .as_ref()
                .map(|s| (s.blocks.purpose, effect_action(s.blocks.effect)))
            {
                Some((Purpose::Debuff, Some(effect))) => debuff_monster(
                    &roll,
                    &monsters,
                    index,
                    &character,
                    skill.as_ref().ok_or(SessionError::InvalidSkill)?,
                    effect,
                )?,
                _ => attack_monster(
                    &roll,
                    session,
                    &monsters,
                    index,
                    participation,
                    &character,
                    skill.as_ref(),
                )?,
            }
        }
        (Some(Unit::Player(slot)), Some(Unit::Player(target))) => {
            require!(
                participation.core.slot == slot,
                SessionError::InvalidPendingAccounts
            );
            let skill = read_skill(
                &accounts.skill_set,
                &participation.core.character,
                pending.skill_slot,
            )?;
            let effect = effect_action(skill.blocks.effect).ok_or(SessionError::InvalidSkill)?;
            // A self-cast lands on the caster's own participation, another ally on its account.
            let target_participation: &mut Participation = match ally {
                None if target == slot => participation,
                Some(ally) if target != slot && ally.core.slot == target => ally,
                _ => return err!(SessionError::InvalidPendingAccounts),
            };
            support_ally(&roll, target_participation, &character, &skill, effect)?;
        }
        (Some(Unit::Monster(index)), Some(Unit::Player(slot))) => {
            require!(
                participation.core.slot == slot,
                SessionError::InvalidPendingAccounts
            );
            monster_attack(
                &roll,
                session,
                &monsters,
                index,
                participation,
                &character,
                &SessionSigner {
                    session_authority: &accounts.session_authority,
                    config: &accounts.config,
                    bump: vrf::SESSION_AUTHORITY_BUMP,
                },
                &accounts.character,
            )?;
        }
        _ => return err!(SessionError::InvalidPendingAccounts),
    }

    settle(session, randomness)?;
    outcomes::emit_if_ended(session, session_key);
    Ok(())
}

/// The action being resolved and its VRF value.
struct Roll {
    session_key: Pubkey,
    pending: PendingAction,
    seed: [u8; 32],
    /// `GameSession.action_counter` of this action (before `settle` counts it).
    action: u32,
}

impl Roll {
    #[allow(clippy::too_many_arguments)]
    fn emit_effect(
        &self,
        rider: bool,
        effect: Effect,
        roll: &EffectRoll,
        applied: CombatApplied,
        hp: u32,
        states: &States,
        mods: &TimedMods,
    ) {
        emit!(EffectResolved {
            session: self.session_key,
            caster: self.pending.attacker,
            target: self.pending.target,
            seed: self.seed,
            action_counter: self.action,
            skill_slot: self.pending.skill_slot,
            effect,
            rider,
            roll: roll.into(),
            applied: applied.into(),
            hp,
            states: *states,
            mods: from_combat(mods),
        });
    }
}

/// A player's weapon or damage skill on a monster, then that skill's effect (if any) when the
/// hit landed and the monster lives.
#[inline(never)]
fn attack_monster(
    roll: &Roll,
    session: &mut GameSession,
    monsters: &Monsters,
    index: u8,
    participation: &Participation,
    character: &CharacterData,
    skill: Option<&SkillRecord>,
) -> Result<()> {
    // Off the 4 KB stack frame: `HitResult` alone is over 1 KB.
    let mut input = Box::<HitInput>::default();
    let mut result = Box::<HitResult>::default();
    let mut monster = monsters.get(index)?;
    let mut mods = monsters.mods(index)?;
    input.attacker = player_attacker(
        roll.pending.attacker,
        character,
        skill,
        &participation.combat_mods(),
    );
    input.defender = monster_defender(&monster, &mods).ok_or(SessionError::InvalidMonster)?;
    resolve_hit_into(
        &input,
        &Balance::DEFAULT,
        &mut SyscallRoll::new(roll.seed),
        &mut result,
    );

    let (hp, died) = take_damage(monster.hp, result.damage);
    monster.hp = hp;
    monster.states = result.states;
    if died {
        outcomes::monster_killed(
            session,
            roll.session_key,
            &mut monster,
            index,
            roll.pending.attacker,
            roll.seed,
        );
    }
    emit_hit_resolved(
        &roll.session_key,
        roll.pending.attacker,
        roll.pending.target,
        &roll.seed,
        roll.action,
        &result,
    )?;

    // The skill's effect: its own chance on a hit (certain on a crit), on the next dice.
    if let Some(skill) = skill {
        if let Some(effect) = effect_action(skill.blocks.effect) {
            if result.outcome == Outcome::Hit && monster.alive {
                let (min, max) = effect_range(character.class, skill.power_bp, effect);
                let rider = resolve_rider(
                    SupportBalance::DEFAULT.rider_chance_bp,
                    result.crit,
                    min,
                    max,
                    &Balance::DEFAULT,
                    &mut SyscallRoll::at(roll.seed, result.rolls_used),
                );
                let applied = if rider.outcome == Outcome::Hit {
                    land_effect(
                        effect,
                        rider.amount,
                        skill.blocks.aspects[0],
                        roll.pending.attacker,
                        &mut monster.states,
                        &mut mods,
                    )
                } else {
                    CombatApplied::None
                };
                monsters.set_mods(index, &mods)?;
                roll.emit_effect(
                    true,
                    skill.blocks.effect,
                    &rider,
                    applied,
                    monster.hp,
                    &monster.states,
                    &mods,
                );
            }
        }
    }
    monsters.set(index, &monster)
}

/// A debuff skill on a monster: it rolls to land (d20 against the monster's defense).
#[inline(never)]
fn debuff_monster(
    roll: &Roll,
    monsters: &Monsters,
    index: u8,
    character: &CharacterData,
    skill: &SkillRecord,
    effect: EffectAction,
) -> Result<()> {
    let mut monster = monsters.get(index)?;
    let mut mods = monsters.mods(index)?;
    let defense = monster_defender(&monster, &mods)
        .ok_or(SessionError::InvalidMonster)?
        .defense;
    let input = support_input(character, skill, effect, Some(defense));
    let result = resolve_support(&input, &Balance::DEFAULT, &mut SyscallRoll::new(roll.seed));
    let applied = if result.outcome == Outcome::Hit && monster.alive {
        land_effect(
            effect,
            result.amount,
            skill.blocks.aspects[0],
            roll.pending.attacker,
            &mut monster.states,
            &mut mods,
        )
    } else {
        CombatApplied::None
    };
    monsters.set(index, &monster)?;
    monsters.set_mods(index, &mods)?;
    roll.emit_effect(
        false,
        skill.blocks.effect,
        &result,
        applied,
        monster.hp,
        &monster.states,
        &mods,
    );
    Ok(())
}

/// A heal or buff on the caster or an ally: no roll to land, a crit roll and the d100 strength.
#[inline(never)]
fn support_ally(
    roll: &Roll,
    target: &mut Participation,
    character: &CharacterData,
    skill: &SkillRecord,
    effect: EffectAction,
) -> Result<()> {
    let input = support_input(character, skill, effect, None);
    let result = resolve_support(&input, &Balance::DEFAULT, &mut SyscallRoll::new(roll.seed));
    let mut mods = target.combat_mods();
    // `act` checked the target lives and nothing ran since; a dead one is never raised.
    let applied = if target.core.hp == 0 {
        CombatApplied::None
    } else if effect == EffectAction::Heal {
        target.core.hp = heal(target.core.hp, target.hp_max, result.amount);
        CombatApplied::None
    } else {
        land_effect(
            effect,
            result.amount,
            skill.blocks.aspects[0],
            roll.pending.attacker,
            &mut target.core.states,
            &mut mods,
        )
    };
    target.set_combat_mods(&mods);
    roll.emit_effect(
        false,
        skill.blocks.effect,
        &result,
        applied,
        target.core.hp,
        &target.core.states,
        &mods,
    );
    Ok(())
}

/// A monster's natural attack on a player.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn monster_attack<'info>(
    roll: &Roll,
    session: &mut GameSession,
    monsters: &Monsters,
    index: u8,
    participation: &mut Participation,
    character: &CharacterData,
    signer: &SessionSigner<'_, 'info>,
    character_info: &AccountInfo<'info>,
) -> Result<()> {
    let mut input = Box::<HitInput>::default();
    let mut result = Box::<HitResult>::default();
    let slot = participation.core.slot;
    let monster = monsters.get(index)?;
    input.attacker = monster_attacker(roll.pending.attacker, &monster, &monsters.mods(index)?)
        .ok_or(SessionError::InvalidMonster)?;
    input.defender = player_defender(
        character,
        &participation.core.states,
        &participation.combat_mods(),
    );
    resolve_hit_into(
        &input,
        &Balance::DEFAULT,
        &mut SyscallRoll::new(roll.seed),
        &mut result,
    );

    let (hp, died) = take_damage(participation.core.hp, result.damage);
    participation.core.hp = hp;
    participation.core.states = result.states;
    if died {
        outcomes::player_killed(
            session,
            roll.session_key,
            slot,
            roll.pending.attacker,
            signer,
            character_info,
        )?;
    }
    emit_hit_resolved(
        &roll.session_key,
        roll.pending.attacker,
        roll.pending.target,
        &roll.seed,
        roll.action,
        &result,
    )
}

/// Clears the pending roll and counts the action; returns its number.
fn settle(session: &mut GameSession, randomness: [u8; 32]) -> Result<u32> {
    let action = session.core.action_counter;
    session.core.pending_vrf = false;
    session.pending = PendingAction::default();
    session.pending_target = Pubkey::default();
    session.core.last_seed = randomness;
    session.core.action_counter = action.checked_add(1).ok_or(SessionError::Overflow)?;
    Ok(action)
}
