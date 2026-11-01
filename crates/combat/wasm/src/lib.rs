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
//! so the website repeats no formula. Resonances and monsters are registry data: the page passes
//! the tables the session copied at `open_session` (`set_tables`) and each monster's bestiary
//! record and skills from the same account (`MonsterState.records`, `skill_table`), or the
//! registry tables themselves in the map builder, so nothing here is a copy of the registry.
//!
//! Patch 6 (ABI 6): monster templates and skills (`monster_sheet`, `monster_skill`,
//! `spawn_skills`), a monster's attack on a player (`preview_monster_attack`), the monsters'
//! auras in every aura view, and the monster AI of the auto mode (`monster_plan`).
//!
//! Patch 9 (ABI 7): resistances. A `TimedMod` carries its aspect weights and there are seven
//! (Resistance), an aura its weights too; the hero sheet gives the resistances and the state
//! resistance, the monster sheet the defenses by tag. The buffers grew to 8 KB (a monster plan
//! with 28 units and their modifiers passes 4 KB).
//!
//! Patch 9b (ABI 8): the hero sheet gives the resistance to each state; a state roll and an effect
//! roll record the counterweight their chance met.
//!
//! Triples pass (ABI 10): the resonance tables carry the `triples` body after the pairs (`TABLES_LEN`
//! grew by 1920 bytes); sixteen modifier slots (Blinded, OffGuard, Sundered, Dazed, Silenced,
//! NoHeal, Dulled); an attacker carries the counterweight of its d20 (`blinded_bp`) and a defender
//! the one of its armor against Piercing (`sunder_bp`); `HitResult` has the triple fields
//! (`jump_mods`, `tick_damage`, `remove_mods`, splash `line`, jump `filter_any`).
//!
//! Resonance pass (ABI 9): nine modifier slots (Exposed, Root); a defender carries the crit
//! chance every attacker gets against it.
//!
//! Patch B (ABI 11): a monster may stand on a 2x2 square (`deadroll_rules::Footprint`). A bearer,
//! an AI unit and a unit-mods request carry the unit's `size` (cells per side) at their end; the
//! hit requests take it from the monster's record.
//!
//! Built with `std`: `deadroll-types` pulls `solana-pubkey`, whose `atomic` feature links `std`
//! (`parking_lot`) on non-SBF targets, so `no_std` is not possible here. std on wasm32 aborts on
//! panic and brings its own allocator.

use borsh::{BorshDeserialize, BorshSerialize};
use deadroll_combat::{
    preview_hit, preview_range, resolve_hit, AccuracyBuff, Attacker, DamageRange, Defender,
    HitInput, ModKind, ResonanceTables, TimedMods, VrfRoll, TAGS,
};
use deadroll_rules::{
    aura::{Aura, AuraField, MonsterAuraField, MonsterBearer, PartyAuras, AURAS_PER_PLAYER},
    input::{
        hero, monster_attacker, monster_defender, player_attacker, player_defender, HeroStats,
        SkillStats,
    },
    items::Equipment,
    line::{line_cells, LINE_MAX},
    monster::{record, select_skills, NO_SKILL},
    monster_ai::{plan, AiMonster, AiTarget, AiUnit},
    player::stat_damage_increases,
    BalanceConfig, Footprint, MonsterSkillStats, MonsterTemplate,
};
use deadroll_skillcheck::{
    check_monster_selection,
    monster::{MonsterSkill, POOL_MAX, SESSION_SIZE},
    Category, SkillBlocks, SkillExt, SKILL_ASPECTS_MAX,
};
use deadroll_types::{
    ActiveState, Aspect, Cell, Class, Race, RangeTag, Stat, StateKind, EQUIP_SLOTS, MAP_CELLS,
    MAP_MAX, MONSTERS_MAX, MONSTER_SLOTS, PARTY_MAX, STATES_MAX, STATS_COUNT,
};

/// Bumped on any change of the exports or the byte layouts.
pub const ABI_VERSION: u32 = 11;
/// Size of the input and output buffers.
pub const BUFFER_LEN: usize = 8192;

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
    /// The monster's bestiary record does not decode (an empty slot, or a refused one).
    UnknownMonster = -4,
    /// No resonance tables yet (`set_tables` not called), or they do not decode.
    NoTables = -5,
}

/// The registry resonance tables as `MonsterState` holds them: the `aspects`, `traits`,
/// `resonances` and `triples` bodies, back to back.
pub const TABLES_LEN: usize = deadroll_combat::tables::ASPECTS_LEN
    + deadroll_combat::tables::TRAITS_LEN
    + deadroll_combat::tables::PAIRS_LEN
    + deadroll_combat::tables::TRIPLES_LEN;

/// Decodes the four table bodies (`TABLES_LEN` bytes).
pub fn decode_tables(bytes: &[u8]) -> Result<ResonanceTables, Error> {
    use deadroll_combat::tables::{ASPECTS_LEN, PAIRS_LEN, TRAITS_LEN};
    if bytes.len() != TABLES_LEN {
        return Err(Error::NoTables);
    }
    let (a, rest) = bytes.split_at(ASPECTS_LEN);
    let (t, rest) = rest.split_at(TRAITS_LEN);
    let (p, x) = rest.split_at(PAIRS_LEN);
    ResonanceTables::from_bytes(a, t, p, x).map_err(|_| Error::NoTables)
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

/// A bestiary record (`MonsterState.records`, or the registry `bestiary` table).
pub type RecordInput = [u8; record::SIZE];

fn template(r: &RecordInput) -> Result<MonsterTemplate, Error> {
    MonsterTemplate::from_record(r).ok_or(Error::UnknownMonster)
}

/// A live monster: its bestiary record at a level, with its states and modifiers (its own; the
/// auras come from the party input).
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetInput {
    pub record: RecordInput,
    pub level: u8,
    pub states: [ActiveState; STATES_MAX],
    pub mods: TimedMods,
}

/// A monster as the battle screen and the map builder show it: its bestiary record, a level and
/// the spawn flags (`SPAWN_ELITE`).
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterInput {
    pub record: RecordInput,
    pub level: u8,
    pub flags: u8,
}

/// What the rules make of a monster (`deadroll_rules::MonsterProfile` and its template).
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
    // ABI 6.
    /// HP at spawn at this level (an elite's more), XP for the kill, a boss.
    pub hp_max: u32,
    pub xp: u32,
    pub boss: bool,
    /// `Role` index, cells per side, minimum map tier, action points, initiative.
    pub role: u8,
    pub size: u8,
    pub min_tier: u8,
    pub speed: u8,
    pub initiative: u16,
    /// The natural attack's source and form, its range tag.
    pub source: deadroll_types::Source,
    pub form: deadroll_types::Form,
    pub range: RangeTag,
    /// The skill pool (a run of the registry `monster_skills` table) and the kin.
    pub pool_start: u8,
    pub pool_len: u8,
    pub kin: u8,
    /// Skill slots by `Category` (active, reactive, passive) for these flags.
    pub slots: [u8; Category::COUNT],
    // ABI 7.
    /// Resistance per defense tag (`deadroll_combat::TAGS`: Magical, Combat, Strike,
    /// Projectile), bp before the cap, and the immune tags (`source_bit` / `form_bit`).
    pub tag_resist_bp: [u32; TAGS],
    pub immune_tags: u8,
}

pub fn monster_sheet(m: &MonsterInput) -> Result<MonsterSheet, Error> {
    let t = template(&m.record)?;
    let p = t.profile(m.level, &CFG.monsters);
    Ok(MonsterSheet {
        weapon: p.weapon,
        attack_bonus: p.attack_bonus,
        evasion: p.defense,
        armor: p.armor,
        resist_bp: p.resist_bp,
        immune_aspects: p.immune_aspects,
        immune_states: p.immune_states,
        resist_cap_bp: CFG.combat.resist_cap_bp,
        hp_max: t.hp_at(m.level, m.flags, &CFG.monsters),
        xp: p.xp,
        boss: p.boss,
        role: t.role as u8,
        size: t.size,
        min_tier: t.min_tier,
        speed: deadroll_rules::monster::monster_ap(t.speed, &CFG.monsters),
        initiative: t.initiative,
        source: t.source,
        form: t.form,
        range: deadroll_rules::units::form_range(t.form),
        pool_start: t.pool_start,
        pool_len: t.pool_len,
        kin: t.kin,
        slots: t.slots(m.flags, &CFG.monsters),
        tag_resist_bp: p.tag_resist_bp,
        immune_tags: p.immune_tags,
    })
}

/// A monster skill as `MonsterState.skill_table` holds it (the registry record's bytes `1..17`).
pub type SkillCopy = [u8; SESSION_SIZE];

fn monster_skill_of(copy: &SkillCopy) -> Result<MonsterSkillStats, Error> {
    let k = MonsterSkill::decode_session(copy).ok_or(Error::BadInput)?;
    Ok(MonsterSkillStats::new(&k, &CFG.skills))
}

/// What a monster skill is and costs (`monster_skill`).
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterSkillSheet {
    pub blocks: SkillBlocks,
    pub ext: SkillExt,
    pub kin_only: bool,
    pub shares_bp: [u32; SKILL_ASPECTS_MAX],
    pub power_bp: u32,
    pub cooldown: u8,
    pub ap_cost: u8,
}

pub fn monster_skill(copy: &SkillCopy) -> Result<MonsterSkillSheet, Error> {
    let k = monster_skill_of(copy)?;
    Ok(MonsterSkillSheet {
        blocks: k.stats.blocks,
        ext: k.stats.ext,
        kin_only: k.kin_only,
        shares_bp: k.stats.shares_bp,
        power_bp: k.stats.power_bp,
        cooldown: k.stats.cooldown,
        ap_cost: k.stats.ap_cost,
    })
}

/// A spawn's skill choice in the map builder (`spawn_skills`): its record, flags, the category of
/// each pool skill (`Category` bytes, `pool_len` used) and the chosen pool bits.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnSkillsRequest {
    pub record: RecordInput,
    pub flags: u8,
    pub categories: [u8; POOL_MAX],
    pub mask: u16,
}

/// The builder's verdict (`deadroll_skillcheck::check_monster_selection`, as `finalize_map`
/// checks it) and what fills the slots by default.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpawnSkillsResult {
    /// `SkillError` code + 1, `0` = valid.
    pub error: u8,
    pub slots: [u8; Category::COUNT],
    pub default_mask: u16,
}

pub fn spawn_skills(r: &SpawnSkillsRequest) -> Result<SpawnSkillsResult, Error> {
    let t = template(&r.record)?;
    let len = (t.pool_len as usize).min(POOL_MAX);
    let mut pool = [Category::Active; POOL_MAX];
    for (c, &b) in pool.iter_mut().zip(&r.categories).take(len) {
        *c = Category::try_from(b).map_err(|_| Error::BadInput)?;
    }
    let slots = t.slots(r.flags, &CFG.monsters);
    let error = match check_monster_selection(&pool[..len], r.mask, slots) {
        Ok(()) => 0,
        Err(e) => e as u8 + 1,
    };
    let default_mask = select_skills(&pool[..len], u16::MAX, slots)
        .iter()
        .filter(|&&i| i != NO_SKILL)
        .fold(0u16, |m, &i| m | 1 << i);
    Ok(SpawnSkillsResult {
        error,
        slots,
        default_mask,
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
    /// ABI 7: a Resistance aura's aspect weights (`GameSession.aura_weights`,
    /// `MonsterState.aura_weights`).
    pub weights_pct: [u8; Aspect::COUNT],
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
            kin_only: false,
            weights_pct: a.weights_pct,
        }
    }
}

/// A living monster with an aura (ABI 6): `MonsterState.auras` (`MonsterAuraData`: the aura and
/// its kin-only flag), its cell and kin.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BearerInput {
    pub index: u8,
    pub cell: Cell,
    pub kin: u8,
    pub auras: [(AuraInput, bool); AURAS_PER_PLAYER],
    /// ABI 11: cells per side (the record's `size`; 0 counts as 1).
    pub size: u8,
}

/// Patch B: the square of a monster of template `t` whose top-left cell is `at`.
fn square(at: Cell, t: &MonsterTemplate) -> Footprint {
    Footprint::new(at, t.size)
}

/// The party's auras as the session holds them (`GameSession.auras`, `player_pos`,
/// `alive_mask`), and the monsters' (ABI 6, the first `bearer_count` of `bearers`): what the
/// auras over a unit add (docs/SKILL_BUILDER.md 3.3, 3.5).
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PartyInput {
    pub auras: [[AuraInput; AURAS_PER_PLAYER]; PARTY_MAX],
    pub cells: [Cell; PARTY_MAX],
    pub alive: u8,
    pub bearers: [BearerInput; MONSTERS_MAX],
    pub bearer_count: u8,
}

/// A `PartyInput` as the rules see it (`AuraField` borrows it).
struct PartyView {
    auras: PartyAuras,
    cells: [Cell; PARTY_MAX],
    alive: u8,
    bearers: Vec<MonsterBearer>,
}

impl PartyView {
    fn field(&self) -> AuraField<'_> {
        AuraField {
            auras: &self.auras,
            cells: &self.cells,
            alive: self.alive,
            monsters: MonsterAuraField {
                bearers: &self.bearers,
            },
        }
    }
}

impl PartyInput {
    fn view(&self) -> PartyView {
        let n = (self.bearer_count as usize).min(MONSTERS_MAX);
        PartyView {
            auras: self.auras.map(|p| p.map(Aura::from)),
            cells: self.cells,
            alive: self.alive,
            bearers: self.bearers[..n]
                .iter()
                .map(|b| MonsterBearer {
                    index: b.index,
                    cell: b.cell,
                    kin: b.kin,
                    auras: b.auras.map(|(a, kin_only)| Aura {
                        kin_only,
                        ..Aura::from(a)
                    }),
                    size: b.size,
                })
                .collect(),
        }
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
    /// ABI 6: the target's monster index (its own auras).
    pub target_index: u8,
}

/// A monster's natural attack (`skill = None`) or damage skill on a player
/// (`preview_monster_attack`), as the session program builds it.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterAttackRequest {
    pub monster: TargetInput,
    pub index: u8,
    pub at: Cell,
    pub skill: Option<SkillCopy>,
    pub hero: HeroInput,
    pub hero_states: [ActiveState; STATES_MAX],
    pub hero_mods: TimedMods,
    pub slot: u8,
    pub party: PartyInput,
}

pub fn monster_attack_input(r: &MonsterAttackRequest) -> Result<HitInput, Error> {
    let t = template(&r.monster.record)?;
    let profile = t.profile(r.monster.level, &CFG.monsters);
    let skill = r.skill.as_ref().map(monster_skill_of).transpose()?;
    let view = r.party.view();
    let field = view.field();
    let monster_mods = field.monster_mods(
        &r.monster.mods,
        r.index,
        square(r.at, &t),
        profile.kin,
        skill.is_none(),
    );
    let at = r
        .party
        .cells
        .get(r.slot as usize)
        .copied()
        .unwrap_or_default();
    let hero_mods = field.hero_mods(&r.hero_mods, r.slot, at, false);
    let attacker = monster_attacker(
        CFG,
        deadroll_rules::MONSTER_UNIT_BASE + r.index,
        &profile,
        skill.as_ref().map(|k| &k.stats),
        &monster_mods,
    );
    let defender = player_defender(CFG, &r.hero.hero(), &r.hero_states, &hero_mods);
    Ok(HitInput { attacker, defender })
}

/// A unit as the monster AI sees it (`deadroll_rules::monster_ai::AiUnit`).
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AiUnitInput {
    pub cell: Cell,
    pub hp: u32,
    pub hp_max: u32,
    pub alive: bool,
    pub states: [ActiveState; STATES_MAX],
    pub mods: TimedMods,
    pub kin: u8,
    /// ABI 11: cells per side (players 1).
    pub size: u8,
}

impl From<AiUnitInput> for AiUnit {
    fn from(u: AiUnitInput) -> Self {
        AiUnit {
            cell: u.cell,
            hp: u.hp,
            hp_max: u.hp_max,
            alive: u.alive,
            states: u.states,
            mods: u.mods,
            kin: u.kin,
            size: u.size,
        }
    }
}

/// The auto mode's question (`monster_plan`): monster `index` (its record, level, skills by slot
/// from `MonsterState`, cooldowns, action points) among the players and the monsters (the first
/// `monster_count`).
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterPlanRequest {
    pub record: RecordInput,
    pub level: u8,
    pub index: u8,
    pub ap: u8,
    pub skills: [Option<SkillCopy>; MONSTER_SLOTS],
    pub cooldowns: [u8; MONSTER_SLOTS],
    pub heroes: [AiUnitInput; PARTY_MAX],
    pub hero_count: u8,
    pub monsters: [AiUnitInput; MONSTERS_MAX],
    pub monster_count: u8,
}

/// The AI's choice: `found`, the skill slot (`None` = the natural attack), the target (a player
/// slot, or a monster index when `ally`), the reach and the AP cost.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonsterPlan {
    pub found: bool,
    pub slot: Option<u8>,
    pub ally: bool,
    pub target: u8,
    pub range: RangeTag,
    pub ap: u8,
}

pub fn monster_plan(r: &MonsterPlanRequest) -> Result<MonsterPlan, Error> {
    let t = template(&r.record)?;
    let profile = t.profile(r.level, &CFG.monsters);
    let mut skills = [None; MONSTER_SLOTS];
    for (out, copy) in skills.iter_mut().zip(&r.skills) {
        *out = copy.as_ref().map(monster_skill_of).transpose()?;
    }
    let heroes: Vec<AiUnit> = r.heroes[..(r.hero_count as usize).min(PARTY_MAX)]
        .iter()
        .map(|&u| u.into())
        .collect();
    let monsters: Vec<AiUnit> = r.monsters[..(r.monster_count as usize).min(MONSTERS_MAX)]
        .iter()
        .map(|&u| u.into())
        .collect();
    let me = AiMonster {
        index: r.index,
        ap: r.ap,
        profile: &profile,
        skills: &skills,
        cooldowns: &r.cooldowns,
    };
    Ok(match plan(CFG, &me, &heroes, &monsters) {
        None => MonsterPlan::default(),
        Some(c) => {
            let (ally, target) = match c.target {
                AiTarget::Player(s) => (false, s),
                AiTarget::Monster(i) => (true, i),
            };
            MonsterPlan {
                found: true,
                slot: c.slot,
                ally,
                target,
                range: c.range,
                ap: c.ap,
            }
        }
    })
}

/// A unit's buffs and debuffs with the auras over it (`unit_mods`): a player slot, or monster
/// `slot` of kin `kin` at `at` (`monster`). `basic_attack`: the view of a basic or natural
/// attack, which no passive Empowerment strengthens.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitModsRequest {
    pub party: PartyInput,
    pub mods: TimedMods,
    pub monster: bool,
    pub slot: u8,
    pub at: Cell,
    pub basic_attack: bool,
    pub kin: u8,
    /// ABI 11: a monster's cells per side.
    pub size: u8,
}

pub fn unit_mods(r: &UnitModsRequest) -> TimedMods {
    let view = r.party.view();
    let field = view.field();
    if r.monster {
        field.monster_mods(
            &r.mods,
            r.slot,
            Footprint::new(r.at, r.size),
            r.kin,
            r.basic_attack,
        )
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
    // ABI 7.
    /// Resistance per aspect (items), bp before the cap, and the state resistance (Endurance and
    /// items), the counterweight on every state and effect chance against the hero.
    pub resist_bp: [u32; Aspect::COUNT],
    pub state_resist_bp: u32,
    // ABI 8.
    /// Resistance to each state, bp by `StateKind`: `state_resist_bp` plus the items'
    /// resistance to that state.
    pub state_resist_by_kind_bp: [u32; StateKind::COUNT],
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
        resist_bp: hero::resist(CFG, h),
        state_resist_bp: hero::state_resist(CFG, h),
        state_resist_by_kind_bp: hero::state_resists(CFG, h),
    }
}

/// The hit input of a player's attack on a monster, as the session program builds it.
pub fn attack_input(r: &AttackRequest) -> Result<HitInput, Error> {
    let t = template(&r.target.record)?;
    let profile = t.profile(r.target.level, &CFG.monsters);
    let skill = r.skill.map(|s| s.stats());
    let view = r.party.view();
    let field = view.field();
    let at = r
        .party
        .cells
        .get(r.slot as usize)
        .copied()
        .unwrap_or_default();
    let hero_mods = field.hero_mods(&r.hero_mods, r.slot, at, skill.is_none());
    let target_mods = field.monster_mods(
        &r.target.mods,
        r.target_index,
        square(r.target_at, &t),
        profile.kin,
        false,
    );
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

/// Decodes a 16-byte skill copy, writes a borsh `MonsterSkillSheet`.
pub fn monster_skill_bytes(input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let copy = SkillCopy::try_from_slice(input).map_err(|_| Error::BadInput)?;
    write(&monster_skill(&copy)?, out)
}

/// Decodes a borsh `SpawnSkillsRequest`, writes a borsh `SpawnSkillsResult`.
pub fn spawn_skills_bytes(input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let request = SpawnSkillsRequest::try_from_slice(input).map_err(|_| Error::BadInput)?;
    write(&spawn_skills(&request)?, out)
}

/// Decodes a borsh `MonsterAttackRequest`, writes a borsh `HitPreview`.
pub fn preview_monster_attack_bytes(
    input: &[u8],
    tables: &ResonanceTables,
    out: &mut [u8],
) -> Result<usize, Error> {
    let request = MonsterAttackRequest::try_from_slice(input).map_err(|_| Error::BadInput)?;
    write(
        &preview_hit(&monster_attack_input(&request)?, &CFG.combat, tables),
        out,
    )
}

/// Decodes a borsh `MonsterPlanRequest`, writes a borsh `MonsterPlan`.
pub fn monster_plan_bytes(input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let request = MonsterPlanRequest::try_from_slice(input).map_err(|_| Error::BadInput)?;
    write(&monster_plan(&request)?, out)
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

    /// ABI 6. A monster skill: its 16-byte session copy in, `MonsterSkillSheet` out.
    #[no_mangle]
    pub extern "C" fn monster_skill(input_len: u32) -> i32 {
        with_input(input_len, |input, _, out| monster_skill_bytes(input, out))
    }

    /// A spawn's skill choice: `SpawnSkillsRequest` in, `SpawnSkillsResult` out.
    #[no_mangle]
    pub extern "C" fn spawn_skills(input_len: u32) -> i32 {
        with_input(input_len, |input, _, out| spawn_skills_bytes(input, out))
    }

    /// A monster's attack on a player before the dice: `MonsterAttackRequest` in, `HitPreview` out.
    #[no_mangle]
    pub extern "C" fn preview_monster_attack(input_len: u32) -> i32 {
        with_tables(input_len, |input, _, t, out| {
            preview_monster_attack_bytes(input, t, out)
        })
    }

    /// The monster AI's choice for the auto mode: `MonsterPlanRequest` in, `MonsterPlan` out.
    #[no_mangle]
    pub extern "C" fn monster_plan(input_len: u32) -> i32 {
        with_input(input_len, |input, _, out| monster_plan_bytes(input, out))
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
        [r.aspects, r.traits, r.resonances, r.triples].concat()
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
        weights_pct: [0; Aspect::COUNT],
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

    /// The shipped bestiary record of `id` (as `MonsterState.records` holds it).
    fn record_of(id: u8) -> RecordInput {
        registry().record(id).unwrap()
    }

    fn request(skill: Option<SkillInput>, id: u8) -> AttackRequest {
        AttackRequest {
            hero: warrior(),
            hero_mods: NO_MODS,
            skill,
            target: TargetInput {
                record: record_of(id),
                level: 1,
                states: [ActiveState::default(); STATES_MAX],
                mods: NO_MODS,
            },
            accuracy: AccuracyBuff::default(),
            party: PartyInput::default(),
            slot: 0,
            target_at: Cell::default(),
            target_index: 0,
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
    fn monster_sheet_reads_the_given_record() {
        let wolf = MonsterInput {
            record: record_of(0),
            level: 1,
            flags: 0,
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
        let t = registry().template(0).unwrap();
        assert_eq!(sheet.hp_max, t.hp_at(1, 0, &CFG.monsters));
        assert_eq!((sheet.pool_start, sheet.pool_len, sheet.kin), (0, 4, t.kin));
        assert_eq!(sheet.slots, [3, 0, 1]);
        assert!(!sheet.boss);
        let elite = MonsterInput {
            flags: deadroll_types::SPAWN_ELITE,
            ..wolf
        };
        let e = monster_sheet(&elite).unwrap();
        assert!(e.hp_max > sheet.hp_max);
        assert_eq!(e.slots, [4, 0, 1]);
        let mut bare = wolf;
        bare.record[record::RESIST] = 0;
        assert_eq!(monster_sheet(&bare).unwrap().resist_bp, [0; Aspect::COUNT]);
        let unknown = MonsterInput {
            record: [0; record::SIZE],
            ..wolf
        };
        assert_eq!(monster_sheet(&unknown), Err(Error::UnknownMonster));
    }

    /// The session copy of the shipped pool skill named `name`.
    fn skill_copy(name: &str) -> SkillCopy {
        let reg = registry();
        let i = reg.skill_names.iter().position(|n| n == name).unwrap();
        reg.skills[i].unwrap().encode_session()
    }

    #[test]
    fn monster_skills_and_spawn_choices() {
        let mut out = [0u8; BUFFER_LEN];
        let copy = skill_copy("Mending");
        let len = monster_skill_bytes(&borsh::to_vec(&copy).unwrap(), &mut out).unwrap();
        let sheet = MonsterSkillSheet::try_from_slice(&out[..len]).unwrap();
        assert_eq!(sheet.blocks.effect, deadroll_skillcheck::Effect::Heal);
        assert!(sheet.kin_only && sheet.ap_cost > 0);
        assert_eq!(
            monster_skill_bytes(&[9; 16], &mut out),
            Err(Error::BadInput)
        );
        // The wolf: four actives in the pool, three regular slots.
        let mut r = SpawnSkillsRequest {
            record: record_of(0),
            flags: 0,
            categories: [0; POOL_MAX],
            mask: 0b0111,
        };
        let check = |r: &SpawnSkillsRequest| {
            let mut out = [0u8; BUFFER_LEN];
            let len = spawn_skills_bytes(&borsh::to_vec(r).unwrap(), &mut out).unwrap();
            SpawnSkillsResult::try_from_slice(&out[..len]).unwrap()
        };
        let ok = check(&r);
        assert_eq!(
            (ok.error, ok.slots, ok.default_mask),
            (0, [3, 0, 1], 0b0111)
        );
        r.mask = 0b1111;
        let count = deadroll_skillcheck::SkillError::MonsterSkillCount as u8 + 1;
        assert_eq!(check(&r).error, count);
        r.flags = deadroll_types::SPAWN_ELITE;
        assert_eq!(check(&r).error, 0);
        r.mask = 0b1_0000;
        let unknown = deadroll_skillcheck::SkillError::MonsterSkillUnknown as u8 + 1;
        assert_eq!(check(&r).error, unknown);
    }

    #[test]
    fn the_monster_attack_preview_is_the_program_input() {
        let tables = registry().tables;
        let mut out = [0u8; BUFFER_LEN];
        for skill in [None, Some(skill_copy("Dagger Throw"))] {
            let r = MonsterAttackRequest {
                monster: TargetInput {
                    record: record_of(2),
                    level: 1,
                    states: [ActiveState::default(); STATES_MAX],
                    mods: NO_MODS,
                },
                index: 0,
                at: Cell { x: 3, y: 2 },
                skill,
                hero: warrior(),
                hero_states: [ActiveState::default(); STATES_MAX],
                hero_mods: NO_MODS,
                slot: 0,
                party: PartyInput::default(),
            };
            let len = preview_monster_attack_bytes(&borsh::to_vec(&r).unwrap(), &tables, &mut out)
                .unwrap();
            let p = HitPreview::try_from_slice(&out[..len]).unwrap();
            let profile = registry().template(2).unwrap().profile(1, &CFG.monsters);
            let k = skill.map(|c| monster_skill_of(&c).unwrap());
            let expected = HitInput {
                attacker: monster_attacker(
                    CFG,
                    deadroll_rules::MONSTER_UNIT_BASE,
                    &profile,
                    k.as_ref().map(|k| &k.stats),
                    &NO_MODS,
                ),
                defender: player_defender(
                    CFG,
                    &warrior().hero(),
                    &[ActiveState::default(); STATES_MAX],
                    &NO_MODS,
                ),
            };
            assert_eq!(
                p,
                deadroll_combat::preview_hit(&expected, &CFG.combat, &tables)
            );
        }
    }

    #[test]
    fn the_monster_plan_heals_a_hurt_kin_ally() {
        let reg = registry();
        let shaman = reg.template(4).unwrap();
        let unit = |x, hp| AiUnitInput {
            cell: Cell { x, y: 5 },
            hp,
            hp_max: 10,
            alive: true,
            kin: shaman.kin,
            ..AiUnitInput::default()
        };
        let mut skills = [None; MONSTER_SLOTS];
        skills[0] = Some(skill_copy("Spark"));
        skills[1] = Some(skill_copy("Mending"));
        let mut r = MonsterPlanRequest {
            record: record_of(4),
            level: 1,
            index: 0,
            ap: 3,
            skills,
            cooldowns: [0; MONSTER_SLOTS],
            heroes: [AiUnitInput::default(); PARTY_MAX],
            hero_count: 1,
            monsters: [AiUnitInput::default(); MONSTERS_MAX],
            monster_count: 2,
        };
        r.heroes[0] = unit(9, 10);
        r.monsters[0] = unit(5, 10);
        r.monsters[1] = unit(6, 2);
        let plan_of = |r: &MonsterPlanRequest| {
            let mut out = [0u8; BUFFER_LEN];
            let bytes = borsh::to_vec(r).unwrap();
            assert!(bytes.len() < BUFFER_LEN);
            let len = monster_plan_bytes(&bytes, &mut out).unwrap();
            MonsterPlan::try_from_slice(&out[..len]).unwrap()
        };
        let p = plan_of(&r);
        assert!(p.found && p.ally);
        assert_eq!((p.slot, p.target), (Some(1), 1));
        // Nobody hurt: Spark on the player.
        r.monsters[1].hp = 10;
        let p = plan_of(&r);
        assert_eq!((p.slot, p.ally, p.target), (Some(0), false, 0));
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
            let profile = MonsterTemplate::from_record(&r.target.record)
                .unwrap()
                .profile(1, &CFG.monsters);
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
        unknown.target.record = [0; record::SIZE];
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
            weights_pct: [0; Aspect::COUNT],
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
                kin: 0,
                size: 1,
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
