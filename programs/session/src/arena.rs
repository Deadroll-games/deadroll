//! Arena update (docs/PARTY_AND_TURNS.md 4.1, docs/MAP_DESCRIPTION.md 17): the steps between the
//! map and a pack's arena, shared by `enter_arena` and `leave_arena`. The rules (who moves first,
//! where the party stands, the timeline, the pack's experience) are `deadroll_rules::arena`.
use anchor_lang::prelude::*;
use deadroll_combat::TimedMods;
use deadroll_rules::{
    arena::pack_xp,
    aura::{monster_passive_aura, Aura, AURAS_PER_PLAYER, PASSIVE_AURA},
    monster::{select_skills, NO_SKILL, PASSIVE_SLOT},
    MonsterTemplate,
};
use deadroll_skillcheck::{monster::POOL_MAX, Category};
use deadroll_types::{
    ActiveState, CacheSlot, Monster, Spawn, MONSTER_SLOTS, PARTY_MAX, STATES_MAX,
};

use crate::{
    errors::SessionError,
    loot::Caches,
    rules::BALANCE,
    state::{GameSession, Monsters, Participation},
};

/// The `Participation` of each living player, by slot: `remaining` holds exactly those, writable,
/// in slot order.
pub fn living_participations<'info>(
    remaining: &'info [AccountInfo<'info>],
    session: &GameSession,
) -> Result<[Option<Box<Account<'info, Participation>>>; PARTY_MAX]> {
    let mut out: [Option<Box<Account<'info, Participation>>>; PARTY_MAX] = Default::default();
    let mut i = 0;
    for slot in 0..session.core.player_count {
        if !session.player_alive(slot) {
            continue;
        }
        let info = remaining.get(i).ok_or(SessionError::InvalidArenaAccounts)?;
        require!(
            *info.key == session.participations[slot as usize] && info.is_writable,
            SessionError::InvalidArenaAccounts
        );
        let p = Box::new(Account::<Participation>::try_from(info)?);
        require!(p.core.slot == slot, SessionError::InvalidArenaAccounts);
        out[slot as usize] = Some(p);
        i += 1;
    }
    require!(i == remaining.len(), SessionError::InvalidArenaAccounts);
    Ok(out)
}

/// Writes the participations back.
pub fn store_participations(players: &[Option<Box<Account<'_, Participation>>>]) -> Result<()> {
    for p in players.iter().flatten() {
        p.exit(&crate::ID)?;
    }
    Ok(())
}

/// The caches of the living players at hand (`leave_arena` pays the pack there).
pub struct ArenaCaches<'p, 'info> {
    pub players: &'p mut [Option<Box<Account<'info, Participation>>>; PARTY_MAX],
}

impl Caches for ArenaCaches<'_, '_> {
    fn add(&mut self, slot: u8, xp: u32, rewards: &[CacheSlot]) -> Result<bool> {
        Ok(
            match self.players.get_mut(slot as usize).and_then(|p| p.as_mut()) {
                Some(p) => p.add_to_cache(xp, rewards) == 0,
                None => true,
            },
        )
    }
}

/// Loads layout spawn `spawn_index` into arena slot `slot` (docs/BESTIARY.md 3.2, 12): its record
/// from the party's copy of its template, full HP, no states, buffs or cooldowns, its chosen
/// skills (the mask's pool bits, at most the rank's slots) from the party's skill table, the aura
/// of its passive skill, its speed and initiative. It stands where the DM put it in the arena.
pub fn load_monster(
    session: &mut GameSession,
    monsters: &Monsters,
    slot: u8,
    spawn_index: u8,
    spawn: &Spawn,
) -> Result<()> {
    let record = monsters.template_record(spawn.bestiary_id)?;
    let template =
        MonsterTemplate::from_record(&record).ok_or(error!(SessionError::InvalidMonster))?;
    monsters.set_record(slot, &record)?;
    monsters.set_defense(slot, &template.defense)?;
    monsters.set_spawn_of(slot, spawn_index)?;
    monsters.set_skill_slots(slot, &pick_skills(monsters, spawn, &template)?)?;
    monsters.set_skill_cooldowns(slot, &[0; MONSTER_SLOTS])?;
    monsters.set_mods(slot, &TimedMods::default())?;
    let mut auras = [Aura::default(); AURAS_PER_PLAYER];
    if let Some(skill) = monsters.skill(slot, PASSIVE_SLOT as u8)? {
        let profile = template.profile(spawn.level, &BALANCE.monsters);
        if let Some(a) = monster_passive_aura(BALANCE, &profile, &skill) {
            auras[PASSIVE_AURA] = a;
        }
    }
    monsters.set_auras(slot, &auras)?;
    let hp = template.hp_at(spawn.level, spawn.flags, &BALANCE.monsters);
    monsters.set(
        slot,
        &Monster {
            bestiary_id: spawn.bestiary_id,
            level: spawn.level,
            x: spawn.x,
            y: spawn.y,
            hp,
            hp_max: hp,
            action_points: 0,
            alive: true,
            pack: spawn.pack,
            flags: spawn.flags,
            states: [ActiveState::default(); STATES_MAX],
        },
    )?;
    session.monster_speed[slot as usize] = template.speed;
    session.monster_initiative[slot as usize] = template.initiative;
    Ok(())
}

/// The spawn's skill slots: indices into the party's skill table (`open_session` copied every
/// chosen pool skill with its registry index).
fn pick_skills(
    monsters: &Monsters,
    spawn: &Spawn,
    template: &MonsterTemplate,
) -> Result<[u8; MONSTER_SLOTS]> {
    let len = (template.pool_len as usize).min(POOL_MAX);
    let mut table = [NO_SKILL; POOL_MAX];
    // Unchosen pool entries keep a category that never takes a slot: their mask bit is clear.
    let mut categories = [Category::Active; POOL_MAX];
    for i in 0..len {
        if spawn.skills & (1 << i) == 0 {
            continue;
        }
        let id = template.pool_start as u16 + i as u16;
        let t = monsters
            .skill_index_of(id)?
            .ok_or(error!(SessionError::InvalidMonsterState))?;
        categories[i] = monsters.table_skill(t)?.blocks.category;
        table[i] = t;
    }
    let chosen = select_skills(
        &categories[..len],
        spawn.skills,
        template.slots(spawn.flags, &BALANCE.monsters),
    );
    let mut slots = [NO_SKILL; MONSTER_SLOTS];
    for (slot, &p) in slots.iter_mut().zip(chosen.iter()) {
        if p != NO_SKILL {
            *slot = table[p as usize];
        }
    }
    Ok(slots)
}

/// The experience of the arena's pack: the sum of its monsters'.
pub fn arena_pack_xp(monsters: &Monsters) -> Result<u32> {
    let mut xp = [0u32; deadroll_types::ARENA_MONSTERS_MAX];
    let count = monsters.count()?;
    for (i, x) in xp.iter_mut().enumerate().take(count as usize) {
        let m = monsters.get(i as u8)?;
        *x = monsters.profile(i as u8, m.level)?.xp;
    }
    Ok(pack_xp(xp))
}

/// Every monster of the arena is dead.
pub fn pack_dead(monsters: &Monsters) -> Result<bool> {
    for i in 0..monsters.count()? {
        if monsters.get(i)?.alive {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_support::{ids, monsters_account, registry, session};
    use deadroll_types::Cell;

    #[test]
    fn a_spawn_loads_into_its_arena_slot_with_full_hp_and_its_skills() {
        let reg = registry();
        let key = Pubkey::new_unique();
        let mut account = monsters_account(key, &[]);
        let info = account.info();
        let monsters = Monsters::load(&info, &key).unwrap();
        let t = reg.template(ids::WOLF).unwrap();
        // The party's table holds the first three pool skills by their registry index (as
        // `open_session` copies the chosen ones).
        for (i, k) in reg.pool(&t).iter().enumerate().take(3) {
            monsters
                .add_skill(&k.unwrap(), t.pool_start as u16 + i as u16)
                .unwrap();
        }
        let spawn = Spawn {
            bestiary_id: ids::WOLF,
            x: 3,
            y: 4,
            level: 2,
            skills: 0b0111,
            flags: 0,
            aspects: 0,
            pack: 1,
        };
        let mut s = session(&[Cell::default()]);
        load_monster(&mut s, &monsters, 2, 9, &spawn).unwrap();
        let m = monsters.get(2).unwrap();
        let hp = t.hp_at(2, 0, &BALANCE.monsters);
        assert_eq!((m.x, m.y, m.hp, m.hp_max, m.pack), (3, 4, hp, hp, 1));
        assert!(m.alive);
        assert_eq!(monsters.spawn_of(2).unwrap(), 9);
        assert_eq!(monsters.template(2).unwrap(), t);
        assert_eq!(monsters.skill_cooldowns(2).unwrap(), [0; MONSTER_SLOTS]);
        assert_eq!(s.monster_speed[2], t.speed);
        assert_eq!(s.monster_initiative[2], t.initiative);
        let chosen = monsters
            .skill_slots(2)
            .unwrap()
            .iter()
            .filter(|&&k| k != NO_SKILL)
            .count();
        assert_eq!(chosen, 3, "the mask's three pool skills");
        // A skill the party did not copy cannot be taken.
        let other = Spawn {
            skills: 0b1000,
            ..spawn
        };
        assert!(load_monster(&mut s, &monsters, 3, 10, &other).is_err());
        // A template the party did not copy neither.
        let stranger = Spawn {
            bestiary_id: 200,
            ..spawn
        };
        assert!(load_monster(&mut s, &monsters, 3, 10, &stranger).is_err());
    }

    #[test]
    fn the_pack_pays_the_sum_of_its_monsters_and_dies_with_the_last() {
        use crate::state::test_support::{monster, profile};
        let key = Pubkey::new_unique();
        let mut wolf = monster(1, 1);
        wolf.bestiary_id = ids::WOLF;
        wolf.level = 1;
        let mut goblin = monster(2, 1);
        goblin.bestiary_id = ids::GOBLIN;
        goblin.level = 1;
        let mut account = monsters_account(key, &[wolf, goblin]);
        let info = account.info();
        let monsters = Monsters::load(&info, &key).unwrap();
        assert_eq!(
            arena_pack_xp(&monsters).unwrap(),
            profile(ids::WOLF).xp + profile(ids::GOBLIN).xp
        );
        assert!(!pack_dead(&monsters).unwrap());
        for i in 0..2 {
            let mut m = monsters.get(i).unwrap();
            m.alive = false;
            monsters.set(i, &m).unwrap();
        }
        assert!(pack_dead(&monsters).unwrap());
    }
}
