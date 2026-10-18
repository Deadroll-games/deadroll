//! The work of one VRF callback (`consume_randomness`): the action, then the reactive skills it
//! triggered (docs/SKILL_BUILDER.md, sections 2, 3.2-3.5 and 5.3).
//!
//! One roll stream serves the whole callback (CLAUDE.md, rule 10): the action's dice first, then
//! each line target's in line order, then each reaction's in player slot order. The rules are
//! `deadroll_rules` (`resolve`, `aura`, `reactive`, `line`), the code the simulator runs.
//!
//! - A line hits every unit of the skill's side on it ([`Cb::strike`], [`Cb::debuff`],
//!   [`Cb::buff`]); a unit an earlier hit of the same line killed is skipped.
//! - Auras: every hit sees the attacker's and the target's own modifiers plus the auras over
//!   them; the stored modifiers never take them in.
//! - Reactions: what happened to the players in this action (a monster's hit, an HP drop, hard
//!   control, an enemy pushed next to them) is checked against every living player's reactive
//!   skill, in slot order; each fires at most once per action, and what a reaction does triggers
//!   no further reactions. Patch 6: then the monsters' reactive skills, in index order, answer
//!   what the action did to them (`crate::monster_cb`).
use anchor_lang::prelude::*;
use deadroll_combat::{
    AccuracyBuff, Applied as CombatApplied, EffectRoll, HitInput, HitResult, ResonanceTables,
    States, TimedMods,
};
use deadroll_rules::{
    aura::REACTIVE_AURA,
    input::{effect_action, EffectAction, SkillStats},
    line::{targets_on_line, LINE_TARGETS_MAX},
    reactive::{entered_next_to, reaction, Event, Reaction},
    resolve,
    spread::{spread, Field, SpreadOutcome},
    HeroStats, MonsterDefense,
};
use deadroll_skillcheck::{Purpose, Target};
use deadroll_types::{seeds, Cell, Monster, SessionStatus, MONSTERS_MAX, PARTY_MAX};

use crate::{
    errors::SessionError,
    events::{
        ActionRequested, AuraChanged, EffectResolved, ReactionKind, ReactiveTriggered,
        ResonanceSpread, SpreadHitData,
    },
    hit_event::{emit_hit_resolved, HitItemsData, ItemBonusData},
    line::skill_line,
    monster_cb::{self, MonsterEvents},
    outcomes,
    party::{party_keys, Party},
    rules::{cooldown_after_use, step_cost, Unit, BALANCE},
    state::{
        external::MapLayoutView, mods::from_combat, pending_kind, GameSession, Monsters,
        Participation, PendingAction, PendingReactiveData,
    },
    vrf::{self, SyscallRoll},
};

/// Knockbacks of one action: monster index, from, to.
#[derive(Clone, Copy, Debug, Default)]
pub struct Moves {
    moves: [(u8, Cell, Cell); 8],
    len: usize,
}

impl Moves {
    fn push(&mut self, monster: u8, from: Cell, to: Cell) {
        if self.len < self.moves.len() {
            self.moves[self.len] = (monster, from, to);
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &(u8, Cell, Cell)> {
        self.moves[..self.len].iter()
    }
}

/// What happened to a player in this action, for its reactive skill.
#[derive(Clone, Copy, Debug)]
pub struct ReactEvent {
    pub bearer: u8,
    pub event: Event,
    /// The monster that caused it.
    pub source: Option<u8>,
}

/// Events of one action: at most a few per player.
#[derive(Clone, Copy, Debug)]
pub struct ReactEvents {
    list: [Option<ReactEvent>; PARTY_MAX * 4],
    len: usize,
}

impl Default for ReactEvents {
    fn default() -> Self {
        ReactEvents {
            list: [None; PARTY_MAX * 4],
            len: 0,
        }
    }
}

impl ReactEvents {
    pub fn push(&mut self, e: ReactEvent) {
        if self.len < self.list.len() {
            self.list[self.len] = Some(e);
            self.len += 1;
        }
    }

    /// In player slot order, each player's events in the order they happened.
    fn ordered(&self) -> impl Iterator<Item = ReactEvent> + '_ {
        (0..PARTY_MAX as u8).flat_map(move |slot| {
            self.list[..self.len]
                .iter()
                .flatten()
                .filter(move |e| e.bearer == slot)
                .copied()
        })
    }
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

fn cell_of(m: &Monster) -> Cell {
    Cell { x: m.x, y: m.y }
}

fn cell_of_player(p: &Participation) -> Cell {
    Cell {
        x: p.core.x,
        y: p.core.y,
    }
}

/// One callback's state: the roll stream, the hit buffers (off the 4 KB stack frame), the party.
pub struct Cb<'a, 'info> {
    pub session_key: Pubkey,
    pub seed: [u8; 32],
    /// `GameSession.action_counter` of this action.
    pub action: u32,
    pub(crate) monsters: &'a Monsters<'a, 'info>,
    pub(crate) layout: &'a AccountInfo<'info>,
    pub party: Party<'a, 'info>,
    pub(crate) src: SyscallRoll,
    /// The callback's own participation's character, with what it wears.
    main_hero: HeroStats,
    pub(crate) input: Box<HitInput>,
    pub(crate) result: Box<HitResult>,
    pub(crate) tables: Option<Box<ResonanceTables>>,
    /// Bit per player slot whose reactive skill fired in this action.
    fired: u8,
    /// Patch 6: what the action did to the monsters (for their reactives), the monsters whose
    /// reactive fired, the monsters that have one (read once).
    pub(crate) monster_events: Box<MonsterEvents>,
    pub(crate) monster_fired: u32,
    pub(crate) reactive_monsters: Option<u32>,
}

impl<'a, 'info> Cb<'a, 'info> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_key: Pubkey,
        seed: [u8; 32],
        action: u32,
        monsters: &'a Monsters<'a, 'info>,
        layout: &'a AccountInfo<'info>,
        party: Party<'a, 'info>,
        main_hero: HeroStats,
    ) -> Self {
        Cb {
            session_key,
            seed,
            action,
            monsters,
            layout,
            party,
            src: SyscallRoll::new(seed),
            main_hero,
            input: Box::default(),
            result: Box::default(),
            tables: None,
            fired: 0,
            monster_events: Box::default(),
            monster_fired: 0,
            reactive_monsters: None,
        }
    }

    pub(crate) fn load_tables(&mut self) -> Result<()> {
        if self.tables.is_none() {
            let mut t = Box::<ResonanceTables>::default();
            self.monsters.tables_into(&mut t)?;
            self.tables = Some(t);
        }
        Ok(())
    }

    pub(crate) fn hero_of(&mut self, main: &Participation, slot: u8) -> Result<HeroStats> {
        if slot == main.core.slot {
            Ok(self.main_hero)
        } else {
            self.party.hero(slot)
        }
    }

    /// Runs `f` on player `slot`'s participation: the callback's own, or one of the party's.
    pub(crate) fn on_player<R>(
        &mut self,
        main: &mut Participation,
        slot: u8,
        f: impl FnOnce(&mut Self, &mut Participation) -> Result<R>,
    ) -> Result<R> {
        if slot == main.core.slot {
            return f(self, main);
        }
        let mut p = self.party.take(slot)?;
        let out = f(self, &mut p);
        self.party.put(slot, p);
        out
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_effect(
        &self,
        caster: u8,
        target: u8,
        skill_slot: u8,
        rider: bool,
        effect: deadroll_skillcheck::Effect,
        roll: &EffectRoll,
        applied: CombatApplied,
        hp: u32,
        states: &States,
        mods: &TimedMods,
    ) {
        emit!(EffectResolved {
            session: self.session_key,
            caster,
            target,
            seed: self.seed,
            action_counter: self.action,
            skill_slot,
            effect,
            rider,
            roll: roll.into(),
            applied: applied.into(),
            hp,
            states: *states,
            mods: from_combat(mods),
        });
    }

    /// The monsters on the line of `skill` from `from` through monster `target`, in line order.
    pub(crate) fn line_monsters(
        &self,
        session: &GameSession,
        from: Cell,
        target: u8,
        skill: &SkillStats,
    ) -> Result<([u8; LINE_TARGETS_MAX], usize)> {
        let to = cell_of(&self.monsters.get(target)?);
        let layout = MapLayoutView::load(self.layout)?;
        let line = skill_line(session, &layout, from, to, skill.ext.range);
        let count = self.monsters.count()?;
        let mut units = [(0u8, Cell::default()); MONSTERS_MAX];
        let mut n = 0;
        for i in 0..count {
            let m = self.monsters.get(i)?;
            if m.alive {
                units[n] = (i, cell_of(&m));
                n += 1;
            }
        }
        Ok(targets_on_line(&line, units[..n].iter().copied()))
    }

    /// One hit of player `p` (as `hero`) with `skill` (`None` = the weapon) on monster `index`:
    /// HP, states, death, the skill's effect and the resonance's spread, with their events.
    /// Returns the monster's knockback.
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn hit_monster(
        &mut self,
        session: &mut GameSession,
        p: &mut Participation,
        hero: &HeroStats,
        skill: Option<&SkillStats>,
        skill_slot: u8,
        index: u8,
    ) -> Result<Option<(Cell, Cell)>> {
        let mut monster = self.monsters.get(index)?;
        if !monster.alive {
            return Ok(None);
        }
        self.load_tables()?;
        let mut mods = self.monsters.mods(index)?;
        let profile = self.monsters.profile(index, monster.level)?;
        let (hero_mods, view) = {
            let field = monster_cb::Field::load(session, self.monsters)?;
            let view = field.view();
            (
                p.mods_with_auras(&view, skill.is_none()),
                view.monster_mods(&mods, index, cell_of(&monster), profile.kin, false),
            )
        };
        let before = monster.hp;
        let attacker = Unit::Player(p.core.slot).id();
        let target = Unit::Monster(index).id();
        let mut accuracy = AccuracyBuff::from(p.accuracy);
        let tables = self
            .tables
            .as_deref()
            .ok_or(SessionError::InvalidMonsterState)?;
        let outcome = resolve::attack_monster(
            BALANCE,
            tables,
            attacker,
            hero,
            skill,
            &hero_mods,
            &mut accuracy,
            &profile,
            &mut monster,
            &mut mods,
            &view,
            &mut self.input,
            &mut self.result,
            &mut self.src,
        );
        p.accuracy = accuracy.into();
        if outcome.died {
            outcomes::monster_killed(
                session,
                self.session_key,
                self.monsters,
                &mut monster,
                index,
                attacker,
                self.seed,
            )?;
        }
        emit_hit_resolved(
            &self.session_key,
            attacker,
            target,
            &self.seed,
            self.action,
            &self.result,
            &HitItemsData {
                attacker: ItemBonusData::of(hero),
                defender: ItemBonusData::default(),
            },
        )?;
        // A resonance's timed modifiers landed on `mods` in the hit, a rider's after it.
        self.monsters.set_mods(index, &mods)?;
        if let (Some(rider), Some(skill)) = (outcome.rider, skill) {
            self.emit_effect(
                attacker,
                target,
                skill_slot,
                true,
                skill.blocks.effect,
                &rider.roll,
                rider.applied,
                monster.hp,
                &monster.states,
                &mods,
            );
        }
        self.monsters.set(index, &monster)?;
        self.note_monster_hit(index, before, p.core.slot)?;
        self.spread_hit(
            session,
            index,
            cell_of_player(p),
            attacker,
            target,
            &accuracy,
        )
    }

    /// Plays the jumps, knockback and burning cells of the last hit on monster `target` and
    /// writes `ResonanceSpread` (`deadroll_rules::spread`, the simulator's code too).
    #[inline(never)]
    fn spread_hit(
        &mut self,
        session: &GameSession,
        target: u8,
        from: Cell,
        attacker: u8,
        target_unit: u8,
        accuracy: &AccuracyBuff,
    ) -> Result<Option<(Cell, Cell)>> {
        let result = &self.result;
        if result.jump_count == 0 && result.knockback == 0 && result.ignite_power == 0 {
            if !result.accuracy_buff.is_empty() {
                self.emit_spread(
                    attacker,
                    target_unit,
                    &Default::default(),
                    &[],
                    accuracy,
                    0,
                    session.core.round,
                );
            }
            return Ok(None);
        }
        let count = self.monsters.count()?;
        let mut all = Box::new([Monster::default(); MONSTERS_MAX]);
        for (i, m) in all.iter_mut().enumerate().take(count as usize) {
            *m = self.monsters.get(i as u8)?;
        }
        let defense: [MonsterDefense; MONSTERS_MAX] = self.monsters.defenses()?;
        let layout = MapLayoutView::load(self.layout)?;
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
            field.units[field.unit_count] = cell_of(m);
            field.unit_count += 1;
        }
        let mut cells = self.monsters.burning_cells()?;
        let out = spread(
            BALANCE,
            &self.result,
            from,
            target as usize,
            &mut all[..count as usize],
            &defense[..count as usize],
            &field,
            &mut cells,
            session.core.round,
            attacker,
        );
        let mut hits = [SpreadHitData::default(); 8];
        for (h, data) in out.hits().iter().zip(hits.iter_mut()) {
            self.monsters.set(h.monster, &all[h.monster as usize])?;
            *data = SpreadHitData {
                monster: h.monster,
                kind: h.kind,
                applied: h.applied.into(),
                states: all[h.monster as usize].states,
            };
        }
        if out.moved.is_some() {
            self.monsters.set(target, &all[target as usize])?;
        }
        if out.ignited_count > 0 {
            self.monsters.set_burning_cells(&cells)?;
        }
        let until = session
            .core
            .round
            .saturating_add(BALANCE.combat.burning_cell_rounds as u16);
        let ignite = self.result.ignite_power;
        self.emit_spread(
            attacker,
            target_unit,
            &out,
            &hits[..out.hit_count as usize],
            accuracy,
            ignite,
            until,
        );
        Ok(out.moved)
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_spread(
        &self,
        attacker: u8,
        target: u8,
        out: &SpreadOutcome,
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
            session: self.session_key,
            action_counter: self.action,
            attacker,
            target,
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

    /// Player `p`'s weapon or damage skill on monster `target`: one hit, or a hit on every
    /// monster of the line through it. Returns the knockbacks.
    #[allow(clippy::too_many_arguments)]
    pub fn strike(
        &mut self,
        session: &mut GameSession,
        p: &mut Participation,
        hero: &HeroStats,
        skill: Option<&SkillStats>,
        skill_slot: u8,
        target: u8,
    ) -> Result<Moves> {
        let mut moves = Moves::default();
        let (targets, n) = match skill.filter(|s| s.blocks.target == Target::Line) {
            Some(s) => self.line_monsters(session, cell_of_player(p), target, s)?,
            None => {
                let mut one = [0u8; LINE_TARGETS_MAX];
                one[0] = target;
                (one, 1)
            }
        };
        for &index in &targets[..n] {
            if session.core.status != SessionStatus::Active {
                break;
            }
            if let Some((from, to)) =
                self.hit_monster(session, p, hero, skill, skill_slot, index)?
            {
                moves.push(index, from, to);
            }
        }
        Ok(moves)
    }

    /// Player `caster`'s debuff skill on monster `target` (on every monster of its line for a
    /// line): each rolls to land.
    #[allow(clippy::too_many_arguments)]
    pub fn debuff(
        &mut self,
        session: &GameSession,
        caster: u8,
        from: Cell,
        hero: &HeroStats,
        skill: &SkillStats,
        skill_slot: u8,
        effect: EffectAction,
        target: u8,
    ) -> Result<()> {
        let (targets, n) = if skill.blocks.target == Target::Line {
            self.line_monsters(session, from, target, skill)?
        } else {
            let mut one = [0u8; LINE_TARGETS_MAX];
            one[0] = target;
            (one, 1)
        };
        let caster_slot = caster;
        let caster = Unit::Player(caster).id();
        for &index in &targets[..n] {
            let mut monster = self.monsters.get(index)?;
            if !monster.alive {
                continue;
            }
            let mut mods = self.monsters.mods(index)?;
            let profile = self.monsters.profile(index, monster.level)?;
            let out = resolve::debuff_monster(
                BALANCE,
                caster,
                hero,
                skill,
                effect,
                &profile,
                &mut monster,
                &mut mods,
                &mut self.src,
            );
            self.monsters.set(index, &monster)?;
            self.monsters.set_mods(index, &mods)?;
            self.emit_effect(
                caster,
                Unit::Monster(index).id(),
                skill_slot,
                false,
                skill.blocks.effect,
                &out.roll,
                out.applied,
                monster.hp,
                &monster.states,
                &mods,
            );
            if monster_cb::controls(effect, out.applied) {
                self.note_monster_controlled(index, caster_slot)?;
            }
        }
        Ok(())
    }

    /// Player `caster`'s buff on player `target` (on every ally of its line for a line; the
    /// caster's own cell is not on it, so a line aimed at oneself is the caster only).
    #[allow(clippy::too_many_arguments)]
    pub fn buff(
        &mut self,
        session: &GameSession,
        main: &mut Participation,
        caster: u8,
        hero: &HeroStats,
        skill: &SkillStats,
        skill_slot: u8,
        effect: EffectAction,
        target: u8,
    ) -> Result<()> {
        let mut targets = [0u8; LINE_TARGETS_MAX];
        let mut n = 1;
        targets[0] = target;
        if skill.blocks.target == Target::Line && target != caster {
            let from = session.player_pos[caster as usize];
            let to = session.player_pos[target as usize];
            let layout = MapLayoutView::load(self.layout)?;
            let line = skill_line(session, &layout, from, to, skill.ext.range);
            let mut units = [(0u8, Cell::default()); PARTY_MAX];
            let mut m = 0;
            for slot in 0..session.core.player_count {
                if slot != caster && session.player_alive(slot) {
                    units[m] = (slot, session.player_pos[slot as usize]);
                    m += 1;
                }
            }
            (targets, n) = targets_on_line(&line, units[..m].iter().copied());
        }
        let caster_unit = Unit::Player(caster).id();
        for &slot in &targets[..n] {
            self.on_player(main, slot, |cb, t| {
                let mut mods = t.combat_mods();
                let hp_max = t.hp_max;
                let out = resolve::support_ally(
                    BALANCE,
                    caster_unit,
                    hero,
                    skill,
                    effect,
                    slot == caster,
                    &mut t.core.hp,
                    hp_max,
                    &mut t.core.states,
                    &mut mods,
                    &mut cb.src,
                );
                t.set_combat_mods(&mods);
                cb.emit_effect(
                    caster_unit,
                    Unit::Player(slot).id(),
                    skill_slot,
                    false,
                    skill.blocks.effect,
                    &out.roll,
                    out.applied,
                    t.core.hp,
                    &t.core.states,
                    &mods,
                );
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Reactions to monsters pushed next to players by this action's knockbacks.
    pub fn react_to_moves(
        &mut self,
        session: &mut GameSession,
        main: &mut Participation,
        moves: &Moves,
    ) -> Result<()> {
        let mut events = ReactEvents::default();
        for &(monster, from, to) in moves.iter() {
            for b in 0..session.core.player_count {
                if session.player_alive(b)
                    && entered_next_to(from, to, session.player_pos[b as usize])
                {
                    events.push(ReactEvent {
                        bearer: b,
                        event: Event::EnemyEntered,
                        source: Some(monster),
                    });
                }
            }
        }
        self.react(session, main, &events)
    }

    /// The reactions of a roll requested only for them (`monster_move`, `start_turn`).
    pub fn react_to_pending(
        &mut self,
        session: &mut GameSession,
        main: &mut Participation,
    ) -> Result<()> {
        let p = session.pending_reactive;
        let source = match Unit::from_id(p.source) {
            Some(Unit::Monster(i)) => Some(i),
            _ => None,
        };
        let mut events = ReactEvents::default();
        for b in 0..PARTY_MAX as u8 {
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
            events.push(ReactEvent {
                bearer: b,
                event,
                source,
            });
        }
        self.react(session, main, &events)
    }

    /// Every bearer's reaction to `events`, in player slot order.
    pub(crate) fn react(
        &mut self,
        session: &mut GameSession,
        main: &mut Participation,
        events: &ReactEvents,
    ) -> Result<()> {
        for e in events.ordered() {
            if session.core.status != SessionStatus::Active {
                break;
            }
            self.react_one(session, main, &e)?;
        }
        Ok(())
    }

    fn react_one(
        &mut self,
        session: &mut GameSession,
        main: &mut Participation,
        e: &ReactEvent,
    ) -> Result<()> {
        let bearer = e.bearer;
        let bit = 1u8 << bearer;
        if self.fired & bit != 0 || !session.player_alive(bearer) {
            return Ok(());
        }
        let data = session.reactives[bearer as usize];
        let Some(skill) = data.skill() else {
            return Ok(());
        };
        let at = session.player_pos[bearer as usize];
        let source = match e.source {
            Some(i) => {
                let m = self.monsters.get(i)?;
                m.alive.then_some((i, cell_of(&m)))
            }
            None => None,
        };
        let ally_cell = match e.event {
            Event::AllyHpDrop { ally, .. } => Some(session.player_pos[ally as usize]),
            Event::Hit { .. } | Event::HpDrop { .. } | Event::Controlled | Event::EnemyEntered => {
                None
            }
        };
        let Some(r) = reaction(
            BALANCE,
            &skill,
            data.cooldown_left,
            bearer,
            at,
            &e.event,
            source,
            ally_cell,
        ) else {
            return Ok(());
        };
        self.fired |= bit;
        let cooldown_left = cooldown_after_use(skill.cooldown);
        session.reactives[bearer as usize].cooldown_left = cooldown_left;
        let bearer_unit = Unit::Player(bearer).id();
        let (kind, target) = match r {
            Reaction::Strike { monster } => (ReactionKind::Strike, Unit::Monster(monster).id()),
            Reaction::Debuff { monster } => (ReactionKind::Debuff, Unit::Monster(monster).id()),
            Reaction::Buff { slot } => (ReactionKind::Buff, Unit::Player(slot).id()),
            Reaction::Aura => (ReactionKind::Aura, bearer_unit),
        };
        emit!(ReactiveTriggered {
            session: self.session_key,
            action_counter: self.action,
            bearer,
            skill_slot: data.skill_slot,
            trigger: skill.blocks.trigger,
            reaction: kind,
            target,
            cooldown_left,
        });

        let hero = self.hero_of(main, bearer)?;
        let effect = effect_action(skill.blocks.effect);
        match r {
            Reaction::Strike { monster } => {
                self.on_player(main, bearer, |cb, p| {
                    cb.strike(session, p, &hero, Some(&skill), data.skill_slot, monster)
                })?;
            }
            Reaction::Debuff { monster } => {
                let effect = effect.ok_or(SessionError::InvalidSkill)?;
                self.debuff(
                    session,
                    bearer,
                    at,
                    &hero,
                    &skill,
                    data.skill_slot,
                    effect,
                    monster,
                )?;
            }
            Reaction::Buff { slot } => {
                let effect = effect.ok_or(SessionError::InvalidSkill)?;
                self.buff(
                    session,
                    main,
                    bearer,
                    &hero,
                    &skill,
                    data.skill_slot,
                    effect,
                    slot,
                )?;
            }
            Reaction::Aura => {
                self.start_aura(session, main, bearer, &hero, &skill, data.skill_slot)?
            }
        }
        Ok(())
    }

    /// A reactive skill's temporary aura around `bearer`: the strength roll (`EffectResolved`),
    /// then `AuraChanged`.
    fn start_aura(
        &mut self,
        session: &mut GameSession,
        main: &mut Participation,
        bearer: u8,
        hero: &HeroStats,
        skill: &SkillStats,
        skill_slot: u8,
    ) -> Result<()> {
        let (roll, aura) = resolve::start_aura(BALANCE, hero, skill, &mut self.src);
        let unit = Unit::Player(bearer).id();
        let applied = match aura {
            Some(a) => {
                session.auras[bearer as usize][REACTIVE_AURA] = a.into();
                emit!(AuraChanged {
                    session: self.session_key,
                    bearer,
                    aura_slot: REACTIVE_AURA as u8,
                    aura: a.into(),
                });
                CombatApplied::Added
            }
            None => CombatApplied::None,
        };
        self.on_player(main, bearer, |cb, p| {
            cb.emit_effect(
                unit,
                unit,
                skill_slot,
                false,
                skill.blocks.effect,
                &roll,
                applied,
                p.core.hp,
                &p.core.states,
                &p.combat_mods(),
            );
            Ok(())
        })
    }
}

/// Whether `skill`'s purpose and effect make it a debuff action (a monster target that rolls to
/// land) rather than a hit.
pub fn is_debuff(skill: Option<&SkillStats>) -> Option<EffectAction> {
    let s = skill?;
    match s.blocks.purpose {
        Purpose::Debuff => effect_action(s.blocks.effect),
        Purpose::Damage | Purpose::Buff => None,
    }
}

/// Whether player `bearer`'s reactive skill fires on `event` (the same check as the callback's,
/// from the session alone): `monster_move` and `start_turn` ask before they request a roll.
/// `source`: the monster that caused it and its cell.
pub fn fires_for(
    session: &GameSession,
    bearer: u8,
    event: &Event,
    source: Option<(u8, Cell)>,
) -> bool {
    if !session.player_alive(bearer) {
        return false;
    }
    let data = session.reactives[bearer as usize];
    let Some(skill) = data.skill() else {
        return false;
    };
    let ally_cell = match *event {
        Event::AllyHpDrop { ally, .. } => Some(session.player_pos[ally as usize]),
        Event::Hit { .. } | Event::HpDrop { .. } | Event::Controlled | Event::EnemyEntered => None,
    };
    reaction(
        BALANCE,
        &skill,
        data.cooldown_left,
        bearer,
        session.player_pos[bearer as usize],
        event,
        source,
        ally_cell,
    )
    .is_some()
}

/// `PendingAction.skill_slot` of a roll requested only for reactives.
pub const REACTIVE_ROLL: u8 = 0xFE;

/// Requests a VRF value only for the reactives of `pending` (`monster_move`, `start_turn`): the
/// callback's `participation` is the first bearer's, the other living players come as the party
/// accounts. Not an action: `acted_seq` and the escape intents stay.
#[allow(clippy::too_many_arguments)]
pub fn request_reactive_roll(
    session: &mut GameSession,
    session_key: Pubkey,
    pending: PendingReactiveData,
    accounts: vrf::RequestAccounts,
    identity_bump: u8,
    monsters: Pubkey,
) -> Result<()> {
    require!(pending.bearers != 0, SessionError::InvalidPendingAccounts);
    let first = pending.bearers.trailing_zeros() as u8;
    let character = session.core.players[first as usize];
    let participation = session.participations[first as usize];
    let pda = |seed: &[u8]| {
        Pubkey::find_program_address(&[seed, character.as_ref()], &deadroll_character::ID).0
    };
    session.pending = PendingAction {
        attacker: Unit::Player(first).id(),
        target: pending.source,
        skill_slot: REACTIVE_ROLL,
        participation,
    };
    session.pending_target = Pubkey::default();
    session.pending_reactive = pending;
    session.core.pending_vrf = true;
    let (party, party_len) = party_keys(session, first);
    vrf::request(
        accounts,
        identity_bump,
        vrf::caller_seed(&session_key, session.core.action_counter),
        &vrf::CallbackAccounts {
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
    emit!(ActionRequested {
        session: session_key,
        action: session.core.action_counter,
        attacker: Unit::Player(first).id(),
        target: pending.source,
        skill_slot: REACTIVE_ROLL,
        action_points: 0,
    });
    Ok(())
}
