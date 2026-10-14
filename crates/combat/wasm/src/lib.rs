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
//! so the website repeats no formula.
//!
//! Built with `std`: `deadroll-types` pulls `solana-pubkey`, whose `atomic` feature links `std`
//! (`parking_lot`) on non-SBF targets, so `no_std` is not possible here. std on wasm32 aborts on
//! panic and brings its own allocator.

use borsh::{BorshDeserialize, BorshSerialize};
use deadroll_combat::{
    preview_hit, preview_range, resolve_hit, Attacker, Defender, HitInput, TimedMods, VrfRoll,
};
use deadroll_rules::{
    input::{hero, monster_defender, player_attacker, HeroStats, SkillStats},
    items::Equipment,
    monster::p0,
    player::stat_damage_increases,
    BalanceConfig,
};
use deadroll_skillcheck::{SkillBlocks, SKILL_ASPECTS_MAX};
use deadroll_types::{
    ActiveState, Aspect, Class, Race, Stat, EQUIP_SLOTS, STATES_MAX, STATS_COUNT,
};

/// Bumped on any change of the exports or the byte layouts.
pub const ABI_VERSION: u32 = 2;
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

/// A live monster: its bestiary template at a level, with its states and modifiers.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetInput {
    pub bestiary_id: u8,
    pub level: u8,
    pub states: [ActiveState; STATES_MAX],
    pub mods: TimedMods,
}

/// A player's attack on a monster: the weapon's basic attack (`skill = None`) or a damage skill.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttackRequest {
    pub hero: HeroInput,
    /// The attacker's buffs and debuffs.
    pub hero_mods: TimedMods,
    pub skill: Option<SkillInput>,
    pub target: TargetInput,
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
        crit_bp: hero::crit_bp(CFG, h),
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
        crit_roll_bp: match hero::crit_bp(CFG, h) {
            0 => 0,
            bp => CFG.combat.clamp_chance(bp),
        },
    }
}

/// The hit input of a player's attack on a monster, as the session program builds it.
pub fn attack_input(r: &AttackRequest) -> Result<HitInput, Error> {
    let base = p0::monster_base(r.target.bestiary_id).ok_or(Error::UnknownMonster)?;
    let profile = base.profile(r.target.level, &CFG.monsters);
    let skill = r.skill.map(|s| SkillStats {
        blocks: s.blocks,
        shares_bp: s.shares_bp,
        power_bp: s.power_bp,
        cooldown: 0,
    });
    let attacker: Attacker = player_attacker(CFG, 0, &r.hero.hero(), skill.as_ref(), &r.hero_mods);
    let defender: Defender = monster_defender(&profile, &r.target.states, &r.target.mods);
    Ok(HitInput { attacker, defender })
}

/// Decodes a borsh `HitInput`, resolves it with `seed`, writes a borsh `HitResult` to `out`.
/// Returns the number of bytes written.
pub fn resolve_bytes(input: &[u8], seed: &[u8; 32], out: &mut [u8]) -> Result<usize, Error> {
    let input = HitInput::try_from_slice(input).map_err(|_| Error::BadInput)?;
    let result = resolve_hit(&input, &CFG.combat, &mut VrfRoll::new(*seed));
    write(&result, out)
}

/// Decodes a borsh `HitInput`, writes `min: u32 LE, max: u32 LE` (8 bytes) to `out`.
pub fn preview_bytes(input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let input = HitInput::try_from_slice(input).map_err(|_| Error::BadInput)?;
    let (min, max) = preview_range(&input, &CFG.combat);
    write(&(min, max), out)
}

/// Decodes a borsh `HitInput`, writes a borsh `HitPreview`.
pub fn preview_hit_bytes(input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let input = HitInput::try_from_slice(input).map_err(|_| Error::BadInput)?;
    write(&preview_hit(&input, &CFG.combat), out)
}

/// Decodes a borsh `HeroInput`, writes a borsh `HeroSheet`.
pub fn hero_sheet_bytes(input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let input = HeroInput::try_from_slice(input).map_err(|_| Error::BadInput)?;
    write(&hero_sheet(&input.hero()), out)
}

/// Decodes a borsh `AttackRequest`, writes a borsh `HitPreview`.
pub fn preview_attack_bytes(input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let request = AttackRequest::try_from_slice(input).map_err(|_| Error::BadInput)?;
    write(&preview_hit(&attack_input(&request)?, &CFG.combat), out)
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
    }

    /// wasm32 runs one thread: the exports never overlap.
    struct Shared(UnsafeCell<Buffers>);
    unsafe impl Sync for Shared {}

    static BUFFERS: Shared = Shared(UnsafeCell::new(Buffers {
        input: [0; BUFFER_LEN],
        output: [0; BUFFER_LEN],
        seed: [0; 32],
    }));

    fn buffers() -> *mut Buffers {
        BUFFERS.0.get()
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
        with_input(input_len, resolve_bytes)
    }

    /// Damage range without dice: writes `min, max` as two `u32` LE. Returns 8 or an error.
    #[no_mangle]
    pub extern "C" fn preview(input_len: u32) -> i32 {
        with_input(input_len, |input, _, out| preview_bytes(input, out))
    }

    /// Hit chance, crit chance and damage of a `HitInput` without dice: a borsh `HitPreview`.
    #[no_mangle]
    pub extern "C" fn preview_hit(input_len: u32) -> i32 {
        with_input(input_len, |input, _, out| preview_hit_bytes(input, out))
    }

    /// A hero's derived stats: `HeroInput` in, `HeroSheet` out.
    #[no_mangle]
    pub extern "C" fn hero_sheet(input_len: u32) -> i32 {
        with_input(input_len, |input, _, out| hero_sheet_bytes(input, out))
    }

    /// A player's attack on a monster before the dice: `AttackRequest` in, `HitPreview` out.
    #[no_mangle]
    pub extern "C" fn preview_attack(input_len: u32) -> i32 {
        with_input(input_len, |input, _, out| preview_attack_bytes(input, out))
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
    use deadroll_rules::{input::monster_defender as rules_defender, items::starter_loadout};
    use deadroll_skillcheck::presets;
    use deadroll_types::{Form, Source};

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

    fn request(skill: Option<SkillInput>) -> AttackRequest {
        AttackRequest {
            hero: warrior(),
            hero_mods: NO_MODS,
            skill,
            target: TargetInput {
                bestiary_id: 1,
                level: 1,
                states: [ActiveState::default(); STATES_MAX],
                mods: NO_MODS,
            },
        }
    }

    #[test]
    fn resolve_round_trip_matches_direct_call() {
        let input = input();
        let bytes = borsh::to_vec(&input).unwrap();
        assert!(bytes.len() < BUFFER_LEN);
        let mut out = [0u8; BUFFER_LEN];
        for s in 0u8..32 {
            let seed = [s; 32];
            let len = resolve_bytes(&bytes, &seed, &mut out).unwrap();
            let decoded = HitResult::try_from_slice(&out[..len]).unwrap();
            let direct = resolve_hit(&input, &CFG.combat, &mut VrfRoll::new(seed));
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
        let bytes = borsh::to_vec(&input()).unwrap();
        let mut out = [0u8; BUFFER_LEN];
        let len = preview_hit_bytes(&bytes, &mut out).unwrap();
        let decoded = HitPreview::try_from_slice(&out[..len]).unwrap();
        assert_eq!(decoded, deadroll_combat::preview_hit(&input(), &CFG.combat));
        // immune: bool, then six u32.
        assert_eq!(len, 1 + 6 * 4);
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
    fn preview_attack_builds_the_program_input() {
        let thrust = presets::WARRIOR[0].blocks;
        let d = deadroll_skillcheck::derive(&thrust);
        let skill = SkillInput {
            blocks: thrust,
            shares_bp: d.shares_bp,
            power_bp: d.power_bp,
        };
        for r in [request(None), request(Some(skill))] {
            let bytes = borsh::to_vec(&r).unwrap();
            let mut out = [0u8; BUFFER_LEN];
            let len = preview_attack_bytes(&bytes, &mut out).unwrap();
            let p = HitPreview::try_from_slice(&out[..len]).unwrap();
            let profile = p0::monster_base(1).unwrap().profile(1, &CFG.monsters);
            let stats = r.skill.map(|s| SkillStats {
                blocks: s.blocks,
                shares_bp: s.shares_bp,
                power_bp: s.power_bp,
                cooldown: 0,
            });
            let expected = HitInput {
                attacker: player_attacker(CFG, 0, &r.hero.hero(), stats.as_ref(), &NO_MODS),
                defender: rules_defender(&profile, &r.target.states, &NO_MODS),
            };
            assert_eq!(p, deadroll_combat::preview_hit(&expected, &CFG.combat));
            assert!(p.hit_bp > 0 && p.max >= p.min);
        }
        let mut unknown = request(None);
        unknown.target.bestiary_id = 200;
        let bytes = borsh::to_vec(&unknown).unwrap();
        let mut out = [0u8; BUFFER_LEN];
        assert_eq!(
            preview_attack_bytes(&bytes, &mut out),
            Err(Error::UnknownMonster)
        );
    }

    #[test]
    fn errors() {
        let bytes = borsh::to_vec(&input()).unwrap();
        let mut out = [0u8; BUFFER_LEN];
        assert_eq!(
            resolve_bytes(&bytes[..10], &[0; 32], &mut out),
            Err(Error::BadInput)
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            resolve_bytes(&trailing, &[0; 32], &mut out),
            Err(Error::BadInput)
        );
        assert_eq!(
            resolve_bytes(&bytes, &[0; 32], &mut out[..16]),
            Err(Error::OutputTooSmall)
        );
        assert_eq!(hero_sheet_bytes(&[1, 2], &mut out), Err(Error::BadInput));
        assert_eq!(code(Err(Error::InputTooLong)), -1);
        assert_eq!(code(Err(Error::UnknownMonster)), -4);
    }

    #[test]
    fn exports_on_host() {
        let bytes = borsh::to_vec(&input()).unwrap();
        // SAFETY: test-only access to the export buffers; this is the only test using them.
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), abi::input_ptr(), bytes.len());
            core::ptr::write_bytes(abi::seed_ptr(), 7, 32);
        }
        let len = abi::resolve(bytes.len() as u32);
        assert!(len > 0);
        let out = unsafe { core::slice::from_raw_parts(abi::output_ptr(), len as usize) };
        let decoded = HitResult::try_from_slice(out).unwrap();
        assert_eq!(
            decoded,
            resolve_hit(&input(), &CFG.combat, &mut VrfRoll::new([7; 32]))
        );
        let len = abi::preview_hit(bytes.len() as u32);
        let out = unsafe { core::slice::from_raw_parts(abi::output_ptr(), len as usize) };
        assert_eq!(
            HitPreview::try_from_slice(out).unwrap(),
            deadroll_combat::preview_hit(&input(), &CFG.combat)
        );
        assert_eq!(abi::roll_u64(0), VrfRoll::roll_at(&[7; 32], 0));
        assert_eq!(
            abi::resolve(BUFFER_LEN as u32 + 1),
            Error::InputTooLong as i32
        );
        assert_eq!(abi::abi_version(), ABI_VERSION);
        assert_eq!(abi::buffer_len(), BUFFER_LEN as u32);
    }
}
