use anchor_lang::prelude::*;
use deadroll_character::state::SkillRecord;
use deadroll_combat::{
    AccuracyBuff, Applied as CombatApplied, EffectRoll, HitInput, HitResult, ResonanceTables,
    States, TimedMods,
};
use deadroll_rules::{
    resolve,
    spread::{spread, Field},
    HeroStats, MonsterDefense,
};
use deadroll_skillcheck::{Effect, Purpose};
use deadroll_types::{seeds, SessionStatus};
use deadroll_types::{Cell, Monster, MONSTERS_MAX, PARTY_MAX};

use crate::{
    combat_input::{effect_action, skill_stats, EffectAction},
    cpi_out::{character::DeathAccounts, SessionSigner},
    errors::SessionError,
    events::{EffectResolved, ResonanceSpread, SpreadHitData},
    hit_event::{emit_hit_resolved, HitItemsData, ItemBonusData},
    outcomes,
    rules::{monster_profile, step_cost, Unit, BALANCE, BASE_ATTACK},
    state::{
        external::{read_character, read_skill, MapLayoutView},
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
    /// CHECK: the `SkillSet` of the participation's character (recorded by `act` / `monster_act`
    /// from its seeds): skills read by `read_skill`; burned by `character::mark_dead`.
    #[account(mut)]
    pub skill_set: UncheckedAccount<'info>,
    /// CHECK: the `Bag` of the participation's character (recorded from its seeds); burned by
    /// `character::mark_dead`.
    #[account(mut)]
    pub bag: UncheckedAccount<'info>,
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
    /// CHECK: the session's `MapLayout` (patch 4): a resonance's knockback and burning cells
    /// need the walls. Address pinned at `open_session`, read by `MapLayoutView`.
    #[account(address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
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
    // The character of `participation` (the caster, or the target of a monster) with what it
    // wears.
    let hero = participation.hero(&character);
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
                    &hero,
                    skill.as_ref().ok_or(SessionError::InvalidSkill)?,
                    effect,
                )?,
                _ => attack_monster(
                    &roll,
                    session,
                    &monsters,
                    index,
                    participation,
                    &hero,
                    skill.as_ref(),
                    &accounts.layout,
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
            support_ally(&roll, target_participation, &hero, &skill, effect)?;
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
                &hero,
                &SessionSigner {
                    session_authority: &accounts.session_authority,
                    config: &accounts.config,
                    bump: vrf::SESSION_AUTHORITY_BUMP,
                },
                &DeathAccounts {
                    character: &accounts.character,
                    skill_set: &accounts.skill_set,
                    bag: &accounts.bag,
                },
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
/// hit landed and the monster lives (`deadroll_rules::resolve::attack_monster`).
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn attack_monster<'info>(
    roll: &Roll,
    session: &mut GameSession,
    monsters: &Monsters,
    index: u8,
    participation: &mut Participation,
    hero: &HeroStats,
    skill: Option<&SkillRecord>,
    layout: &AccountInfo<'info>,
) -> Result<()> {
    // Off the 4 KB stack frame: `HitResult` alone is over 1 KB, the tables about 1 KB.
    let mut input = Box::<HitInput>::default();
    let mut result = Box::<HitResult>::default();
    let mut tables = Box::<ResonanceTables>::default();
    monsters.tables_into(&mut tables)?;
    let mut monster = monsters.get(index)?;
    let mut mods = monsters.mods(index)?;
    let defense = monsters.defense(index)?;
    let profile = monster_profile(monster.bestiary_id, monster.level, &defense)
        .ok_or(SessionError::InvalidMonster)?;
    let skill = skill.map(skill_stats);
    let mut accuracy = AccuracyBuff::from(participation.accuracy);
    // One stream for the action: the skill's effect reads the indices after the hit's
    // `rolls_used`, the same as `SyscallRoll::at(seed, rolls_used)`.
    let outcome = resolve::attack_monster(
        BALANCE,
        &tables,
        roll.pending.attacker,
        hero,
        skill.as_ref(),
        &participation.combat_mods(),
        &mut accuracy,
        &profile,
        &mut monster,
        &mut mods,
        &mut input,
        &mut result,
        &mut SyscallRoll::new(roll.seed),
    );
    participation.accuracy = accuracy.into();
    if outcome.died {
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
        &HitItemsData {
            attacker: ItemBonusData::of(hero),
            defender: ItemBonusData::default(),
        },
    )?;

    // A resonance's timed modifiers landed on `mods` in the hit, a rider's after it.
    monsters.set_mods(index, &mods)?;
    if let (Some(rider), Some(skill)) = (outcome.rider, skill) {
        roll.emit_effect(
            true,
            skill.blocks.effect,
            &rider.roll,
            rider.applied,
            monster.hp,
            &monster.states,
            &mods,
        );
    }
    monsters.set(index, &monster)?;
    let attacker = Cell {
        x: participation.core.x,
        y: participation.core.y,
    };
    spread_hit(
        roll, session, monsters, index, attacker, &result, layout, &accuracy,
    )
}

/// The map as a resonance's spread sees it: the layout's walls (the registry tile costs) and
/// every living unit before the spread.
struct SessionField<'a> {
    width: u8,
    height: u8,
    layout: &'a MapLayoutView<'a>,
    tile_cost: &'a [u8; deadroll_types::Tile::COUNT],
    units: [Cell; PARTY_MAX + MONSTERS_MAX],
    unit_count: usize,
}

impl Field for SessionField<'_> {
    fn passable(&self, c: Cell) -> bool {
        c.x < self.width
            && c.y < self.height
            && self
                .layout
                .tile(c.x, c.y)
                .is_ok_and(|t| step_cost(t, self.tile_cost).is_some())
    }

    fn occupied(&self, c: Cell) -> bool {
        self.units[..self.unit_count].contains(&c)
    }
}

/// Plays the jumps, knockback and burning cells of a player's hit on monster `target` and
/// writes `ResonanceSpread` (`deadroll_rules::spread`, the simulator's code too).
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn spread_hit<'info>(
    roll: &Roll,
    session: &GameSession,
    monsters: &Monsters,
    target: u8,
    attacker: Cell,
    result: &HitResult,
    layout: &AccountInfo<'info>,
    accuracy: &AccuracyBuff,
) -> Result<()> {
    if result.jump_count == 0 && result.knockback == 0 && result.ignite_power == 0 {
        if !result.accuracy_buff.is_empty() {
            emit_spread(
                roll,
                &Default::default(),
                &[],
                accuracy,
                0,
                session.core.round,
            );
        }
        return Ok(());
    }
    let count = monsters.count()?;
    let mut all = Box::new([Monster::default(); MONSTERS_MAX]);
    for (i, m) in all.iter_mut().enumerate().take(count as usize) {
        *m = monsters.get(i as u8)?;
    }
    let defense: [MonsterDefense; MONSTERS_MAX] = monsters.defenses()?;
    let layout = MapLayoutView::load(layout)?;
    let mut field = SessionField {
        width: session.width,
        height: session.height,
        layout: &layout,
        tile_cost: &session.tile_cost,
        units: [Cell::default(); PARTY_MAX + MONSTERS_MAX],
        unit_count: 0,
    };
    for slot in 0..session.core.player_count {
        if session.player_alive(slot) {
            field.units[field.unit_count] = session.player_pos[slot as usize];
            field.unit_count += 1;
        }
    }
    for m in all.iter().take(count as usize).filter(|m| m.alive) {
        field.units[field.unit_count] = Cell { x: m.x, y: m.y };
        field.unit_count += 1;
    }
    let mut cells = monsters.burning_cells()?;
    let out = spread(
        BALANCE,
        result,
        attacker,
        target as usize,
        &mut all[..count as usize],
        &defense[..count as usize],
        &field,
        &mut cells,
        session.core.round,
        roll.pending.attacker,
    );
    let mut hits = [SpreadHitData::default(); 8];
    for (h, data) in out.hits().iter().zip(hits.iter_mut()) {
        monsters.set(h.monster, &all[h.monster as usize])?;
        *data = SpreadHitData {
            monster: h.monster,
            kind: h.kind,
            applied: h.applied.into(),
            states: all[h.monster as usize].states,
        };
    }
    if out.moved.is_some() {
        monsters.set(target, &all[target as usize])?;
    }
    if out.ignited_count > 0 {
        monsters.set_burning_cells(&cells)?;
    }
    let until = session
        .core
        .round
        .saturating_add(BALANCE.combat.burning_cell_rounds as u16);
    emit_spread(
        roll,
        &out,
        &hits[..out.hit_count as usize],
        accuracy,
        result.ignite_power,
        until,
    );
    Ok(())
}

fn emit_spread(
    roll: &Roll,
    out: &deadroll_rules::spread::SpreadOutcome,
    hits: &[SpreadHitData],
    accuracy: &AccuracyBuff,
    ignite_power: u32,
    until_round: u16,
) {
    let mut all_hits = [SpreadHitData::default(); 8];
    all_hits[..hits.len()].copy_from_slice(hits);
    let (moved_from, moved_to) = out.moved.unwrap_or_default();
    let mut ignited = [Cell::default(); 8];
    ignited[..out.ignited().len()].copy_from_slice(out.ignited());
    emit!(ResonanceSpread {
        session: roll.session_key,
        action_counter: roll.action,
        attacker: roll.pending.attacker,
        target: roll.pending.target,
        hits: all_hits,
        hit_count: hits.len() as u8,
        moved: out.moved.is_some(),
        moved_from,
        moved_to,
        ignited,
        ignited_count: out.ignited_count,
        ignite_power,
        until_round,
        accuracy: (*accuracy).into(),
    });
}

/// A debuff skill on a monster: it rolls to land (d20 against the monster's defense).
#[inline(never)]
fn debuff_monster(
    roll: &Roll,
    monsters: &Monsters,
    index: u8,
    hero: &HeroStats,
    skill: &SkillRecord,
    effect: EffectAction,
) -> Result<()> {
    let mut monster = monsters.get(index)?;
    let mut mods = monsters.mods(index)?;
    let profile = monster_profile(
        monster.bestiary_id,
        monster.level,
        &monsters.defense(index)?,
    )
    .ok_or(SessionError::InvalidMonster)?;
    let out = resolve::debuff_monster(
        BALANCE,
        roll.pending.attacker,
        hero,
        &skill_stats(skill),
        effect,
        &profile,
        &mut monster,
        &mut mods,
        &mut SyscallRoll::new(roll.seed),
    );
    monsters.set(index, &monster)?;
    monsters.set_mods(index, &mods)?;
    roll.emit_effect(
        false,
        skill.blocks.effect,
        &out.roll,
        out.applied,
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
    hero: &HeroStats,
    skill: &SkillRecord,
    effect: EffectAction,
) -> Result<()> {
    let mut mods = target.combat_mods();
    let hp_max = target.hp_max;
    // A player's unit id is its slot: the caster buffing itself.
    let on_self = target.core.slot == roll.pending.attacker;
    // `act` checked the target lives and nothing ran since; a dead one is never raised.
    let out = resolve::support_ally(
        BALANCE,
        roll.pending.attacker,
        hero,
        &skill_stats(skill),
        effect,
        on_self,
        &mut target.core.hp,
        hp_max,
        &mut target.core.states,
        &mut mods,
        &mut SyscallRoll::new(roll.seed),
    );
    target.set_combat_mods(&mods);
    roll.emit_effect(
        false,
        skill.blocks.effect,
        &out.roll,
        out.applied,
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
    hero: &HeroStats,
    signer: &SessionSigner<'_, 'info>,
    death: &DeathAccounts<'_, 'info>,
) -> Result<()> {
    let mut input = Box::<HitInput>::default();
    let mut result = Box::<HitResult>::default();
    let mut tables = Box::<ResonanceTables>::default();
    monsters.tables_into(&mut tables)?;
    let slot = participation.core.slot;
    let monster = monsters.get(index)?;
    let profile = monster_profile(
        monster.bestiary_id,
        monster.level,
        &monsters.defense(index)?,
    )
    .ok_or(SessionError::InvalidMonster)?;
    let hero_mods = participation.combat_mods();
    let died = resolve::monster_attack(
        BALANCE,
        &tables,
        roll.pending.attacker,
        &profile,
        &monsters.mods(index)?,
        hero,
        &mut participation.core.hp,
        &mut participation.core.states,
        &hero_mods,
        &mut input,
        &mut result,
        &mut SyscallRoll::new(roll.seed),
    );
    if died {
        outcomes::player_killed(
            session,
            roll.session_key,
            slot,
            roll.pending.attacker,
            signer,
            death,
        )?;
    }
    emit_hit_resolved(
        &roll.session_key,
        roll.pending.attacker,
        roll.pending.target,
        &roll.seed,
        roll.action,
        &result,
        &HitItemsData {
            attacker: ItemBonusData::default(),
            defender: ItemBonusData::of(hero),
        },
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
