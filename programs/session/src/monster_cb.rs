//! Patch 6: the monster side of one VRF callback (docs/BESTIARY.md 3, docs/SKILL_BUILDER.md
//! 3.2-3.5): a monster's natural attack or skill on the players (one, or every player of a line),
//! its heal or buff on allied monsters, and the monsters' reactive skills. The rules are
//! `deadroll_rules` (`resolve`, `aura`, `reactive`), the code the simulator runs.
//!
//! Order on one roll stream (CLAUDE.md, rule 10): the action's dice, the line targets in line
//! order, the players' reactions in slot order, then the monsters' reactions in index order. What
//! a reaction does triggers no further reactions.
use anchor_lang::prelude::*;
use deadroll_combat::{Applied, EffectRoll, States, TimedMods};
use deadroll_rules::{
    aura::{AuraField, MonsterAuras, PartyAuras, REACTIVE_AURA},
    input::{effect_action, EffectAction},
    line::{targets_on_line, LINE_TARGETS_MAX},
    monster::{kin_reaches, record, REACTIVE_SLOT},
    reactive::{hit_events, nearby, reaction, Event, Reaction},
    resolve, MonsterSkillStats,
};
use deadroll_skillcheck::{Purpose, Target};
use deadroll_types::{Cell, Monster, SessionStatus, MONSTERS_MAX, PARTY_MAX};

use crate::{
    errors::SessionError,
    events::{MonsterAuraChanged, MonsterReactiveTriggered, ReactionKind},
    hit_event::{emit_hit_resolved, HitItemsData, ItemBonusData},
    line::skill_line,
    outcomes,
    reactions::{Cb, ReactEvent, ReactEvents},
    rules::{cooldown_after_use, Unit, BALANCE},
    state::{external::MapLayoutView, pending_kind, GameSession, MonsterBearers, Participation},
};

/// What happened to a monster in this action, for its reactive skill.
#[derive(Clone, Copy, Debug)]
pub struct MonsterEvent {
    pub bearer: u8,
    pub event: Event,
    /// The player that caused it.
    pub source: Option<u8>,
}

/// The monster events of one action.
#[derive(Clone, Copy, Debug)]
pub struct MonsterEvents {
    list: [Option<MonsterEvent>; MONSTERS_MAX * 2],
    len: usize,
}

impl Default for MonsterEvents {
    fn default() -> Self {
        MonsterEvents {
            list: [None; MONSTERS_MAX * 2],
            len: 0,
        }
    }
}

impl MonsterEvents {
    pub fn push(&mut self, e: MonsterEvent) {
        if self.len < self.list.len() {
            self.list[self.len] = Some(e);
            self.len += 1;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn of(&self, bearer: u8) -> impl Iterator<Item = MonsterEvent> + '_ {
        self.list[..self.len]
            .iter()
            .flatten()
            .filter(move |e| e.bearer == bearer)
            .copied()
    }
}

fn cell_of(m: &Monster) -> Cell {
    Cell { x: m.x, y: m.y }
}

/// Whether `effect` landing as `applied` put hard control (Freeze, Stun) on its target.
pub fn controls(effect: EffectAction, applied: Applied) -> bool {
    match effect {
        EffectAction::Stun | EffectAction::Freeze => applied != Applied::None,
        EffectAction::Heal | EffectAction::Mod(_) => false,
    }
}

/// The aura field of the whole session, without borrowing it: the party's auras over copies of
/// its cells, and the monster bearers.
pub struct Field {
    party: PartyAuras,
    cells: [Cell; PARTY_MAX],
    alive: u8,
    bearers: Box<MonsterBearers>,
}

impl Field {
    pub fn load(session: &GameSession, monsters: &crate::state::Monsters) -> Result<Self> {
        Ok(Field {
            party: session.party_auras(),
            cells: session.player_pos,
            alive: session.alive_mask,
            bearers: monsters.bearers()?,
        })
    }

    pub fn view(&self) -> AuraField<'_> {
        AuraField {
            auras: &self.party,
            cells: &self.cells,
            alive: self.alive,
            monsters: self.bearers.field(),
        }
    }
}

impl<'a, 'info> Cb<'a, 'info> {
    /// Bit per monster index with a reactive skill (read once per callback).
    fn reactive_monsters(&mut self) -> Result<u32> {
        if let Some(bits) = self.reactive_monsters {
            return Ok(bits);
        }
        let mut bits = 0u32;
        for i in 0..self.monsters.count()? {
            if self.monsters.skill_slots(i)?[REACTIVE_SLOT] != deadroll_rules::monster::NO_SKILL {
                bits |= 1 << i;
            }
        }
        self.reactive_monsters = Some(bits);
        Ok(bits)
    }

    /// What player `slot`'s last hit (`self.result`) did to monster `index` (HP `before` it), for
    /// the monsters' reactives: hit, crit, HP drop, control, and an HP drop seen by its nearby
    /// allies.
    pub(crate) fn note_monster_hit(&mut self, index: u8, before: u32, slot: u8) -> Result<()> {
        let reactive = self.reactive_monsters()?;
        if reactive == 0 {
            return Ok(());
        }
        let m = self.monsters.get(index)?;
        if reactive & (1 << index) != 0 {
            for event in hit_events(&self.result, before, m.hp, m.hp_max)
                .into_iter()
                .flatten()
            {
                self.monster_events.push(MonsterEvent {
                    bearer: index,
                    event,
                    source: Some(slot),
                });
            }
        }
        if m.hp < before {
            let at = cell_of(&m);
            for b in 0..self.monsters.count()? {
                if b == index || reactive & (1 << b) == 0 {
                    continue;
                }
                let o = self.monsters.get(b)?;
                if o.alive && nearby(BALANCE, cell_of(&o), at) {
                    self.monster_events.push(MonsterEvent {
                        bearer: b,
                        event: Event::AllyHpDrop {
                            ally: index,
                            before,
                            after: m.hp,
                            hp_max: m.hp_max,
                        },
                        source: Some(slot),
                    });
                }
            }
        }
        Ok(())
    }

    /// Player `slot`'s debuff froze or stunned monster `index`.
    pub(crate) fn note_monster_controlled(&mut self, index: u8, slot: u8) -> Result<()> {
        if self.reactive_monsters()? & (1 << index) != 0 {
            self.monster_events.push(MonsterEvent {
                bearer: index,
                event: Event::Controlled,
                source: Some(slot),
            });
        }
        Ok(())
    }

    /// Takes the monster events noted so far (the action's, before any reaction adds more).
    pub(crate) fn take_monster_events(&mut self) -> Box<MonsterEvents> {
        core::mem::take(&mut self.monster_events)
    }

    /// Monster `index`'s action on player `target` (the callback's own participation): its
    /// natural attack (`slot = None`) or the skill in monster slot `slot`, then the players'
    /// reactions to it.
    #[allow(clippy::too_many_arguments)]
    pub fn monster_strikes(
        &mut self,
        session: &mut GameSession,
        main: &mut Participation,
        index: u8,
        slot: Option<u8>,
    ) -> Result<()> {
        let skill = match slot {
            Some(s) => Some(
                self.monsters
                    .skill(index, s)?
                    .ok_or(SessionError::MonsterSkillMissing)?,
            ),
            None => None,
        };
        // `EffectResolved.skill_slot`: the monster slot (a natural attack has no effect).
        let skill_slot = slot.unwrap_or(0);
        let target = main.core.slot;
        let events =
            self.monster_on_players(session, main, index, skill.as_ref(), skill_slot, target)?;
        self.react(session, main, &events)
    }

    /// One monster action on players: a hit (natural attack or damage skill) or a debuff, on
    /// player `target` or every living player of the skill's line through it. Returns what it
    /// did to the players, for their reactives.
    #[allow(clippy::too_many_arguments)]
    fn monster_on_players(
        &mut self,
        session: &mut GameSession,
        main: &mut Participation,
        index: u8,
        skill: Option<&MonsterSkillStats>,
        skill_slot: u8,
        target: u8,
    ) -> Result<ReactEvents> {
        let mut events = ReactEvents::default();
        let monster = self.monsters.get(index)?;
        if !monster.alive || !session.player_alive(target) {
            return Ok(events);
        }
        let from = cell_of(&monster);
        let (targets, n) = match skill.filter(|k| k.stats.blocks.target == Target::Line) {
            Some(k) => {
                let layout = MapLayoutView::load(self.layout)?;
                let to = session.player_pos[target as usize];
                let line = skill_line(session, &layout, from, to, k.stats.ext.range);
                let mut units = [(0u8, Cell::default()); PARTY_MAX];
                let mut m = 0;
                for s in 0..session.core.player_count {
                    if session.player_alive(s) {
                        units[m] = (s, session.player_pos[s as usize]);
                        m += 1;
                    }
                }
                targets_on_line(&line, units[..m].iter().copied())
            }
            None => {
                let mut one = [0u8; LINE_TARGETS_MAX];
                one[0] = target;
                (one, 1)
            }
        };
        let debuff = skill.and_then(|k| match k.stats.blocks.purpose {
            Purpose::Debuff => effect_action(k.stats.blocks.effect),
            Purpose::Damage | Purpose::Buff => None,
        });
        for &slot in &targets[..n] {
            if session.core.status != SessionStatus::Active || !session.player_alive(slot) {
                continue;
            }
            match (debuff, skill) {
                (Some(effect), Some(k)) => self.monster_debuff_one(
                    session,
                    main,
                    index,
                    k,
                    skill_slot,
                    effect,
                    slot,
                    &mut events,
                )?,
                _ => self.monster_hit_one(
                    session,
                    main,
                    index,
                    skill,
                    skill_slot,
                    slot,
                    &mut events,
                )?,
            }
        }
        Ok(events)
    }

    /// Monster `index`'s hit on player `slot`, with the hit's events for the players' reactives.
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn monster_hit_one(
        &mut self,
        session: &mut GameSession,
        main: &mut Participation,
        index: u8,
        skill: Option<&MonsterSkillStats>,
        skill_slot: u8,
        slot: u8,
        events: &mut ReactEvents,
    ) -> Result<()> {
        self.load_tables()?;
        let monster = self.monsters.get(index)?;
        let profile = self.monsters.profile(index, monster.level)?;
        let field = Field::load(session, self.monsters)?;
        let monster_mods = field.view().monster_mods(
            &self.monsters.mods(index)?,
            index,
            cell_of(&monster),
            profile.kin,
            skill.is_none(),
        );
        let hero = self.hero_of(main, slot)?;
        let attacker = Unit::Monster(index).id();
        let (before, after, hp_max, died) = self.on_player(main, slot, |cb, victim| {
            let view = victim.mods_with_auras(&field.view(), false);
            let mut own = victim.combat_mods();
            let before = victim.core.hp;
            let tables = cb
                .tables
                .as_deref()
                .ok_or(SessionError::InvalidMonsterState)?;
            let out = resolve::monster_attack(
                BALANCE,
                tables,
                attacker,
                &profile,
                skill.map(|k| &k.stats),
                &monster_mods,
                &hero,
                &mut victim.core.hp,
                &mut victim.core.states,
                &mut own,
                &view,
                &mut cb.input,
                &mut cb.result,
                &mut cb.src,
            );
            victim.set_combat_mods(&own);
            emit_hit_resolved(
                &cb.session_key,
                attacker,
                Unit::Player(slot).id(),
                &cb.seed,
                cb.action,
                &cb.result,
                &HitItemsData {
                    attacker: ItemBonusData::default(),
                    defender: ItemBonusData::of(&hero),
                },
            )?;
            if let (Some(rider), Some(k)) = (out.rider, skill) {
                cb.emit_effect(
                    attacker,
                    Unit::Player(slot).id(),
                    skill_slot,
                    true,
                    k.stats.blocks.effect,
                    &rider.roll,
                    rider.applied,
                    victim.core.hp,
                    &victim.core.states,
                    &own,
                );
                let controlled = effect_action(k.stats.blocks.effect)
                    .is_some_and(|e| controls(e, rider.applied));
                if controlled {
                    events.push(ReactEvent {
                        bearer: slot,
                        event: Event::Controlled,
                        source: Some(index),
                    });
                }
            }
            Ok((before, victim.core.hp, victim.hp_max, out.died))
        })?;
        if died {
            // Down now (no turn, no target); `start_turn` or a claim finalizes the death.
            outcomes::player_down(session, self.session_key, slot, attacker);
        }
        for e in hit_events(&self.result, before, after, hp_max)
            .into_iter()
            .flatten()
        {
            events.push(ReactEvent {
                bearer: slot,
                event: e,
                source: Some(index),
            });
        }
        if after < before {
            let at = session.player_pos[slot as usize];
            for b in 0..session.core.player_count {
                if b != slot
                    && session.player_alive(b)
                    && nearby(BALANCE, session.player_pos[b as usize], at)
                {
                    events.push(ReactEvent {
                        bearer: b,
                        event: Event::AllyHpDrop {
                            ally: slot,
                            before,
                            after,
                            hp_max,
                        },
                        source: Some(index),
                    });
                }
            }
        }
        Ok(())
    }

    /// Monster `index`'s debuff skill on player `slot`: it rolls to land (`EffectResolved`).
    #[allow(clippy::too_many_arguments)]
    fn monster_debuff_one(
        &mut self,
        session: &GameSession,
        main: &mut Participation,
        index: u8,
        skill: &MonsterSkillStats,
        skill_slot: u8,
        effect: EffectAction,
        slot: u8,
        events: &mut ReactEvents,
    ) -> Result<()> {
        let monster = self.monsters.get(index)?;
        let profile = self.monsters.profile(index, monster.level)?;
        let field = Field::load(session, self.monsters)?;
        let hero = self.hero_of(main, slot)?;
        let caster = Unit::Monster(index).id();
        let applied = self.on_player(main, slot, |cb, victim| {
            let view = victim.mods_with_auras(&field.view(), false);
            let mut own = victim.combat_mods();
            let out = resolve::monster_debuff_player(
                BALANCE,
                caster,
                &profile,
                &skill.stats,
                effect,
                &hero,
                victim.core.hp,
                &mut victim.core.states,
                &mut own,
                &view,
                &mut cb.src,
            );
            victim.set_combat_mods(&own);
            cb.emit_effect(
                caster,
                Unit::Player(slot).id(),
                skill_slot,
                false,
                skill.stats.blocks.effect,
                &out.roll,
                out.applied,
                victim.core.hp,
                &victim.core.states,
                &own,
            );
            Ok(out.applied)
        })?;
        if controls(effect, applied) {
            events.push(ReactEvent {
                bearer: slot,
                event: Event::Controlled,
                source: Some(index),
            });
        }
        Ok(())
    }

    /// Monster `index`'s heal or buff skill in `slot` on monster `target` (every allied monster
    /// of its line for a line, itself left out; a kin-only skill skips other kins).
    pub fn monster_support(
        &mut self,
        session: &GameSession,
        index: u8,
        slot: u8,
        target: u8,
    ) -> Result<()> {
        let skill = self
            .monsters
            .skill(index, slot)?
            .ok_or(SessionError::MonsterSkillMissing)?;
        self.monster_support_with(session, index, &skill, slot, target)
    }

    fn monster_support_with(
        &mut self,
        session: &GameSession,
        index: u8,
        skill: &MonsterSkillStats,
        skill_slot: u8,
        target: u8,
    ) -> Result<()> {
        let effect = effect_action(skill.stats.blocks.effect).ok_or(SessionError::InvalidSkill)?;
        let caster_monster = self.monsters.get(index)?;
        let profile = self.monsters.profile(index, caster_monster.level)?;
        let (targets, n) = if skill.stats.blocks.target == Target::Line && target != index {
            let from = cell_of(&caster_monster);
            self.line_monsters(session, from, target, &skill.stats)?
        } else {
            let mut one = [0u8; LINE_TARGETS_MAX];
            one[0] = target;
            (one, 1)
        };
        let caster = Unit::Monster(index).id();
        for &j in &targets[..n] {
            if j == index && target != index {
                continue;
            }
            let mut m = self.monsters.get(j)?;
            let kin = self.monsters.record(j)?[record::KIN];
            if !m.alive || !kin_reaches(skill.kin_only, profile.kin, kin, j == index) {
                continue;
            }
            let mut mods = self.monsters.mods(j)?;
            let out = resolve::monster_support(
                BALANCE,
                caster,
                &profile,
                &skill.stats,
                effect,
                &mut m,
                &mut mods,
                &mut self.src,
            );
            self.monsters.set(j, &m)?;
            self.monsters.set_mods(j, &mods)?;
            self.emit_effect(
                caster,
                Unit::Monster(j).id(),
                skill_slot,
                false,
                skill.stats.blocks.effect,
                &out.roll,
                out.applied,
                m.hp,
                &m.states,
                &mods,
            );
        }
        Ok(())
    }

    /// Every monster bearer's reaction to `events`, in monster index order, each at most once
    /// per callback.
    pub fn react_monsters(
        &mut self,
        session: &mut GameSession,
        main: &mut Participation,
        events: &MonsterEvents,
    ) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        for b in 0..self.monsters.count()? {
            for e in events.of(b) {
                if session.core.status != SessionStatus::Active {
                    return Ok(());
                }
                if self.react_monster_one(session, main, &e)? {
                    break;
                }
            }
        }
        Ok(())
    }

    #[inline(never)]
    fn react_monster_one(
        &mut self,
        session: &mut GameSession,
        main: &mut Participation,
        e: &MonsterEvent,
    ) -> Result<bool> {
        let b = e.bearer;
        if self.monster_fired & (1 << b) != 0 {
            return Ok(false);
        }
        let Some((skill, r)) = monster_reaction(session, self.monsters, e)? else {
            return Ok(false);
        };
        self.monster_fired |= 1 << b;
        let mut cooldowns = self.monsters.skill_cooldowns(b)?;
        cooldowns[REACTIVE_SLOT] = cooldown_after_use(skill.stats.cooldown);
        self.monsters.set_skill_cooldowns(b, &cooldowns)?;
        let bearer_unit = Unit::Monster(b).id();
        let (kind, target) = match r {
            Reaction::Strike { monster } => (ReactionKind::Strike, Unit::Player(monster).id()),
            Reaction::Debuff { monster } => (ReactionKind::Debuff, Unit::Player(monster).id()),
            Reaction::Buff { slot } => (ReactionKind::Buff, Unit::Monster(slot).id()),
            Reaction::Aura => (ReactionKind::Aura, bearer_unit),
        };
        emit!(MonsterReactiveTriggered {
            session: self.session_key,
            action_counter: self.action,
            bearer: b,
            skill_slot: REACTIVE_SLOT as u8,
            trigger: skill.stats.blocks.trigger,
            reaction: kind,
            target,
            cooldown_left: cooldowns[REACTIVE_SLOT],
        });
        let skill_slot = REACTIVE_SLOT as u8;
        match r {
            // A reaction triggers no further reactions: the players' events are dropped.
            Reaction::Strike { monster: slot } | Reaction::Debuff { monster: slot } => {
                self.monster_on_players(session, main, b, Some(&skill), skill_slot, slot)?;
            }
            Reaction::Buff { slot: ally } => {
                self.monster_support_with(session, b, &skill, skill_slot, ally)?;
            }
            Reaction::Aura => {
                let m = self.monsters.get(b)?;
                let profile = self.monsters.profile(b, m.level)?;
                let (roll, aura) =
                    resolve::monster_start_aura(BALANCE, &profile, &skill, &mut self.src);
                let applied = match aura {
                    Some(a) => {
                        let mut auras: MonsterAuras = self.monsters.auras(b)?;
                        auras[REACTIVE_AURA] = a;
                        self.monsters.set_auras(b, &auras)?;
                        emit!(MonsterAuraChanged {
                            session: self.session_key,
                            bearer: b,
                            aura_slot: REACTIVE_AURA as u8,
                            aura: a.into(),
                            kin_only: a.kin_only,
                        });
                        Applied::Added
                    }
                    None => Applied::None,
                };
                self.emit_monster_effect(bearer_unit, skill_slot, &skill, &roll, applied, &m)?;
            }
        }
        Ok(true)
    }

    fn emit_monster_effect(
        &self,
        unit: u8,
        skill_slot: u8,
        skill: &MonsterSkillStats,
        roll: &EffectRoll,
        applied: Applied,
        m: &Monster,
    ) -> Result<()> {
        let index = unit.saturating_sub(crate::rules::MONSTER_UNIT_BASE);
        let mods: TimedMods = self.monsters.mods(index)?;
        let states: States = m.states;
        self.emit_effect(
            unit,
            unit,
            skill_slot,
            false,
            skill.stats.blocks.effect,
            roll,
            applied,
            m.hp,
            &states,
            &mods,
        );
        Ok(())
    }

    /// The monster reactions of a roll requested only for them (`move_unit`, `start_turn`).
    pub fn react_monsters_to_pending(
        &mut self,
        session: &mut GameSession,
        main: &mut Participation,
    ) -> Result<()> {
        let p = session.pending_monster_reactive;
        let events = pending_events(&p)?;
        self.react_monsters(session, main, &events)
    }
}

/// The events a monster reactive-only roll answers.
fn pending_events(p: &crate::state::PendingMonsterReactiveData) -> Result<Box<MonsterEvents>> {
    let source = match Unit::from_id(p.source) {
        Some(Unit::Player(s)) => Some(s),
        _ => None,
    };
    let mut events = Box::<MonsterEvents>::default();
    for b in 0..MONSTERS_MAX as u8 {
        if p.bearers & (1 << b) == 0 {
            continue;
        }
        let event = match p.kind {
            pending_kind::ENEMY_ENTERED => Event::EnemyEntered,
            pending_kind::HP_DROP if b == p.victim => Event::HpDrop {
                before: p.hp_before,
                after: p.hp_after,
                hp_max: p.hp_max,
            },
            pending_kind::HP_DROP => Event::AllyHpDrop {
                ally: p.victim,
                before: p.hp_before,
                after: p.hp_after,
                hp_max: p.hp_max,
            },
            _ => return err!(SessionError::InvalidPendingAccounts),
        };
        events.push(MonsterEvent {
            bearer: b,
            event,
            source,
        });
    }
    Ok(events)
}

/// The reaction of monster `e.bearer`'s reactive skill to `e`, without acting on it: the same
/// check in the callback and in the instructions that ask before they request a roll
/// (`move_unit`, `start_turn`).
pub fn monster_reaction(
    session: &GameSession,
    monsters: &crate::state::Monsters,
    e: &MonsterEvent,
) -> Result<Option<(MonsterSkillStats, Reaction)>> {
    let source = e
        .source
        .filter(|&s| session.player_alive(s))
        .map(|s| (s, session.player_pos[s as usize]));
    monster_reaction_from(monsters, e, source)
}

/// [`monster_reaction`] with the source player and its cell given (`move_unit` asks before the
/// player's position is written).
pub fn monster_reaction_from(
    monsters: &crate::state::Monsters,
    e: &MonsterEvent,
    source: Option<(u8, Cell)>,
) -> Result<Option<(MonsterSkillStats, Reaction)>> {
    let b = e.bearer;
    let m = monsters.get(b)?;
    if !m.alive {
        return Ok(None);
    }
    let Some(skill) = monsters.skill(b, REACTIVE_SLOT as u8)? else {
        return Ok(None);
    };
    let ally_cell = match e.event {
        Event::AllyHpDrop { ally, .. } => {
            let a = monsters.get(ally)?;
            let (mine, theirs) = (
                monsters.record(b)?[record::KIN],
                monsters.record(ally)?[record::KIN],
            );
            // A kin-only skill answers allies of its kin only.
            if !kin_reaches(skill.kin_only, mine, theirs, false) {
                return Ok(None);
            }
            Some(cell_of(&a))
        }
        Event::Hit { .. } | Event::HpDrop { .. } | Event::Controlled | Event::EnemyEntered => None,
    };
    let cooldown = monsters.skill_cooldowns(b)?[REACTIVE_SLOT];
    Ok(reaction(
        BALANCE,
        &skill.stats,
        cooldown,
        b,
        cell_of(&m),
        &e.event,
        source,
        ally_cell,
    )
    .map(|r| (skill, r)))
}

/// Requests a VRF value only for the monster reactives of `pending` (`move_unit`, `start_turn`):
/// the callback's `participation` is player `main`'s (the mover, or any living player), the other
/// living players come as the party accounts. Not an action: `acted_seq` and the escape intents
/// stay.
#[allow(clippy::too_many_arguments)]
pub fn request_monster_reactive_roll(
    session: &mut GameSession,
    session_key: Pubkey,
    pending: crate::state::PendingMonsterReactiveData,
    main: u8,
    accounts: crate::vrf::RequestAccounts,
    identity_bump: u8,
    monsters: Pubkey,
) -> Result<()> {
    use deadroll_types::seeds;
    require!(
        pending.bearers != 0 && session.player_alive(main),
        SessionError::InvalidPendingAccounts
    );
    let character = session.core.players[main as usize];
    let participation = session.participations[main as usize];
    let pda = |seed: &[u8]| {
        Pubkey::find_program_address(&[seed, character.as_ref()], &deadroll_character::ID).0
    };
    let first = pending.bearers.trailing_zeros() as u8;
    session.pending = crate::state::PendingAction {
        attacker: Unit::Monster(first).id(),
        target: pending.source,
        skill_slot: crate::reactions::REACTIVE_ROLL,
        participation,
    };
    session.pending_target = Pubkey::default();
    session.pending_monster_reactive = pending;
    session.core.pending_vrf = true;
    let (party, party_len) = crate::party::party_keys(session, main);
    crate::vrf::request(
        accounts,
        identity_bump,
        crate::vrf::caller_seed(&session_key, session.core.action_counter),
        &crate::vrf::CallbackAccounts {
            session: session_key,
            monsters,
            participation,
            character,
            skill_set: pda(seeds::SKILLS),
            bag: pda(seeds::BAG),
            target_participation: None,
            layout: session.layout,
            party,
            party_len,
        },
    )?;
    emit!(crate::events::ActionRequested {
        session: session_key,
        action: session.core.action_counter,
        attacker: Unit::Monster(first).id(),
        target: pending.source,
        skill_slot: crate::reactions::REACTIVE_ROLL,
        action_points: 0,
    });
    Ok(())
}
