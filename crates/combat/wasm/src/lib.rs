//! C ABI over `deadroll-combat` for the website: replay a hit from the VRF seed in a
//! `HitResolved` event, preview an attack before the dice (docs/COMBAT_MATH.md, section 13), and
//! a hero's derived stats. See `README.md` next to this crate for the protocol.
//!
//! Static buffers, no imports. The page writes borsh input into the input buffer (and the
//! 32-byte seed into the seed buffer for a replay), calls an export, and reads borsh bytes back
//! from the output buffer. Balance = `BalanceConfig::DEFAULT`, the numbers the programs use.
//!
//! The attack preview and the hero sheet take a hero, a skill and a monster as the chain stores
//! them and build the hit input with `deadroll-rules`, the same code the session program runs,
//! so the website repeats no formula. Resonances and monster defenses are registry data: the page
//! passes the tables the session copied at `open_session` (`set_tables`) and each monster's
//! defenses from the same account, so nothing here is a copy of the registry.
//!
//! Built with `std`: `deadroll-types` pulls `solana-pubkey`, whose `atomic` feature links `std`
//! (`parking_lot`) on non-SBF targets, so `no_std` is not possible here. std on wasm32 aborts on
//! panic and brings its own allocator.

use borsh::{BorshDeserialize, BorshSerialize};
use deadroll_combat::{
    preview_hit, preview_range, resolve_hit, AccuracyBuff, Attacker, DamageRange, Defender,
    HitInput, ModKind, ResonanceTables, TimedMods, VrfRoll,
};
use deadroll_rules::{
    aura::{Aura, AuraField, PartyAuras, AURAS_PER_PLAYER},
    input::{hero, monster_defender, player_attacker, HeroStats, SkillStats},
    items::Equipment,
    line::{line_cells, LINE_MAX},
    monster::p0,
    player::stat_damage_increases,
    BalanceConfig, MonsterDefense,
};
use deadroll_skillcheck::{SkillBlocks, SkillExt, SKILL_ASPECTS_MAX};
use deadroll_types::{
    ActiveState, Aspect, Cell, Class, Race, Stat, EQUIP_SLOTS, MAP_CELLS, MAP_MAX, PARTY_MAX,
    STATES_MAX, STATS_COUNT,
};

/// Bumped on any change of the exports or the byte layouts.
pub const ABI_VERSION: u32 = 5;
/// Size of the input and output buffers.
pub const BUFFER_LEN: usize = 4096;

const CFG: &BalanceConfig = &BalanceConfig::DEFAULT;

/// Export error codes (negative return values).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Error {
    /// `input_len` is larger than the input buffer.
    InputTooLong = -1,
    /// The input is not valid borsh of the expected type (or has trailing bytes).
    BadInput = -2,
    /// The result does not fit the output buffer.
    OutputTooSmall = -3,
    /// The target's bestiary id has no template.
    UnknownMonster = -4,
    /// No resonance tables yet (`set_tables` not called), or they do not decode.
    NoTables = -5,
}

/// The registry resonance tables as `MonsterState` holds them: the `aspects`, `traits` and
/// `resonances` bodies, back to back.
pub const TABLES_LEN: usize = deadroll_combat::tables::ASPECTS_LEN
    + deadroll_combat::tables::TRAITS_LEN
    + deadroll_combat::tables::PAIRS_LEN;

/// Decodes the three table bodies (`TABLES_LEN` bytes).
pub fn decode_tables(bytes: &[u8]) -> Result<ResonanceTables, Error> {
    use deadroll_combat::tables::{ASPECTS_LEN, TRAITS_LEN};
    if bytes.len() != TABLES_LEN {
        return Err(Error::NoTables);
    }
    let (a, rest) = bytes.split_at(ASPECTS_LEN);
    let (t, p) = rest.split_at(TRAITS_LEN);
    ResonanceTables::from_bytes(a, t, p).map_err(|_| Error::NoTables)
}

/// A character as the chain stores it: `CharacterData` fields, `Character.class_tier` and what it
/// wears (`ItemBase` bytes by `EquipSlot`, `0xFF` = nothing).
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeroInput {
    pub race: Race,
    pub class: Class,
    pub level: u16,
    pub stats: [u16; STATS_COUNT],
    pub predisposition: Aspect,
    pub predisposition_tier: u8,
    pub class_tier: u8,
    pub equipment: [u8; EQUIP_SLOTS],
}

impl HeroInput {
    pub fn hero(&self) -> HeroStats {
        HeroStats {
            race: self.race,
            class: self.class,
            level: self.level,
            stats: self.stats,
            predisposition: self.predisposition,
            predisposition_tier: self.predisposition_tier,
            class_tier: self.class_tier,
            equipment: Equipment(self.equipment),
        }
    }
}

/// A skill as `SkillRecord` stores it (without the name and description).
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkillInput {
    pub blocks: SkillBlocks,
    pub shares_bp: [u32; SKILL_ASPECTS_MAX],
    pub power_bp: u32,
}

impl SkillInput {
    /// The rules' view for a hit: range, timing and AP do not change a hit.
    pub fn stats(&self) -> SkillStats {
        SkillStats {
            blocks: self.blocks,
            shares_bp: self.shares_bp,
            power_bp: self.power_bp,
            cooldown: 0,
            ext: SkillExt::MELEE,
            ap_cost: 0,
        }
    }
}

/// A monster's registry defenses, as `MonsterState.defense` stores them.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DefenseInput {
    pub resist: u8,
    pub immune_aspects: u8,
    pub immune_states: u8,
}

impl DefenseInput {
    pub fn defense(&self) -> MonsterDefense {
        MonsterDefense {
            resist: self.resist,
            immune_aspects: self.immune_aspects,
            immune_states: self.immune_states,
        }
    }
}

/// A live monster: its bestiary template at a level, with its states, modifiers and defenses.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetInput {
    pub bestiary_id: u8,
    pub level: u8,
    pub states: [ActiveState; STATES_MAX],
    pub mods: TimedMods,
    pub defense: DefenseInput,
}

/// A monster as the battle screen shows it: its template at a level with its registry defenses.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterInput {
    pub bestiary_id: u8,
    pub level: u8,
    pub defense: DefenseInput,
}

/// What the rules make of a monster (`deadroll_rules::MonsterProfile`).
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonsterSheet {
    pub weapon: DamageRange,
    pub attack_bonus: u32,
    /// The hit threshold.
    pub evasion: u32,
    pub armor: u32,
    /// Resistance per aspect, bp (counterweight), before the cap.
    pub resist_bp: [u32; Aspect::COUNT],
    pub immune_aspects: u8,
    pub immune_states: u8,
    /// `Balance::resist_cap_bp`: a hit never uses more resistance.
    pub resist_cap_bp: u32,
}

pub fn monster_sheet(m: &MonsterInput) -> Result<MonsterSheet, Error> {
    let base = p0::monster_base(m.bestiary_id).ok_or(Error::UnknownMonster)?;
    let p = base.profile(m.level, &CFG.monsters, &m.defense.defense());
    Ok(MonsterSheet {
        weapon: p.weapon,
        attack_bonus: p.attack_bonus,
        evasion: p.defense,
        armor: p.armor,
        resist_bp: p.resist_bp,
        immune_aspects: p.immune_aspects,
        immune_states: p.immune_states,
        resist_cap_bp: CFG.combat.resist_cap_bp,
    })
}

/// One aura as `GameSession.auras` stores it (`deadroll_rules::aura::Aura`, the same fields).
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuraInput {
    pub kind: ModKind,
    pub aspect: Aspect,
    pub power_bp: u32,
    pub radius: u8,
    pub turns: u8,
    pub hostile: bool,
    pub passive: bool,
    pub self_only: bool,
}

impl From<AuraInput> for Aura {
    fn from(a: AuraInput) -> Self {
        Aura {
            kind: a.kind,
            aspect: a.aspect,
            power_bp: a.power_bp,
            radius: a.radius,
            turns: a.turns,
            hostile: a.hostile,
            passive: a.passive,
            self_only: a.self_only,
        }
    }
}

/// The party's auras as the session holds them (`GameSession.auras`, `player_pos`,
/// `alive_mask`): what the auras over a unit add (docs/SKILL_BUILDER.md 3.3, 3.5).
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PartyInput {
    pub auras: [[AuraInput; AURAS_PER_PLAYER]; PARTY_MAX],
    pub cells: [Cell; PARTY_MAX],
    pub alive: u8,
}

impl PartyInput {
    fn auras(&self) -> PartyAuras {
        self.auras.map(|p| p.map(Aura::from))
    }
}

/// A player's attack on a monster: the weapon's basic attack (`skill = None`) or a damage skill.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttackRequest {
    pub hero: HeroInput,
    /// The attacker's buffs and debuffs (its own; the auras come from `party`).
    pub hero_mods: TimedMods,
    pub skill: Option<SkillInput>,
    pub target: TargetInput,
    /// The attacker's accuracy buff from a resonance (`Participation.accuracy`).
    pub accuracy: AccuracyBuff,
    /// Patch 5 (ABI 5): the auras, the attacker's player slot and the target's cell.
    pub party: PartyInput,
    pub slot: u8,
    pub target_at: Cell,
}

/// A unit's buffs and debuffs with the auras over it (`unit_mods`): a player slot, or a monster
/// at `at` (`monster`). `basic_attack`: the view of a player's basic attack, which no passive
/// Empowerment strengthens.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitModsRequest {
    pub party: PartyInput,
    pub mods: TimedMods,
    pub monster: bool,
    pub slot: u8,
    pub at: Cell,
    pub basic_attack: bool,
}

pub fn unit_mods(r: &UnitModsRequest) -> TimedMods {
    let auras = r.party.auras();
    let field = AuraField {
        auras: &auras,
        cells: &r.party.cells,
        alive: r.party.alive,
    };
    if r.monster {
        field.monster_mods(&r.mods, r.at)
    } else {
        field.hero_mods(&r.mods, r.slot, r.at, r.basic_attack)
    }
}

/// The cells of a line target (`line`): from `from` through `through` up to `reach` cells, on a
/// `width` x `height` map whose impassable cells are set in `blocked` (bit `y * MAP_MAX + x`).
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineRequest {
    pub from: Cell,
    pub through: Cell,
    pub reach: u8,
    pub width: u8,
    pub height: u8,
    pub blocked: [u8; MAP_CELLS / 8],
}

/// What `line` writes: the cells, nearest first.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineOutput {
    pub len: u8,
    pub cells: [Cell; LINE_MAX],
}

pub fn line(r: &LineRequest) -> LineOutput {
    let open = |c: Cell| {
        let i = c.y as usize * MAP_MAX + c.x as usize;
        c.x < r.width && c.y < r.height && r.blocked[i / 8] & (1 << (i % 8)) == 0
    };
    let l = line_cells(r.from, r.through, r.reach, open);
    LineOutput {
        len: l.len,
        cells: l.cells,
    }
}

/// A hero's derived stats with what it wears (docs/CHARACTER.md, section 3).
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeroSheet {
    /// The `d20` bonus.
    pub accuracy: u32,
    /// The hit threshold.
    pub evasion: u32,
    pub armor: u32,
    /// Crit chance besides a natural 20, bp.
    pub crit_bp: u32,
    pub hp_max: u32,
    pub action_points: u8,
    pub initiative: u16,
    /// Damage increase, bp: Strength on martial strikes, Dexterity on martial projectiles,
    /// Intelligence on magical packets.
    pub martial_strike_bp: u32,
    pub martial_projectile_bp: u32,
    pub magical_bp: u32,
    /// State and effect chance increase, bp: martial skills (Strength), magical skills
    /// (Intelligence), tempo effects (Dexterity).
    pub martial_state_bp: u32,
    pub magical_state_bp: u32,
    pub tempo_state_bp: u32,
    /// Extra turns of the hero's own buffs on itself (Endurance).
    pub self_buff_turns: u8,
    /// The chance the crit roll uses on a hit that is not a natural 20 (`crit_bp` clamped into the
    /// chance limits), 0 when there is no crit roll.
    pub crit_roll_bp: u32,
}

pub fn hero_sheet(h: &HeroStats) -> HeroSheet {
    let (evasion, armor) = hero::defense(CFG, h);
    let damage = stat_damage_increases(&hero::stats(CFG, h), &CFG.players);
    HeroSheet {
        accuracy: hero::accuracy(CFG, h),
        evasion,
        armor,
        crit_bp: hero::crit_pct(CFG, h).saturating_mul(100),
        hp_max: hero::hp_max(CFG, h),
        action_points: hero::ap(CFG, h),
        initiative: hero::initiative(CFG, h),
        martial_strike_bp: damage[0].bp,
        martial_projectile_bp: damage[1].bp,
        magical_bp: damage[2].bp,
        martial_state_bp: hero::state_chance(CFG, h, Stat::Strength),
        magical_state_bp: hero::state_chance(CFG, h, Stat::Intelligence),
        tempo_state_bp: hero::state_chance(CFG, h, Stat::Dexterity),
        self_buff_turns: hero::self_buff_turns(CFG, h),
        // Percent the d100 crit roll is compared with. 0 when only a natural 20 crits.
        crit_roll_bp: match hero::crit_pct(CFG, h) {
            0 => 0,
            pct => CFG.combat.clamp_chance(pct),
        },
    }
}

/// The hit input of a player's attack on a monster, as the session program builds it.
pub fn attack_input(r: &AttackRequest) -> Result<HitInput, Error> {
    let base = p0::monster_base(r.target.bestiary_id).ok_or(Error::UnknownMonster)?;
    let profile = base.profile(r.target.level, &CFG.monsters, &r.target.defense.defense());
    let skill = r.skill.map(|s| s.stats());
    let auras = r.party.auras();
    let field = AuraField {
        auras: &auras,
        cells: &r.party.cells,
        alive: r.party.alive,
    };
    let at = r
        .party
        .cells
        .get(r.slot as usize)
        .copied()
        .unwrap_or_default();
    let hero_mods = field.hero_mods(&r.hero_mods, r.slot, at, skill.is_none());
    let target_mods = field.monster_mods(&r.target.mods, r.target_at);
    let attacker: Attacker = player_attacker(
        CFG,
        0,
        &r.hero.hero(),
        skill.as_ref(),
        &hero_mods,
        &r.accuracy,
    );
    let defender: Defender = monster_defender(&profile, &r.target.states, &target_mods);
    Ok(HitInput { attacker, defender })
}

/// Decodes a borsh `HitInput`, resolves it with `seed` and `tables`, writes a borsh `HitResult`
/// to `out`. Returns the number of bytes written.
pub fn resolve_bytes(
    input: &[u8],
    seed: &[u8; 32],
    tables: &ResonanceTables,
    out: &mut [u8],
) -> Result<usize, Error> {
    let input = HitInput::try_from_slice(input).map_err(|_| Error::BadInput)?;
    let result = resolve_hit(&input, &CFG.combat, tables, &mut VrfRoll::new(*seed));
    write(&result, out)
}

/// Decodes a borsh `HitInput`, writes `min: u32 LE, max: u32 LE` (8 bytes) to `out`.
pub fn preview_bytes(input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let input = HitInput::try_from_slice(input).map_err(|_| Error::BadInput)?;
    let (min, max) = preview_range(&input, &CFG.combat);
    write(&(min, max), out)
}

/// Decodes a borsh `HitInput`, writes a borsh `HitPreview`.
pub fn preview_hit_bytes(
    input: &[u8],
    tables: &ResonanceTables,
    out: &mut [u8],
) -> Result<usize, Error> {
    let input = HitInput::try_from_slice(input).map_err(|_| Error::BadInput)?;
    write(&preview_hit(&input, &CFG.combat, tables), out)
}

/// Decodes a borsh `MonsterInput`, writes a borsh `MonsterSheet`.
pub fn monster_sheet_bytes(input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let input = MonsterInput::try_from_slice(input).map_err(|_| Error::BadInput)?;
    write(&monster_sheet(&input)?, out)
}

/// Decodes a borsh `HeroInput`, writes a borsh `HeroSheet`.
pub fn hero_sheet_bytes(input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let input = HeroInput::try_from_slice(input).map_err(|_| Error::BadInput)?;
    write(&hero_sheet(&input.hero()), out)
}

/// Decodes a borsh `AttackRequest`, writes a borsh `HitPreview`.
pub fn preview_attack_bytes(
    input: &[u8],
    tables: &ResonanceTables,
    out: &mut [u8],
) -> Result<usize, Error> {
    let request = AttackRequest::try_from_slice(input).map_err(|_| Error::BadInput)?;
    write(
        &preview_hit(&attack_input(&request)?, &CFG.combat, tables),
        out,
    )
}

/// Decodes a borsh `UnitModsRequest`, writes the borsh `TimedMods` view.
pub fn unit_mods_bytes(input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let request = UnitModsRequest::try_from_slice(input).map_err(|_| Error::BadInput)?;
    write(&unit_mods(&request), out)
}

/// Decodes a borsh `LineRequest`, writes a borsh `LineOutput`.
pub fn line_bytes(input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let request = LineRequest::try_from_slice(input).map_err(|_| Error::BadInput)?;
    write(&line(&request), out)
}

fn write(value: &impl BorshSerialize, out: &mut [u8]) -> Result<usize, Error> {
    let total = out.len();
    let mut cursor = out;
    value
        .serialize(&mut cursor)
        .map_err(|_| Error::OutputTooSmall)?;
    Ok(total - cursor.len())
}

fn code(result: Result<usize, Error>) -> i32 {
    match result {
        Ok(len) => len as i32,
        Err(e) => e as i32,
    }
}

mod abi {
    use core::cell::UnsafeCell;

    use super::*;

    struct Buffers {
        input: [u8; BUFFER_LEN],
        output: [u8; BUFFER_LEN],
        seed: [u8; 32],
        /// From `set_tables`; `None` until then.
        tables: Option<ResonanceTables>,
    }

    /// wasm32 runs one thread: the exports never overlap.
    struct Shared(UnsafeCell<Buffers>);
    unsafe impl Sync for Shared {}

    static BUFFERS: Shared = Shared(UnsafeCell::new(Buffers {
        input: [0; BUFFER_LEN],
        output: [0; BUFFER_LEN],
        seed: [0; 32],
        tables: None,
    }));

    fn buffers() -> *mut Buffers {
        BUFFERS.0.get()
    }

    /// Runs `f` over `input[..input_len]`, the loaded tables and the output buffer.
    fn with_tables(
        input_len: u32,
        f: impl FnOnce(&[u8], &[u8; 32], &ResonanceTables, &mut [u8]) -> Result<usize, Error>,
    ) -> i32 {
        // SAFETY: single-threaded, no other borrow of the buffers is alive during the call.
        let b = unsafe { &mut *buffers() };
        let Some(tables) = b.tables.as_ref() else {
            return Error::NoTables as i32;
        };
        let Some(input) = b.input.get(..input_len as usize) else {
            return Error::InputTooLong as i32;
        };
        code(f(input, &b.seed, tables, &mut b.output))
    }

    /// Runs `f` over `input[..input_len]` and the output buffer.
    fn with_input(
        input_len: u32,
        f: impl FnOnce(&[u8], &[u8; 32], &mut [u8]) -> Result<usize, Error>,
    ) -> i32 {
        // SAFETY: single-threaded, no other borrow of the buffers is alive during the call.
        let b = unsafe { &mut *buffers() };
        let Some(input) = b.input.get(..input_len as usize) else {
            return Error::InputTooLong as i32;
        };
        code(f(input, &b.seed, &mut b.output))
    }

    #[no_mangle]
    pub extern "C" fn abi_version() -> u32 {
        ABI_VERSION
    }

    #[no_mangle]
    pub extern "C" fn buffer_len() -> u32 {
        BUFFER_LEN as u32
    }

    #[no_mangle]
    pub extern "C" fn input_ptr() -> *mut u8 {
        // SAFETY: only an address is taken; no reference outlives this call.
        unsafe { (*buffers()).input.as_mut_ptr() }
    }

    #[no_mangle]
    pub extern "C" fn output_ptr() -> *const u8 {
        // SAFETY: as above.
        unsafe { (*buffers()).output.as_ptr() }
    }

    #[no_mangle]
    pub extern "C" fn seed_ptr() -> *mut u8 {
        // SAFETY: as above.
        unsafe { (*buffers()).seed.as_mut_ptr() }
    }

    /// Resolves `input[..input_len]` with the seed buffer. Returns the output length or an
    /// [`Error`] code.
    #[no_mangle]
    pub extern "C" fn resolve(input_len: u32) -> i32 {
        with_tables(input_len, resolve_bytes)
    }

    /// Loads the resonance tables (`TABLES_LEN` bytes in the input buffer) for `resolve`,
    /// `preview_hit` and `preview_attack`. Returns 0, or `NoTables` (they stay unloaded).
    #[no_mangle]
    pub extern "C" fn set_tables(input_len: u32) -> i32 {
        // SAFETY: as in `with_input`.
        let b = unsafe { &mut *buffers() };
        let Some(input) = b.input.get(..input_len as usize) else {
            return Error::InputTooLong as i32;
        };
        match decode_tables(input) {
            Ok(t) => {
                b.tables = Some(t);
                0
            }
            Err(e) => {
                b.tables = None;
                e as i32
            }
        }
    }

    /// A monster's numbers: `MonsterInput` in, `MonsterSheet` out.
    #[no_mangle]
    pub extern "C" fn monster_sheet(input_len: u32) -> i32 {
        with_input(input_len, |input, _, out| monster_sheet_bytes(input, out))
    }

    /// Damage range without dice: writes `min, max` as two `u32` LE. Returns 8 or an error.
    #[no_mangle]
    pub extern "C" fn preview(input_len: u32) -> i32 {
        with_input(input_len, |input, _, out| preview_bytes(input, out))
    }

    /// Hit chance, crit chance and damage of a `HitInput` without dice: a borsh `HitPreview`.
    #[no_mangle]
    pub extern "C" fn preview_hit(input_len: u32) -> i32 {
        with_tables(input_len, |input, _, t, out| {
            preview_hit_bytes(input, t, out)
        })
    }

    /// A hero's derived stats: `HeroInput` in, `HeroSheet` out.
    #[no_mangle]
    pub extern "C" fn hero_sheet(input_len: u32) -> i32 {
        with_input(input_len, |input, _, out| hero_sheet_bytes(input, out))
    }

    /// A player's attack on a monster before the dice: `AttackRequest` in, `HitPreview` out.
    #[no_mangle]
    pub extern "C" fn preview_attack(input_len: u32) -> i32 {
        with_tables(input_len, |input, _, t, out| {
            preview_attack_bytes(input, t, out)
        })
    }

    /// A unit's buffs and debuffs with the auras over it: `UnitModsRequest` in, `TimedMods` out.
    #[no_mangle]
    pub extern "C" fn unit_mods(input_len: u32) -> i32 {
        with_input(input_len, |input, _, out| unit_mods_bytes(input, out))
    }

    /// The cells of a line target: `LineRequest` in, `LineOutput` out.
    #[no_mangle]
    pub extern "C" fn line(input_len: u32) -> i32 {
        with_input(input_len, |input, _, out| line_bytes(input, out))
    }

    /// Roll number `index` of the seed buffer (`VrfRoll::roll_at`), for replay checks.
    #[no_mangle]
    pub extern "C" fn roll_u64(index: u32) -> u64 {
        // SAFETY: as in `with_input`.
        let seed = unsafe { &(*buffers()).seed };
        VrfRoll::roll_at(seed, index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_combat::{
        Carrier, DamageRange, HitPreview, HitResult, ModKind, Skill, TimedMod, Tint,
    };
    use deadroll_rules::{
        input::monster_defender as rules_defender, items::starter_loadout, registry::Registry,
    };
    use deadroll_skillcheck::presets;
    use deadroll_types::{Form, Source};

    fn registry() -> Registry {
        Registry::shipped().unwrap()
    }

    fn table_bytes() -> Vec<u8> {
        let r = registry();
        [r.aspects, r.traits, r.resonances].concat()
    }

    fn input() -> HitInput {
        let mut input = HitInput::default();
        input.attacker.weapon = DamageRange::new(Aspect::Piercing, 4, 8);
        input.attacker.skill = Skill::basic(Source::Combat, Form::Strike, 0);
        input.attacker.tints[0] = Tint::new(Aspect::Cold, 3, Carrier::Race);
        input.defender.defense = 8;
        input.defender.armor = 15;
        input
    }

    const NO_MODS: TimedMods = [TimedMod {
        kind: ModKind::Haste,
        aspect: Aspect::Fire,
        turns: 0,
        power_bp: 0,
        source: 0,
    }; deadroll_combat::MODS_MAX];

    fn warrior() -> HeroInput {
        HeroInput {
            race: Race::Human,
            class: Class::Warrior,
            level: 1,
            stats: [12, 6, 5, 7],
            predisposition: Aspect::Piercing,
            predisposition_tier: 14,
            class_tier: 14,
            equipment: starter_loadout(Class::Warrior, &CFG.items).equipment().0,
        }
    }

    fn defense(id: u8) -> DefenseInput {
        let d = registry().defense_of(id);
        DefenseInput {
            resist: d.resist,
            immune_aspects: d.immune_aspects,
            immune_states: d.immune_states,
        }
    }

    fn request(skill: Option<SkillInput>, id: u8) -> AttackRequest {
        AttackRequest {
            hero: warrior(),
            hero_mods: NO_MODS,
            skill,
            target: TargetInput {
                bestiary_id: id,
                level: 1,
                states: [ActiveState::default(); STATES_MAX],
                mods: NO_MODS,
                defense: defense(id),
            },
            accuracy: AccuracyBuff::default(),
            party: PartyInput::default(),
            slot: 0,
            target_at: Cell::default(),
        }
    }

    #[test]
    fn tables_decode_from_the_session_bytes() {
        let bytes = table_bytes();
        assert_eq!(bytes.len(), TABLES_LEN);
        assert_eq!(decode_tables(&bytes), Ok(registry().tables));
        assert_eq!(decode_tables(&bytes[1..]), Err(Error::NoTables));
        let mut bad = bytes.clone();
        bad[0] = 3;
        assert_eq!(decode_tables(&bad), Err(Error::NoTables));
    }

    #[test]
    fn resolve_round_trip_matches_direct_call() {
        let tables = registry().tables;
        let input = input();
        let bytes = borsh::to_vec(&input).unwrap();
        assert!(bytes.len() < BUFFER_LEN);
        let mut out = [0u8; BUFFER_LEN];
        for s in 0u8..32 {
            let seed = [s; 32];
            let len = resolve_bytes(&bytes, &seed, &tables, &mut out).unwrap();
            let decoded = HitResult::try_from_slice(&out[..len]).unwrap();
            let direct = resolve_hit(&input, &CFG.combat, &tables, &mut VrfRoll::new(seed));
            assert_eq!(decoded, direct);
        }
    }

    #[test]
    fn preview_bytes_layout() {
        let bytes = borsh::to_vec(&input()).unwrap();
        let mut out = [0u8; 16];
        assert_eq!(preview_bytes(&bytes, &mut out), Ok(8));
        let (min, max) = preview_range(&input(), &CFG.combat);
        assert_eq!(out[..4], min.to_le_bytes());
        assert_eq!(out[4..8], max.to_le_bytes());
    }

    #[test]
    fn preview_hit_round_trip() {
        let tables = registry().tables;
        let bytes = borsh::to_vec(&input()).unwrap();
        let mut out = [0u8; BUFFER_LEN];
        let len = preview_hit_bytes(&bytes, &tables, &mut out).unwrap();
        let decoded = HitPreview::try_from_slice(&out[..len]).unwrap();
        assert_eq!(
            decoded,
            deadroll_combat::preview_hit(&input(), &CFG.combat, &tables)
        );
        // The Cold tint on the Piercing basic attack: Icy Point.
        assert_eq!(decoded.tint_count, 1);
        assert_eq!(decoded.tints[0].id, 31);
    }

    #[test]
    fn hero_sheet_is_the_rules() {
        let h = warrior();
        let bytes = borsh::to_vec(&h).unwrap();
        let mut out = [0u8; BUFFER_LEN];
        let len = hero_sheet_bytes(&bytes, &mut out).unwrap();
        let sheet = HeroSheet::try_from_slice(&out[..len]).unwrap();
        let hero = h.hero();
        assert_eq!(sheet, hero_sheet(&hero));
        assert_eq!(sheet.accuracy, hero::accuracy(CFG, &hero));
        assert_eq!((sheet.evasion, sheet.armor), hero::defense(CFG, &hero));
        assert_eq!(sheet.hp_max, hero::hp_max(CFG, &hero));
        assert_eq!(sheet.action_points, hero::ap(CFG, &hero));
        assert_eq!(sheet.martial_strike_bp, 12 * CFG.players.str_damage_bp);
        assert_eq!(sheet.magical_state_bp, 5 * CFG.players.int_state_chance_bp);
    }

    #[test]
    fn monster_sheet_reads_the_given_defenses() {
        let wolf = MonsterInput {
            bestiary_id: 0,
            level: 1,
            defense: defense(0),
        };
        let bytes = borsh::to_vec(&wolf).unwrap();
        let mut out = [0u8; BUFFER_LEN];
        let len = monster_sheet_bytes(&bytes, &mut out).unwrap();
        let sheet = MonsterSheet::try_from_slice(&out[..len]).unwrap();
        assert_eq!(
            sheet.resist_bp[Aspect::Cold as usize],
            CFG.monsters.resist_bp
        );
        assert_eq!(sheet.resist_cap_bp, CFG.combat.resist_cap_bp);
        let bare = MonsterInput {
            defense: DefenseInput::default(),
            ..wolf
        };
        assert_eq!(monster_sheet(&bare).unwrap().resist_bp, [0; Aspect::COUNT]);
        let unknown = MonsterInput {
            bestiary_id: 200,
            ..wolf
        };
        assert_eq!(monster_sheet(&unknown), Err(Error::UnknownMonster));
    }

    #[test]
    fn preview_attack_builds_the_program_input() {
        let tables = registry().tables;
        let thrust = presets::WARRIOR[0].blocks;
        let d = deadroll_skillcheck::derive(&thrust);
        let skill = SkillInput {
            blocks: thrust,
            shares_bp: d.shares_bp,
            power_bp: d.power_bp,
        };
        for r in [request(None, 0), request(Some(skill), 1)] {
            let bytes = borsh::to_vec(&r).unwrap();
            let mut out = [0u8; BUFFER_LEN];
            let len = preview_attack_bytes(&bytes, &tables, &mut out).unwrap();
            let p = HitPreview::try_from_slice(&out[..len]).unwrap();
            let profile = p0::monster_base(r.target.bestiary_id).unwrap().profile(
                1,
                &CFG.monsters,
                &r.target.defense.defense(),
            );
            let stats = r.skill.map(|s| s.stats());
            let expected = HitInput {
                attacker: player_attacker(
                    CFG,
                    0,
                    &r.hero.hero(),
                    stats.as_ref(),
                    &NO_MODS,
                    &AccuracyBuff::default(),
                ),
                defender: rules_defender(&profile, &r.target.states, &NO_MODS),
            };
            assert_eq!(
                p,
                deadroll_combat::preview_hit(&expected, &CFG.combat, &tables)
            );
            assert!(p.hit_bp > 0 && p.max >= p.min);
        }
        let mut unknown = request(None, 1);
        unknown.target.bestiary_id = 200;
        let bytes = borsh::to_vec(&unknown).unwrap();
        let mut out = [0u8; BUFFER_LEN];
        assert_eq!(
            preview_attack_bytes(&bytes, &tables, &mut out),
            Err(Error::UnknownMonster)
        );
    }

    #[test]
    fn errors() {
        let tables = registry().tables;
        let bytes = borsh::to_vec(&input()).unwrap();
        let mut out = [0u8; BUFFER_LEN];
        assert_eq!(
            resolve_bytes(&bytes[..10], &[0; 32], &tables, &mut out),
            Err(Error::BadInput)
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            resolve_bytes(&trailing, &[0; 32], &tables, &mut out),
            Err(Error::BadInput)
        );
        assert_eq!(
            resolve_bytes(&bytes, &[0; 32], &tables, &mut out[..16]),
            Err(Error::OutputTooSmall)
        );
        assert_eq!(hero_sheet_bytes(&[1, 2], &mut out), Err(Error::BadInput));
        assert_eq!(code(Err(Error::InputTooLong)), -1);
        assert_eq!(code(Err(Error::UnknownMonster)), -4);
        assert_eq!(code(Err(Error::NoTables)), -5);
    }

    #[test]
    fn exports_on_host() {
        let tables = registry().tables;
        let bytes = borsh::to_vec(&input()).unwrap();
        let table_bytes = table_bytes();
        // SAFETY: test-only access to the export buffers; this is the only test using them.
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), abi::input_ptr(), bytes.len());
            core::ptr::write_bytes(abi::seed_ptr(), 7, 32);
        }
        // No tables yet.
        assert_eq!(abi::resolve(bytes.len() as u32), Error::NoTables as i32);
        unsafe {
            core::ptr::copy_nonoverlapping(
                table_bytes.as_ptr(),
                abi::input_ptr(),
                table_bytes.len(),
            );
        }
        assert_eq!(abi::set_tables(table_bytes.len() as u32), 0);
        assert_eq!(abi::set_tables(3), Error::NoTables as i32);
        assert_eq!(abi::resolve(bytes.len() as u32), Error::NoTables as i32);
        unsafe {
            core::ptr::copy_nonoverlapping(
                table_bytes.as_ptr(),
                abi::input_ptr(),
                table_bytes.len(),
            );
        }
        assert_eq!(abi::set_tables(table_bytes.len() as u32), 0);
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), abi::input_ptr(), bytes.len());
        }
        let len = abi::resolve(bytes.len() as u32);
        assert!(len > 0);
        let out = unsafe { core::slice::from_raw_parts(abi::output_ptr(), len as usize) };
        let decoded = HitResult::try_from_slice(out).unwrap();
        assert_eq!(
            decoded,
            resolve_hit(&input(), &CFG.combat, &tables, &mut VrfRoll::new([7; 32]))
        );
        let len = abi::preview_hit(bytes.len() as u32);
        let out = unsafe { core::slice::from_raw_parts(abi::output_ptr(), len as usize) };
        assert_eq!(
            HitPreview::try_from_slice(out).unwrap(),
            deadroll_combat::preview_hit(&input(), &CFG.combat, &tables)
        );
        assert_eq!(abi::roll_u64(0), VrfRoll::roll_at(&[7; 32], 0));
        assert_eq!(
            abi::resolve(BUFFER_LEN as u32 + 1),
            Error::InputTooLong as i32
        );
        assert_eq!(abi::abi_version(), ABI_VERSION);
        assert_eq!(abi::buffer_len(), BUFFER_LEN as u32);
    }

    fn party_with(aura: AuraInput, bearer: Cell) -> PartyInput {
        let mut party = PartyInput {
            alive: 0b1,
            ..PartyInput::default()
        };
        party.auras[0][0] = aura;
        party.cells[0] = bearer;
        party
    }

    fn aura(kind: ModKind, power_bp: u32, radius: u8, hostile: bool) -> AuraInput {
        AuraInput {
            kind,
            aspect: Aspect::Cold,
            power_bp,
            radius,
            turns: u8::MAX,
            hostile,
            passive: true,
            self_only: false,
        }
    }

    #[test]
    fn the_attack_preview_sees_the_auras() {
        let tables = registry().tables;
        let at = Cell { x: 2, y: 2 };
        let target_at = Cell { x: 3, y: 2 };
        let mut r = request(None, 1);
        r.slot = 0;
        r.target_at = target_at;
        let plain = attack_input(&r).unwrap();
        // A debuff aura over the target: Vulnerability to Piercing (the sword's aspect).
        let mut vuln = aura(ModKind::Vulnerability, 5_000, 2, true);
        vuln.aspect = Aspect::Piercing;
        r.party = party_with(vuln, at);
        let exposed = attack_input(&r).unwrap();
        assert_eq!(exposed.defender.vuln_bp[Aspect::Piercing as usize], 5_000);
        assert_eq!(plain.defender.vuln_bp[Aspect::Piercing as usize], 0);
        let a = deadroll_combat::preview_hit(&plain, &CFG.combat, &tables);
        let b = deadroll_combat::preview_hit(&exposed, &CFG.combat, &tables);
        assert!(b.max > a.max);
        // A passive Empowerment aura does not strengthen the basic attack (SKILL_BUILDER 3.1).
        r.party = party_with(aura(ModKind::Empowerment, 5_000, 2, false), at);
        assert_eq!(attack_input(&r).unwrap(), plain);
    }

    #[test]
    fn unit_mods_merge_the_auras_over_a_unit() {
        let at = Cell { x: 4, y: 4 };
        let party = party_with(aura(ModKind::Protection, 3_000, 1, false), at);
        let view = |slot: u8, cell: Cell, monster: bool| {
            let r = UnitModsRequest {
                party,
                mods: NO_MODS,
                monster,
                slot,
                at: cell,
                basic_attack: false,
            };
            let mut out = [0u8; BUFFER_LEN];
            let len = unit_mods_bytes(&borsh::to_vec(&r).unwrap(), &mut out).unwrap();
            TimedMods::try_from_slice(&out[..len]).unwrap()
        };
        let power = |m: TimedMods| m.iter().find(|x| x.turns > 0).map_or(0, |x| x.power_bp);
        assert_eq!(power(view(0, at, false)), 3_000);
        assert_eq!(power(view(1, Cell { x: 5, y: 5 }, false)), 3_000);
        assert_eq!(power(view(1, Cell { x: 6, y: 4 }, false)), 0);
        assert_eq!(
            power(view(0, Cell { x: 5, y: 4 }, true)),
            0,
            "not on monsters"
        );
    }

    #[test]
    fn line_runs_through_the_target_and_stops_at_a_wall() {
        let mut r = LineRequest {
            from: Cell { x: 1, y: 1 },
            through: Cell { x: 3, y: 1 },
            reach: 6,
            width: 10,
            height: 10,
            blocked: [0; MAP_CELLS / 8],
        };
        let cells = |r: &LineRequest| {
            let mut out = [0u8; BUFFER_LEN];
            let len = line_bytes(&borsh::to_vec(r).unwrap(), &mut out).unwrap();
            let l = LineOutput::try_from_slice(&out[..len]).unwrap();
            l.cells[..l.len as usize].to_vec()
        };
        assert_eq!(cells(&r).len(), 6);
        let wall = MAP_MAX + 5;
        r.blocked[wall / 8] |= 1 << (wall % 8);
        assert_eq!(
            cells(&r),
            (2..=4).map(|x| Cell { x, y: 1 }).collect::<Vec<_>>()
        );
        assert_eq!(cells(&LineRequest { width: 4, ..r }).len(), 2);
    }
}
