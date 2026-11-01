use anchor_lang::prelude::*;
use deadroll_character::state::SKILLS_MAX;
use deadroll_combat::{chill_slow_bp, speed_ap};
use deadroll_rules::{
    aura::{tick_auras, AURAS_PER_PLAYER},
    monster::record,
    reactive::{nearby, Event},
};
use deadroll_types::{seeds, SessionStatus, MONSTER_SLOTS};

use crate::{
    cpi_out::{character::DeathAccounts, SessionSigner},
    errors::SessionError,
    events::{AuraChanged, ModsTicked, MonsterAuraChanged, TurnSkipped, TurnTicked},
    monster_cb::{monster_reaction, request_monster_reactive_roll, Field, MonsterEvent},
    outcomes,
    reactions::{fires_for, request_reactive_roll},
    rules::{monster_ap, Unit, BALANCE},
    state::{
        mods::from_combat, pending_kind, GameSession, Monsters, Participation,
        PendingMonsterReactiveData, PendingReactiveData,
    },
    turns::{self, TurnStart},
    vrf::{self, RequestAccounts},
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
    // Patch 5: a roll for the reactive skills over-time damage triggers (HP below a threshold).
    /// Any participant: pays the VRF fee when the turn start triggers a reactive skill.
    #[account(mut)]
    pub payer: Signer<'info>,
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
}

/// The start of the current unit's turn, before anything else on it. Permissionless: it holds no
/// choice, so any participant (the unit's own tab, the DM for monsters) can send it.
/// Over-time damage takes the death path of hits; Freeze / Stun, or a death, end the turn here.
pub fn handler<'info>(ctx: Context<'info, StartTurn<'info>>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let session_key = session.key();
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    // Patch C: the DM's reveals come before the next turn (docs/MAP_DESCRIPTION.md 8.5).
    session.require_no_reveals()?;
    // Patch 6: the players a callback put down are finalized before anything else
    // (`character.mark_dead`, their accounts in `remaining_accounts`).
    if session.pending_deaths != 0 {
        let signer = SessionSigner {
            session_authority: &accounts.session_authority,
            config: &accounts.config,
            bump: vrf::SESSION_AUTHORITY_BUMP,
        };
        outcomes::finalize_deaths(session, session_key, &signer, ctx.remaining_accounts)?;
        // Arena update: between the map and an arena no turn starts (`leave_arena` waits for the
        // deaths of the last action).
        if session.turn_started() || !session.in_play() {
            // The deaths came from the callback of this turn's action: the turn goes on, but a
            // player who went down in its own action has no turn left.
            if let Some(Unit::Player(slot)) = session.current_unit() {
                if !session.player_alive(slot) {
                    let monsters = Monsters::load(&accounts.monsters, &session_key)?;
                    turns::advance(session, session_key, &monsters)?;
                }
            }
            return Ok(());
        }
    } else {
        require!(
            ctx.remaining_accounts.is_empty(),
            SessionError::InvalidDeathAccounts
        );
    }
    session.require_in_play()?;
    let current = session.current_unit().ok_or(SessionError::NotActive)?;
    require!(!session.turn_started(), SessionError::TurnAlreadyStarted);
    let monsters = Monsters::load(&accounts.monsters, &session_key)?;
    let turn_seq = session.turn_seq;

    let mut cooldowns = [0u8; SKILLS_MAX];
    // Over-time damage that dropped a player's HP: the players whose reactive answers it, and
    // the drop (before, after, maximum).
    let mut reactive_bearers = 0u8;
    let mut dot_drop = (0u32, 0u32, 0u32);
    // Patch 6: the same for a monster's HP: the monsters whose reactive answers it.
    let mut monster_bearers = 0u32;
    // Patch B: the unit's buffs and debuffs with the auras over it, after the tick.
    let delay_mods;
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
            let hp_before = participation.core.hp;
            let start = turns::start_of_turn_with(
                &mut participation.core.states,
                &mut mods,
                &mut participation.cooldowns,
                participation.core.hp,
                &crate::rules::BALANCE.combat,
            );
            participation.core.hp = start.hp;
            participation.set_combat_mods(&mods);
            let mut accuracy = deadroll_combat::AccuracyBuff::from(participation.accuracy);
            accuracy.tick();
            participation.accuracy = accuracy.into();
            // The reactive skill's cooldown and the player's temporary aura count down with its
            // own turn (docs/SKILL_BUILDER.md 3.2, 3.5).
            let reactive = &mut session.reactives[slot as usize];
            reactive.cooldown_left = reactive.cooldown_left.saturating_sub(1);
            let mut auras = session.player_auras(slot);
            let ended = tick_auras(&mut auras);
            session.set_player_auras(slot, &auras);
            for i in 0..AURAS_PER_PLAYER {
                if ended & (1 << i) != 0 {
                    emit!(AuraChanged {
                        session: session_key,
                        bearer: slot,
                        aura_slot: i as u8,
                        aura: Default::default(),
                    });
                }
            }
            // After the tick: this turn's AP see the buffs, debuffs and auras that still cover it
            // (a monster's debuff aura included).
            let field = Field::load(session, &monsters)?;
            participation.refresh_ap(turn_seq, &field.view());
            // Patch B: the delay of this turn, under what covers it now (PARTY_AND_TURNS 6.2).
            delay_mods = participation.mods_with_auras(&field.view(), false);
            if start.hp < hp_before && !start.died {
                reactive_bearers = dot_bearers(
                    session,
                    &monsters,
                    slot,
                    hp_before,
                    start.hp,
                    participation.hp_max,
                    start.dot_source,
                )?;
                dot_drop = (hp_before, start.hp, participation.hp_max);
            }
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
            let hp_before = monster.hp;
            // Every skill slot's cooldown (the reactive's too) and its temporary aura count down
            // with its own turn (patch 6).
            let mut own = monsters.skill_cooldowns(index)?;
            let mut mods = monsters.mods(index)?;
            let start = turns::start_of_turn_with(
                &mut monster.states,
                &mut mods,
                &mut own,
                monster.hp,
                &crate::rules::BALANCE.combat,
            );
            monster.hp = start.hp;
            let mut auras = monsters.auras(index)?;
            let ended = tick_auras(&mut auras);
            monsters.set_auras(index, &auras)?;
            for i in 0..AURAS_PER_PLAYER {
                if ended & (1 << i) != 0 {
                    emit!(MonsterAuraChanged {
                        session: session_key,
                        bearer: index,
                        aura_slot: i as u8,
                        aura: Default::default(),
                        kin_only: false,
                    });
                }
            }
            // `advance` granted the base AP; Haste and Slow apply after the tick, the auras over
            // the monster too (players' debuff auras, its allies' auras).
            let chill = chill_slow_bp(&monster.states, &crate::rules::BALANCE.combat);
            monsters.set(index, &monster)?;
            let field = Field::load(session, &monsters)?;
            let kin = monsters.record(index)?[record::KIN];
            let view =
                field
                    .view()
                    .monster_mods(&mods, index, monsters.footprint(index)?, kin, false);
            monster.action_points = speed_ap(
                monster_ap(session.monster_speed[index as usize]),
                &view,
                chill,
            );
            delay_mods = view;
            if start.died {
                // No VRF value of its own: the reward seed of a boss killed this way is the
                // latest one, already public but out of anyone's hands by now.
                let seed = session.core.last_seed;
                outcomes::monster_killed(
                    session,
                    session_key,
                    &monsters,
                    &mut monster,
                    index,
                    start.dot_source,
                    seed,
                )?;
            }
            monsters.set(index, &monster)?;
            monsters.set_skill_cooldowns(index, &own)?;
            monsters.set_mods(index, &mods)?;
            cooldowns[..MONSTER_SLOTS].copy_from_slice(&own);
            if start.hp < hp_before && !start.died {
                monster_bearers = monster_dot_bearers(
                    session,
                    &monsters,
                    index,
                    hp_before,
                    start.hp,
                    monster.hp_max,
                    start.dot_source,
                )?;
                dot_drop = (hp_before, start.hp, monster.hp_max);
            }
            (start, monster.states, mods, monster.action_points)
        }
    };
    session.started_seq = turn_seq;
    turns::add_turn_delay(session, current, &delay_mods);

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
    let (stored, resist_mod) = from_combat(&mods);
    emit!(ModsTicked {
        session: session_key,
        unit,
        turn_seq,
        expired: start.expired_mods,
        mods: stored,
        action_points,
        resist_mod,
    });
    if outcomes::emit_if_ended(session, session_key)? {
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
    if reactive_bearers != 0 {
        let (hp_before, hp_after, hp_max) = dot_drop;
        request_reactive_roll(
            session,
            session_key,
            PendingReactiveData {
                kind: pending_kind::HP_DROP,
                bearers: reactive_bearers,
                source: start.dot_source,
                victim: current.id(),
                hp_before,
                hp_after,
                hp_max,
            },
            RequestAccounts {
                payer: &accounts.payer,
                program_identity: &accounts.program_identity,
                oracle_queue: &accounts.oracle_queue,
                system_program: &accounts.system_program,
                slot_hashes: &accounts.slot_hashes,
            },
            ctx.bumps.program_identity,
            accounts.monsters.key(),
        )?;
    } else if monster_bearers != 0 {
        // The roll's participation: the player whose over-time state it was, else the first
        // living one (a counterattack goes to the source).
        let main = match Unit::from_id(start.dot_source) {
            Some(Unit::Player(s)) if session.player_alive(s) => s,
            _ => (0..session.core.player_count)
                .find(|&s| session.player_alive(s))
                .ok_or(SessionError::NotActive)?,
        };
        let (hp_before, hp_after, hp_max) = dot_drop;
        request_monster_reactive_roll(
            session,
            session_key,
            PendingMonsterReactiveData {
                kind: pending_kind::HP_DROP,
                bearers: monster_bearers,
                source: start.dot_source,
                victim: match current {
                    Unit::Monster(i) => i,
                    Unit::Player(_) => 0,
                },
                hp_before,
                hp_after,
                hp_max,
            },
            main,
            RequestAccounts {
                payer: &accounts.payer,
                program_identity: &accounts.program_identity,
                oracle_queue: &accounts.oracle_queue,
                system_program: &accounts.system_program,
                slot_hashes: &accounts.slot_hashes,
            },
            ctx.bumps.program_identity,
            accounts.monsters.key(),
        )?;
    }
    if start.died || start.skip.is_some() {
        turns::advance(session, session_key, &monsters)?;
    }
    Ok(())
}

/// The players whose reactive skill answers over-time damage that dropped player `victim`'s HP
/// from `before` to `after` (of `hp_max`): its own `HpBelow`, its nearby allies' `AllyHpBelow`.
/// `source`: the unit of the strongest over-time state (the enemy a counterattack goes to).
fn dot_bearers(
    session: &GameSession,
    monsters: &Monsters,
    victim: u8,
    before: u32,
    after: u32,
    hp_max: u32,
    source: u8,
) -> Result<u8> {
    let source = match Unit::from_id(source) {
        Some(Unit::Monster(i)) => {
            let m = monsters.get(i)?;
            m.alive.then_some((i, monsters.footprint(i)?))
        }
        Some(Unit::Player(_)) | None => None,
    };
    let at = session.player_pos[victim as usize];
    let mut bearers = 0u8;
    for b in 0..session.core.player_count {
        let event = if b == victim {
            Event::HpDrop {
                before,
                after,
                hp_max,
            }
        } else if nearby(BALANCE, session.player_pos[b as usize], at) {
            Event::AllyHpDrop {
                ally: victim,
                before,
                after,
                hp_max,
            }
        } else {
            continue;
        };
        if fires_for(session, b, &event, source) {
            bearers |= 1 << b;
        }
    }
    Ok(bearers)
}

/// The monsters whose reactive skill answers over-time damage that dropped monster `victim`'s HP
/// from `before` to `after` (of `hp_max`): its own `HpBelow`, its nearby allies' `AllyHpBelow`.
/// `source`: the unit of the strongest over-time state (the player a counterattack goes to).
fn monster_dot_bearers(
    session: &GameSession,
    monsters: &Monsters,
    victim: u8,
    before: u32,
    after: u32,
    hp_max: u32,
    source: u8,
) -> Result<u32> {
    let source = match Unit::from_id(source) {
        Some(Unit::Player(s)) => Some(s),
        Some(Unit::Monster(_)) | None => None,
    };
    let at = monsters.footprint(victim)?;
    let mut bearers = 0u32;
    for b in 0..monsters.count()? {
        let event = if b == victim {
            Event::HpDrop {
                before,
                after,
                hp_max,
            }
        } else {
            let o = monsters.get(b)?;
            if !o.alive || !nearby(BALANCE, monsters.footprint(b)?, at) {
                continue;
            }
            Event::AllyHpDrop {
                ally: victim,
                before,
                after,
                hp_max,
            }
        };
        let e = MonsterEvent {
            bearer: b,
            event,
            source,
        };
        if monster_reaction(session, monsters, &e)?.is_some() {
            bearers |= 1 << b;
        }
    }
    Ok(bearers)
}
