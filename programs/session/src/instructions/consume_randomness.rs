use anchor_lang::prelude::*;
use deadroll_types::{seeds, SessionStatus};

use crate::{
    combat_input::{effect_action, target_side, Side},
    errors::SessionError,
    loot::{self, CallbackCaches},
    outcomes,
    party::Party,
    reactions::{is_debuff, Cb},
    rules::{Unit, BASE_ATTACK, NATURAL_ATTACK},
    state::{
        external::read_character, pending_kind, GameSession, Monsters, Participation,
        PendingAction, PendingMonsterReactiveData, PendingReactiveData, NO_CHEST,
    },
    turns, vrf,
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
    /// CHECK: that player's `Character`, decoded with owner and discriminator checks. Patch 6: no
    /// longer written here (a death is finalized by `start_turn` or a claim); kept writable so the
    /// account list stays the same.
    #[account(mut, address = participation.core.character @ SessionError::InvalidPendingAccounts)]
    pub character: UncheckedAccount<'info>,
    /// CHECK: the `SkillSet` of the participation's character (recorded by `act` / `monster_act`
    /// from its seeds): the caster's skills. Patch 6: not burned here any more (see `character`).
    #[account(mut)]
    pub skill_set: UncheckedAccount<'info>,
    /// CHECK: the `Bag` of the participation's character (recorded from its seeds). Patch 6:
    /// unused here, kept for the account list.
    #[account(mut)]
    pub bag: UncheckedAccount<'info>,
    /// CHECK: session `["authority"]` PDA, address checked. Patch 6: no CPI from the callback
    /// any more; kept for the account list (with `config` and `character_program`).
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
    /// CHECK: the session's `MapLayout` (patch 4): a resonance's knockback and burning cells
    /// need the walls. Address pinned at `open_session`, read by `MapLayoutView`.
    #[account(address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
}

pub fn handler<'info>(
    ctx: Context<'info, ConsumeRandomness<'info>>,
    randomness: [u8; 32],
) -> Result<()> {
    let remaining = ctx.remaining_accounts;
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

    // Since patch 5 the allies of a buff come with the party accounts: no `target_participation`.
    require!(
        accounts.target_participation.is_none() && session.pending_target == Pubkey::default(),
        SessionError::InvalidPendingAccounts
    );
    let main = &mut accounts.participation;
    let character = read_character(&accounts.character)?;
    // The character of `participation` (the caster, a monster's target, or the first bearer of
    // a reactive-only roll) with what it wears.
    let hero = main.hero(&character);
    let party = Party::load(remaining, session, main.core.slot)?;
    let mut cb = Cb::new(
        session_key,
        randomness,
        session.core.action_counter,
        &monsters,
        &accounts.layout,
        party,
        hero,
    );

    if session.pending_chest != NO_CHEST {
        // Patch 11, `open_chest`: the chest's reward event, nothing else.
        let chest = session.pending_chest;
        session.pending_chest = NO_CHEST;
        let slot = main.core.slot;
        let mut caches = CallbackCaches {
            main,
            party: &mut cb.party,
        };
        loot::open_chest(
            session,
            session_key,
            &monsters,
            chest,
            slot,
            randomness,
            &mut caches,
        )?;
    } else if session.pending_reactive.kind != pending_kind::NONE {
        // `monster_move` / `start_turn`: a roll only for the reactives it triggered.
        require!(
            session.pending_reactive.bearers & (1 << main.core.slot) != 0,
            SessionError::InvalidPendingAccounts
        );
        cb.react_to_pending(session, main)?;
        session.pending_reactive = PendingReactiveData::default();
    } else if session.pending_monster_reactive.kind != pending_kind::NONE {
        // Patch 6, `move_unit` / `start_turn`: a roll only for the monster reactives it
        // triggered; the participation is the one `request_monster_reactive_roll` named.
        cb.react_monsters_to_pending(session, main)?;
        session.pending_monster_reactive = PendingMonsterReactiveData::default();
    } else {
        match (
            Unit::from_id(pending.attacker),
            Unit::from_id(pending.target),
        ) {
            (Some(Unit::Player(slot)), Some(Unit::Monster(index))) => {
                require!(main.core.slot == slot, SessionError::InvalidPendingAccounts);
                let skill = if pending.skill_slot == BASE_ATTACK {
                    None
                } else {
                    Some(
                        main.map_skill(pending.skill_slot)
                            .ok_or(SessionError::InvalidSkill)?,
                    )
                };
                match is_debuff(skill.as_ref()) {
                    Some(effect) => {
                        let skill = skill.as_ref().ok_or(SessionError::InvalidSkill)?;
                        let from = session.player_pos[slot as usize];
                        cb.debuff(
                            session,
                            slot,
                            from,
                            &hero,
                            skill,
                            pending.skill_slot,
                            effect,
                            index,
                        )?;
                        // What the action did to the monsters, then their reactions.
                        let monster_events = cb.take_monster_events();
                        cb.react_monsters(session, main, &monster_events)?;
                    }
                    None => {
                        let moves = cb.strike(
                            session,
                            main,
                            &hero,
                            skill.as_ref(),
                            pending.skill_slot,
                            index,
                        )?;
                        // The players react first, then the monsters, to the action only.
                        let monster_events = cb.take_monster_events();
                        cb.react_to_moves(session, main, &moves)?;
                        cb.take_monster_events();
                        cb.react_monsters(session, main, &monster_events)?;
                    }
                }
            }
            (Some(Unit::Player(slot)), Some(Unit::Player(target))) => {
                require!(main.core.slot == slot, SessionError::InvalidPendingAccounts);
                let skill = main
                    .map_skill(pending.skill_slot)
                    .ok_or(SessionError::InvalidSkill)?;
                require!(
                    target_side(Some(&skill)) == Side::Ally,
                    SessionError::InvalidSkill
                );
                let effect =
                    effect_action(skill.blocks.effect).ok_or(SessionError::InvalidSkill)?;
                // `act` checked the target lives and nothing ran since; a dead one is never
                // raised.
                cb.buff(
                    session,
                    main,
                    slot,
                    &hero,
                    &skill,
                    pending.skill_slot,
                    effect,
                    target,
                )?;
            }
            (Some(Unit::Monster(index)), Some(Unit::Player(slot))) => {
                require!(main.core.slot == slot, SessionError::InvalidPendingAccounts);
                // `monster_act`: the natural attack, or the skill in slot `action - 1`.
                let skill = pending.skill_slot.checked_sub(1);
                debug_assert!(skill.is_some() || pending.skill_slot == NATURAL_ATTACK);
                cb.monster_strikes(session, main, index, skill)?;
            }
            (Some(Unit::Monster(index)), Some(Unit::Monster(target))) => {
                // Patch 6: a heal or buff on an allied monster; `participation` is any living
                // player's (the callback's account list is fixed).
                let slot = pending
                    .skill_slot
                    .checked_sub(1)
                    .ok_or(SessionError::InvalidPendingAccounts)?;
                cb.monster_support(session, index, slot, target)?;
            }
            _ => return err!(SessionError::InvalidPendingAccounts),
        }
    }
    // Patch 11: the kills of this action, and any waiting from `start_turn`, pay out now (the
    // experience and body rewards into the living players' caches), `loot::KILLS_PER_CALLBACK` of
    // them; the rest wait for the next callback or the settlement.
    if session.unpaid_kills != 0 {
        let mut caches = CallbackCaches {
            main,
            party: &mut cb.party,
        };
        loot::pay_kills_within_budget(session, session_key, &monsters, randomness, &mut caches)?;
    }
    cb.party.store()?;

    settle(session, randomness)?;
    outcomes::emit_if_ended(session, session_key)?;
    Ok(())
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
