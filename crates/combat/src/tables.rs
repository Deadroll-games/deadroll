//! Resonance tables as data (docs/DAMAGE_TYPES_AND_MIXES.md, sections 6 and 10.1).
//!
//! The registry stores four tables; this module owns their byte format, so the registry
//! (validation), the session (the copy taken at `open_session`), the simulator and the website
//! (WASM) all decode the same bytes with the same code.
//!
//! | Table        | Record | Bytes                                                                 |
//! |--------------|--------|-----------------------------------------------------------------------|
//! | `aspects`    | 8      | aspect, family, flags, native state (`NO_STATE`), trait, condition, 0, 0 |
//! | `traits`     | 32     | present (1), 0, hit op, template op 0, template op 1, 12 x 0           |
//! | `resonances` | 16     | condition, flags (`PAIR_OVERRIDE`), op 0, op 1, 0, 0                    |
//! | `triples`    | 16     | 0, flags (`TRIPLE_SET`), 3 compact ops (4 bytes each), 0, 0           |
//!
//! An op is 6 bytes: `kind | CRIT_ONLY`, `arg`, `value` (u32 LE). `arg` is a state, a modifier
//! kind or an aspect depending on the kind, and [`NATIVE`] stands for the base's native state.
//! `value` is a scale in bp (`BP` = x1) of the `Balance` number the kind names, or a count
//! (`NativeFaster` ticks, `JumpChain` targets, `Knockback` cells). A triple record packs an op
//! in 4 bytes (`kind | CRIT_ONLY`, `arg`, `value` as u16 LE), so its values stop at 65_535.
//!
//! Three ops carry two numbers in `value`: `Delayed` and `ModAfterState` (low byte: the state
//! they wait for or follow, bits 8..: a strength in percent, 0 = 100) and `ChainRadius` (low
//! byte: the radius, bits 8..16: the state a target must have, `ANY_STATE` = any).
//!
//! Pair mechanics are generated from the base's native state and the tint's trait template;
//! a pair record with `PAIR_OVERRIDE` replaces them (section 10.1). An empty record is neutral
//! (section 10.2).

use deadroll_types::{Aspect, Family, StateKind};

use crate::resonance::ResonanceCondition;
use crate::support::ModKind;

/// Bytes of one op.
pub const OP_SIZE: usize = 6;
pub const ASPECT_RECORD: usize = 8;
pub const TRAIT_RECORD: usize = 32;
pub const PAIR_RECORD: usize = 16;
pub const TRIPLE_RECORD: usize = 16;
/// One tint trait per aspect, in `Aspect` order.
pub const TRAITS: usize = Aspect::COUNT;
pub const ASPECTS_LEN: usize = Aspect::COUNT * ASPECT_RECORD;
pub const TRAITS_LEN: usize = TRAITS * TRAIT_RECORD;
pub const PAIRS_LEN: usize = Aspect::COUNT * Aspect::COUNT * PAIR_RECORD;
/// Ordered triples of three different aspects: `6 x 5 x 4`.
pub const TRIPLES: usize = Aspect::COUNT * (Aspect::COUNT - 1) * (Aspect::COUNT - 2);
pub const TRIPLES_LEN: usize = TRIPLES * TRIPLE_RECORD;
/// Bytes of one op in a triple record.
pub const TRIPLE_OP_SIZE: usize = 4;
/// Ops of a pair record and of a trait template.
pub const PAIR_OPS: usize = 2;
pub const TEMPLATE_OPS: usize = 2;
/// Ops of a triple record: they replace the second tint's trait template.
pub const TRIPLE_OPS: usize = 3;

/// `arg`: the native state of the resonance's base.
pub const NATIVE: u8 = 0xFF;
/// `arg` of a `Multiplier`: no state required on the target.
pub const ANY_STATE: u8 = 0xFF;
/// Aspect record: the aspect has no native state.
pub const NO_STATE: u8 = 0xFF;
/// High bit of the op kind byte: the op acts only on a crit.
pub const CRIT_ONLY: u8 = 0x80;
/// Pair record flags: the record overrides the generated mechanics.
pub const PAIR_OVERRIDE: u8 = 1;
/// Triple record flags: the record has ops (an empty record means the second tint's trait
/// template is used, as before triples).
pub const TRIPLE_SET: u8 = 1;

/// What a resonance op does. Discriminants are the on-chain format: append only.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum OpKind {
    #[default]
    None,
    /// "More" on the base packet, `resonance_more_bp[tier] x value`. `arg`: a state the target
    /// must have before the hit, or `ANY_STATE`.
    Multiplier,
    /// On a crit, `arg` is applied without a roll.
    StateOnCrit,
    /// An independent roll for `arg`: `extra_state_chance_pct x value`, stat-scaled.
    ExtraStateChance,
    /// When the native state lands, `arg` lands too, strength from the base packet x `value`.
    Linked,
    /// When the native state lands, a roll for `arg`: `extra_state_chance_pct x value`,
    /// stat-scaled.
    LinkedChance,
    /// When the native state lands, a timed modifier `arg` (`ModKind`) on the target,
    /// `res_mod_bp[tier] x value`.
    LinkedMod,
    /// The native state lasts `res_turns[tier] x value` more turns.
    NativeTurns,
    /// The native state is stronger by `res_power_bp[tier] x value`.
    NativePower,
    /// The native state ticks `value` times per turn: power x value, duration / value.
    NativeFaster,
    /// The native state is stronger by `res_power_bp[tier] x value`, scaled by the target's
    /// armor: `armor / (armor + armor_k)`.
    NativePowerArmor,
    /// When Freeze lands, it deals `value` of the base packet (tier-scaled) when it thaws.
    ThawDamage,
    /// After the hit, `arg` is removed from the target.
    RemoveState,
    /// When `arg` lands, it also lands on one adjacent enemy of the target.
    Jump,
    /// When the native state lands, it jumps along adjacent enemies that have `arg`, up to
    /// `value` of them.
    JumpChain,
    /// When `arg` lands, it also lands on the enemy right behind the target.
    JumpLine,
    /// The target is pushed `value` cells away from the attacker.
    Knockback,
    /// The cells around the target burn: Burning of `value` of the base packet (tier-scaled)
    /// on whoever steps in.
    IgniteCells,
    /// Addition: a flat packet of aspect `arg`, `tint_added[tier] x value`.
    AddPacket,
    /// Armor penetration on this hit, `keenness_pen_bp[tier] x value`.
    Penetrate,
    /// The attacker's next attacks with an `arg` packet get `res_accuracy[tier] x value` to the
    /// d20.
    AccuracyBuff,
    /// The hit becomes a crit (the crit-only ops of the hit act too).
    ForceCrit,
    /// `arg` lands without a roll, strength from the base packet x `value`.
    ApplyState,
    /// A timed modifier `arg` (`ModKind`) on the target and on every enemy this resonance's
    /// jumps reach.
    ApplyMod,
    /// The target had the base's native state before the hit: `arg` lands without a roll,
    /// strength from the base packet x `value`.
    StateIfHad,
    /// The target had `arg` before the hit (or `ANY_STATE`): every enemy adjacent to it takes
    /// `value` bp of the hit's damage.
    Splash,
    /// When the native state lands, it and a timed modifier `arg` (`ModKind`) land on every
    /// enemy within `value` cells of the target.
    Nova,
    /// When the native state lands, a timed modifier `arg` (`ModKind`) on the target that lasts
    /// `res_mod_turns` beyond it.
    ModAfterNative,
    /// Triples. The live state `arg` (Burning or Bleeding) ticks `value` times per turn: power x
    /// value, duration / value.
    StateFaster,
    /// The live state `arg` lasts `value` times longer (capped by `Balance::state_turns_max`); a
    /// damage over time ticks with its power / `value`.
    StateSlower,
    /// The live damage over time `arg` ticks now, `value` times (0 = once), on top of the hit.
    TickNow,
    /// The live damage over time `arg` grows by `Balance::escalate_step_bp` after every tick.
    Escalate,
    /// The damage over time of this hit (the native one, else the Bleeding or Burning that
    /// landed) is dealt as aspect `arg`: its power is rebuilt from the rolled damage through that
    /// aspect's resistance and vulnerability, not the armor.
    TickAspect,
    /// After the hit, the modifier `arg` is removed from the target.
    RemoveMod,
    /// The native state lasts `value` times shorter (at least 1 turn).
    NativeShorter,
    /// `arg` starts when the control on the target ends: it lands without a roll (strength from
    /// the base packet x the percent in bits 8.. of `value`, 0 = 100) and waits while the target
    /// is frozen or stunned. The low byte of `value` names the state it waits for; when the
    /// target has none, `arg` lands at once.
    Delayed,
    /// A chance roll for `arg` even when the attack misses (`extra_state_chance_pct x value`,
    /// stat-scaled). Rolled last.
    OnMiss,
    /// When a Freeze with thaw damage landed, the neighbours of the target take that damage now.
    ThawSplash,
    /// The splash of this hit goes only to the enemies in line with the attacker and the target.
    SplashLine,
    /// `arg` (it must have landed) lands on every enemy within the radius (low byte of `value`)
    /// that has the state in bits 8..16 of `value` (`ANY_STATE` = any).
    ChainRadius,
    /// When the state in the low byte of `value` landed, a timed modifier `arg` (`ModKind`) on
    /// the target that lasts `res_mod_turns` beyond it, strength x the percent in bits 8..
    /// (0 = 100).
    ModAfterState,
}

impl OpKind {
    pub const COUNT: u8 = 41;

    pub const fn from_u8(v: u8) -> Option<OpKind> {
        use OpKind::*;
        Some(match v {
            0 => None,
            1 => Multiplier,
            2 => StateOnCrit,
            3 => ExtraStateChance,
            4 => Linked,
            5 => LinkedChance,
            6 => LinkedMod,
            7 => NativeTurns,
            8 => NativePower,
            9 => NativeFaster,
            10 => NativePowerArmor,
            11 => ThawDamage,
            12 => RemoveState,
            13 => Jump,
            14 => JumpChain,
            15 => JumpLine,
            16 => Knockback,
            17 => IgniteCells,
            18 => AddPacket,
            19 => Penetrate,
            20 => AccuracyBuff,
            21 => ForceCrit,
            22 => ApplyState,
            23 => ApplyMod,
            24 => StateIfHad,
            25 => Splash,
            26 => Nova,
            27 => ModAfterNative,
            28 => StateFaster,
            29 => StateSlower,
            30 => TickNow,
            31 => Escalate,
            32 => TickAspect,
            33 => RemoveMod,
            34 => NativeShorter,
            35 => Delayed,
            36 => OnMiss,
            37 => ThawSplash,
            38 => SplashLine,
            39 => ChainRadius,
            40 => ModAfterState,
            _ => return Option::None,
        })
    }

    /// What `arg` names for this kind.
    pub const fn arg(self) -> ArgKind {
        use OpKind::*;
        match self {
            None | NativeTurns | NativePower | NativeFaster | NativePowerArmor | ThawDamage
            | Knockback | IgniteCells | Penetrate | ForceCrit | NativeShorter | ThawSplash
            | SplashLine => ArgKind::Zero,
            Multiplier | Splash => ArgKind::StateOrAny,
            StateOnCrit | ExtraStateChance | Linked | LinkedChance | RemoveState | Jump
            | JumpLine | ApplyState | StateIfHad => ArgKind::State,
            JumpChain | StateSlower | OnMiss | ChainRadius => ArgKind::StateOnly,
            StateFaster | TickNow | Escalate | Delayed => ArgKind::Dot,
            LinkedMod | ApplyMod | Nova | ModAfterNative | RemoveMod | ModAfterState => {
                ArgKind::Mod
            }
            AddPacket | AccuracyBuff | TickAspect => ArgKind::Aspect,
        }
    }
}

/// How an op's `arg` byte reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    /// Unused, must be 0.
    Zero,
    /// A `StateKind` or `NATIVE`.
    State,
    /// A `StateKind` only.
    StateOnly,
    /// Burning or Bleeding.
    Dot,
    /// A `StateKind` or `ANY_STATE`.
    StateOrAny,
    /// A `ModKind`.
    Mod,
    /// An `Aspect`.
    Aspect,
}

/// One resonance op.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Op {
    pub kind: OpKind,
    pub crit_only: bool,
    pub arg: u8,
    pub value: u32,
}

/// Why table bytes do not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableError {
    /// A table is shorter than its records.
    Short,
    /// Record `index` of the table is malformed.
    Record(u8),
}

/// The two-number values of `Delayed`, `ModAfterState` and `ChainRadius` name states.
fn value_ok(kind: OpKind, value: u32) -> bool {
    let state = |v: u32| (v as usize) < StateKind::COUNT;
    match kind {
        OpKind::Delayed | OpKind::ModAfterState => state(value & 0xFF) && value >> 16 == 0,
        OpKind::ChainRadius => {
            value >> 16 == 0
                && value & 0xFF > 0
                && (state(value >> 8 & 0xFF) || (value >> 8 & 0xFF) as u8 == ANY_STATE)
        }
        _ => true,
    }
}

impl Op {
    pub const NONE: Op = Op {
        kind: OpKind::None,
        crit_only: false,
        arg: 0,
        value: 0,
    };

    pub const fn new(kind: OpKind, arg: u8, value: u32) -> Op {
        Op {
            kind,
            crit_only: false,
            arg,
            value,
        }
    }

    pub const fn on_crit(self) -> Op {
        Op {
            crit_only: true,
            ..self
        }
    }

    pub fn is_none(&self) -> bool {
        self.kind == OpKind::None
    }

    /// `None` when malformed.
    pub fn decode(b: &[u8]) -> Option<Op> {
        if b.len() < OP_SIZE {
            return None;
        }
        let kind = OpKind::from_u8(b[0] & !CRIT_ONLY)?;
        let crit_only = b[0] & CRIT_ONLY != 0;
        let arg = b[1];
        let value = u32::from_le_bytes([b[2], b[3], b[4], b[5]]);
        if kind == OpKind::None {
            return (b[..OP_SIZE] == [0; OP_SIZE]).then_some(Op::NONE);
        }
        let state = |a: u8| (a as usize) < StateKind::COUNT;
        let arg_ok = match kind.arg() {
            ArgKind::Zero => arg == 0,
            ArgKind::State => arg == NATIVE || state(arg),
            ArgKind::StateOnly => state(arg),
            ArgKind::Dot => {
                arg == StateKind::Burning as u8 || arg == StateKind::Bleeding as u8
            }
            ArgKind::StateOrAny => arg == ANY_STATE || state(arg),
            // A resonance has no aspect weights to give a Resistance.
            ArgKind::Mod => (arg as usize) < ModKind::COUNT && arg != ModKind::Resistance as u8,
            ArgKind::Aspect => (arg as usize) < Aspect::COUNT,
        };
        if !arg_ok || !value_ok(kind, value) {
            return None;
        }
        Some(Op {
            kind,
            crit_only,
            arg,
            value,
        })
    }

    /// The low byte of `value`: the state `Delayed` and `ModAfterState` name, the radius of
    /// `ChainRadius`.
    pub const fn low(&self) -> u8 {
        self.value as u8
    }

    /// The state `ChainRadius` filters by (`ANY_STATE` = any).
    pub const fn filter(&self) -> u8 {
        (self.value >> 8) as u8
    }

    /// The percent in bits 8.. of `value` (`Delayed`, `ModAfterState`), `BP` = 100%.
    pub const fn percent_bp(&self) -> u32 {
        match self.value >> 8 {
            0 => 10_000,
            p => p * 100,
        }
    }

    pub fn encode(&self) -> [u8; OP_SIZE] {
        let v = self.value.to_le_bytes();
        let kind = self.kind as u8 | if self.crit_only { CRIT_ONLY } else { 0 };
        [kind, self.arg, v[0], v[1], v[2], v[3]]
    }

    /// The state `arg` names, with `NATIVE` read as `native`.
    pub fn state(&self, native: Option<StateKind>) -> Option<StateKind> {
        if self.arg == NATIVE {
            native
        } else {
            StateKind::try_from(self.arg).ok()
        }
    }
}

/// One aspect of the registry (section 10.1: id, family, native state).
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AspectEntry {
    pub family: Family,
    pub flags: u8,
    pub native: Option<StateKind>,
    /// Index into the traits table.
    pub trait_id: u8,
    /// Condition of the generated pairs with this base.
    pub condition: ResonanceCondition,
}

/// What an aspect brings when it acts as a tint (section 3.2).
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TraitEntry {
    pub present: bool,
    /// Acts on the hit whenever the tint triggers (Fury's added Fire, Keenness).
    pub hit: Op,
    /// The trait applied to a base's native state: the generated pair, and the second tint of
    /// a triple (section 6.5).
    pub template: [Op; TEMPLATE_OPS],
}

/// A pair record (section 6.4).
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PairEntry {
    pub condition: ResonanceCondition,
    /// `PAIR_OVERRIDE`: these ops replace the generated ones.
    pub set: bool,
    pub ops: [Op; PAIR_OPS],
}

/// The mechanics of one pair: overridden, or generated by the rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PairMechanics {
    pub condition: ResonanceCondition,
    pub ops: [Op; PAIR_OPS],
    pub generated: bool,
}

/// A triple record (docs/DAMAGE_TYPES_AND_MIXES.md, section 6.5): base, best tint, second tint.
/// Its ops replace the second tint's trait template.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TripleEntry {
    pub set: bool,
    pub ops: [Op; TRIPLE_OPS],
}

/// Place of the ordered triple (`base`, `tint`, `second`) in the table: `None` unless the three
/// aspects differ. Base-major, then tint, then second, each in `Aspect` order among the
/// aspects still free.
pub const fn triple_index(base: Aspect, tint: Aspect, second: Aspect) -> Option<usize> {
    let (b, t, s) = (base as usize, tint as usize, second as usize);
    if b == t || b == s || t == s {
        return None;
    }
    let t = t - (t > b) as usize;
    let s = s - (s > b) as usize - (s > tint as usize) as usize;
    Some((b * (Aspect::COUNT - 1) + t) * (Aspect::COUNT - 2) + s)
}

/// The aspects of triple `index` (the inverse of [`triple_index`]).
pub fn triple_of(index: usize) -> Option<[Aspect; 3]> {
    if index >= TRIPLES {
        return None;
    }
    let s = index % (Aspect::COUNT - 2);
    let t = index / (Aspect::COUNT - 2) % (Aspect::COUNT - 1);
    let b = index / ((Aspect::COUNT - 1) * (Aspect::COUNT - 2));
    let tint = t + (t >= b) as usize;
    let mut second = s;
    for taken in [b.min(tint), b.max(tint)] {
        if second >= taken {
            second += 1;
        }
    }
    Some([Aspect::ALL[b], Aspect::ALL[tint], Aspect::ALL[second]])
}

/// All four tables, decoded.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResonanceTables {
    pub aspects: [AspectEntry; Aspect::COUNT],
    pub traits: [TraitEntry; TRAITS],
    /// `pairs[base][tint]`.
    pub pairs: [[PairEntry; Aspect::COUNT]; Aspect::COUNT],
    /// The records of the `triples` table at `triple_index(base, tint, second)`, as the registry
    /// holds them (every record was checked when the tables were decoded; [`Self::triple`]
    /// decodes one on demand). Kept as bytes: 1920 of them, where decoded entries would be
    /// 3360, and the whole value has to live in the program's heap and be built off its stack.
    pub triples: [[u8; TRIPLE_RECORD]; TRIPLES],
}

impl Default for ResonanceTables {
    fn default() -> Self {
        ResonanceTables {
            aspects: Default::default(),
            traits: Default::default(),
            pairs: Default::default(),
            triples: [[0; TRIPLE_RECORD]; TRIPLES],
        }
    }
}

fn condition(v: u8) -> Option<ResonanceCondition> {
    use ResonanceCondition::*;
    Some(match v {
        0 => Always,
        1 => CanBurn,
        2 => CanBeChilled,
        3 => CanBeFrozen,
        4 => IsFrozen,
        5 => CanBeShocked,
        6 => CanBleed,
        7 => CanBeStunned,
        8 => HasArmor,
        9 => IsBleeding,
        _ => return None,
    })
}

/// Decodes one aspect record (`index` is its aspect).
pub fn decode_aspect(index: usize, r: &[u8]) -> Option<AspectEntry> {
    if r.len() < ASPECT_RECORD || r[0] as usize != index || r[6] != 0 || r[7] != 0 {
        return None;
    }
    let native = match r[3] {
        NO_STATE => None,
        s => Some(StateKind::try_from(s).ok()?),
    };
    if r[4] as usize >= TRAITS {
        return None;
    }
    Some(AspectEntry {
        family: Family::try_from(r[1]).ok()?,
        flags: r[2],
        native,
        trait_id: r[4],
        condition: condition(r[5])?,
    })
}

pub fn encode_aspect(aspect: Aspect, e: &AspectEntry) -> [u8; ASPECT_RECORD] {
    [
        aspect as u8,
        e.family as u8,
        e.flags,
        e.native.map_or(NO_STATE, |s| s as u8),
        e.trait_id,
        e.condition as u8,
        0,
        0,
    ]
}

/// Decodes one trait record. An all-zero record is an absent trait.
pub fn decode_trait(r: &[u8]) -> Option<TraitEntry> {
    if r.len() < TRAIT_RECORD {
        return None;
    }
    if r[..TRAIT_RECORD].iter().all(|b| *b == 0) {
        return Some(TraitEntry::default());
    }
    if r[0] != 1 || r[1] != 0 || r[20..TRAIT_RECORD].iter().any(|b| *b != 0) {
        return None;
    }
    Some(TraitEntry {
        present: true,
        hit: Op::decode(&r[2..8])?,
        template: [Op::decode(&r[8..14])?, Op::decode(&r[14..20])?],
    })
}

pub fn encode_trait(e: &TraitEntry) -> [u8; TRAIT_RECORD] {
    let mut r = [0u8; TRAIT_RECORD];
    if !e.present {
        return r;
    }
    r[0] = 1;
    r[2..8].copy_from_slice(&e.hit.encode());
    r[8..14].copy_from_slice(&e.template[0].encode());
    r[14..20].copy_from_slice(&e.template[1].encode());
    r
}

/// Decodes one pair record.
pub fn decode_pair(r: &[u8]) -> Option<PairEntry> {
    if r.len() < PAIR_RECORD || r[1] & !PAIR_OVERRIDE != 0 || r[14] != 0 || r[15] != 0 {
        return None;
    }
    Some(PairEntry {
        condition: condition(r[0])?,
        set: r[1] & PAIR_OVERRIDE != 0,
        ops: [Op::decode(&r[2..8])?, Op::decode(&r[8..14])?],
    })
}

pub fn encode_pair(e: &PairEntry) -> [u8; PAIR_RECORD] {
    let mut r = [0u8; PAIR_RECORD];
    r[0] = e.condition as u8;
    r[1] = if e.set { PAIR_OVERRIDE } else { 0 };
    r[2..8].copy_from_slice(&e.ops[0].encode());
    r[8..14].copy_from_slice(&e.ops[1].encode());
    r
}

/// Decodes one triple record. An all-zero record has no ops (the second tint's trait template
/// is used).
pub fn decode_triple(r: &[u8]) -> Option<TripleEntry> {
    if r.len() < TRIPLE_RECORD {
        return None;
    }
    if r[..TRIPLE_RECORD].iter().all(|b| *b == 0) {
        return Some(TripleEntry::default());
    }
    if r[0] != 0 || r[1] != TRIPLE_SET || r[14] != 0 || r[15] != 0 {
        return None;
    }
    let mut ops = [Op::NONE; TRIPLE_OPS];
    for (i, op) in ops.iter_mut().enumerate() {
        let b = &r[2 + i * TRIPLE_OP_SIZE..2 + (i + 1) * TRIPLE_OP_SIZE];
        *op = Op::decode(&[b[0], b[1], b[2], b[3], 0, 0])?;
    }
    Some(TripleEntry { set: true, ops })
}

/// Encodes a triple record; `None` when an op value does not fit 16 bits.
pub fn encode_triple(e: &TripleEntry) -> Option<[u8; TRIPLE_RECORD]> {
    let mut r = [0u8; TRIPLE_RECORD];
    if !e.set {
        return Some(r);
    }
    r[1] = TRIPLE_SET;
    for (i, op) in e.ops.iter().enumerate() {
        let v = u16::try_from(op.value).ok()?.to_le_bytes();
        let at = 2 + i * TRIPLE_OP_SIZE;
        r[at] = op.kind as u8 | if op.crit_only { CRIT_ONLY } else { 0 };
        r[at + 1] = op.arg;
        r[at + 2] = v[0];
        r[at + 3] = v[1];
    }
    Some(r)
}

impl ResonanceTables {
    /// Decodes the four table bodies (each at least its full length).
    pub fn from_bytes(
        aspects: &[u8],
        traits: &[u8],
        pairs: &[u8],
        triples: &[u8],
    ) -> Result<ResonanceTables, TableError> {
        let mut t = ResonanceTables::default();
        t.decode_into(aspects, traits, pairs, triples)?;
        Ok(t)
    }

    /// [`Self::from_bytes`] in place (an on-chain caller keeps the ~4.5 KB value in a `Box`, off
    /// the 4 KB stack frame). On an error `self` is left partly written.
    pub fn decode_into(
        &mut self,
        aspects: &[u8],
        traits: &[u8],
        pairs: &[u8],
        triples: &[u8],
    ) -> Result<(), TableError> {
        if aspects.len() < ASPECTS_LEN
            || traits.len() < TRAITS_LEN
            || pairs.len() < PAIRS_LEN
            || triples.len() < TRIPLES_LEN
        {
            return Err(TableError::Short);
        }
        let t = self;
        for (i, e) in t.aspects.iter_mut().enumerate() {
            let r = &aspects[i * ASPECT_RECORD..(i + 1) * ASPECT_RECORD];
            *e = decode_aspect(i, r).ok_or(TableError::Record(i as u8))?;
        }
        for (i, e) in t.traits.iter_mut().enumerate() {
            let r = &traits[i * TRAIT_RECORD..(i + 1) * TRAIT_RECORD];
            *e = decode_trait(r).ok_or(TableError::Record(i as u8))?;
        }
        for i in 0..Aspect::COUNT * Aspect::COUNT {
            let r = &pairs[i * PAIR_RECORD..(i + 1) * PAIR_RECORD];
            t.pairs[i / Aspect::COUNT][i % Aspect::COUNT] =
                decode_pair(r).ok_or(TableError::Record(i as u8))?;
        }
        for (i, e) in t.triples.iter_mut().enumerate() {
            let r = &triples[i * TRIPLE_RECORD..(i + 1) * TRIPLE_RECORD];
            decode_triple(r).ok_or(TableError::Record(i as u8))?;
            e.copy_from_slice(r);
        }
        Ok(())
    }

    /// The four table bodies, in the registry format.
    #[allow(clippy::type_complexity)]
    pub fn to_bytes(
        &self,
    ) -> (
        [u8; ASPECTS_LEN],
        [u8; TRAITS_LEN],
        [u8; PAIRS_LEN],
        [u8; TRIPLES_LEN],
    ) {
        let mut a = [0u8; ASPECTS_LEN];
        let mut t = [0u8; TRAITS_LEN];
        let mut p = [0u8; PAIRS_LEN];
        let mut tr = [0u8; TRIPLES_LEN];
        for (i, e) in self.aspects.iter().enumerate() {
            a[i * ASPECT_RECORD..(i + 1) * ASPECT_RECORD]
                .copy_from_slice(&encode_aspect(Aspect::ALL[i], e));
        }
        for (i, e) in self.traits.iter().enumerate() {
            t[i * TRAIT_RECORD..(i + 1) * TRAIT_RECORD].copy_from_slice(&encode_trait(e));
        }
        for i in 0..Aspect::COUNT * Aspect::COUNT {
            let e = &self.pairs[i / Aspect::COUNT][i % Aspect::COUNT];
            p[i * PAIR_RECORD..(i + 1) * PAIR_RECORD].copy_from_slice(&encode_pair(e));
        }
        for (i, e) in self.triples.iter().enumerate() {
            tr[i * TRIPLE_RECORD..(i + 1) * TRIPLE_RECORD].copy_from_slice(e);
        }
        (a, t, p, tr)
    }

    /// The record of the triple base + tint + second, if the three differ and it has ops.
    pub fn triple(&self, base: Aspect, tint: Aspect, second: Aspect) -> Option<TripleEntry> {
        let e = decode_triple(&self.triples[triple_index(base, tint, second)?])?;
        e.set.then_some(e)
    }

    /// Writes the record of a triple (tests and tools). False when the three aspects are not
    /// different or an op value does not fit a record.
    pub fn set_triple(
        &mut self,
        base: Aspect,
        tint: Aspect,
        second: Aspect,
        entry: &TripleEntry,
    ) -> bool {
        let (Some(i), Some(r)) = (triple_index(base, tint, second), encode_triple(entry)) else {
            return false;
        };
        self.triples[i] = r;
        true
    }

    /// Native state of `aspect` (sections 3.2 and 5).
    pub fn native(&self, aspect: Aspect) -> Option<StateKind> {
        self.aspects[aspect as usize].native
    }

    /// The trait `aspect` brings as a tint.
    pub fn trait_of(&self, aspect: Aspect) -> &TraitEntry {
        &self.traits[self.aspects[aspect as usize].trait_id as usize]
    }

    /// The mechanics of base + tint: the override, or the tint's trait template on the base
    /// with the base's condition (section 10.1).
    pub fn pair(&self, base: Aspect, tint: Aspect) -> PairMechanics {
        let p = &self.pairs[base as usize][tint as usize];
        if p.set {
            return PairMechanics {
                condition: p.condition,
                ops: p.ops,
                generated: false,
            };
        }
        let t = self.trait_of(tint);
        PairMechanics {
            condition: self.aspects[base as usize].condition,
            ops: if t.present {
                t.template
            } else {
                [Op::NONE; PAIR_OPS]
            },
            generated: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_round_trip_and_reject_bad_args() {
        let ops = [
            Op::new(OpKind::Multiplier, ANY_STATE, 10_000),
            Op::new(OpKind::Multiplier, StateKind::Freeze as u8, 5_000).on_crit(),
            Op::new(OpKind::Linked, NATIVE, 10_000),
            Op::new(OpKind::LinkedChance, StateKind::Stun as u8, 10_000),
            Op::new(OpKind::LinkedMod, ModKind::Slow as u8, 10_000),
            Op::new(OpKind::JumpChain, StateKind::Bleeding as u8, 3),
            Op::new(OpKind::AddPacket, Aspect::Fire as u8, 10_000),
            Op::new(OpKind::ForceCrit, 0, 0),
            Op::new(OpKind::ApplyState, NATIVE, 10_000),
            Op::new(OpKind::ApplyMod, ModKind::Exposed as u8, 10_000),
            Op::new(OpKind::StateIfHad, StateKind::Burning as u8, 10_000),
            Op::new(OpKind::Splash, StateKind::Freeze as u8, 5_000),
            Op::new(OpKind::Nova, ModKind::Weakening as u8, 2),
            Op::new(OpKind::ModAfterNative, ModKind::Slow as u8, 10_000),
            Op::new(OpKind::StateFaster, StateKind::Bleeding as u8, 2),
            Op::new(OpKind::StateSlower, StateKind::Stun as u8, 2),
            Op::new(OpKind::TickNow, StateKind::Burning as u8, 2),
            Op::new(OpKind::Escalate, StateKind::Burning as u8, 0),
            Op::new(OpKind::TickAspect, Aspect::Fire as u8, 0),
            Op::new(OpKind::RemoveMod, ModKind::Weakening as u8, 0),
            Op::new(OpKind::NativeShorter, 0, 2),
            Op::new(OpKind::Delayed, StateKind::Bleeding as u8, 3 | 150 << 8),
            Op::new(OpKind::OnMiss, StateKind::Shock as u8, 10_000),
            Op::new(OpKind::ThawSplash, 0, 10_000),
            Op::new(OpKind::SplashLine, 0, 0),
            Op::new(OpKind::ChainRadius, StateKind::Shock as u8, 2 | 3 << 8),
            Op::new(OpKind::ChainRadius, StateKind::Burning as u8, 1 | (ANY_STATE as u32) << 8),
            Op::new(OpKind::ModAfterState, ModKind::Slow as u8, 5),
        ];
        for op in ops {
            assert_eq!(Op::decode(&op.encode()), Some(op));
        }
        // Unknown kind, bad args, a stray byte on None, a short slice.
        assert_eq!(Op::decode(&[41, 0, 0, 0, 0, 0]), None);
        assert_eq!(
            Op::decode(&[OpKind::NativeTurns as u8, 1, 0, 0, 0, 0]),
            None
        );
        assert_eq!(Op::decode(&[OpKind::Linked as u8, 7, 0, 0, 0, 0]), None);
        assert_eq!(
            Op::decode(&[OpKind::JumpChain as u8, NATIVE, 0, 0, 0, 0]),
            None
        );
        assert_eq!(Op::decode(&[OpKind::LinkedMod as u8, 6, 0, 0, 0, 0]), None);
        assert_eq!(Op::decode(&[OpKind::AddPacket as u8, 6, 0, 0, 0, 0]), None);
        assert_eq!(Op::decode(&[OpKind::ForceCrit as u8, 1, 0, 0, 0, 0]), None);
        assert_eq!(Op::decode(&[OpKind::ApplyMod as u8, 6, 0, 0, 0, 0]), None);
        assert_eq!(Op::decode(&[OpKind::Nova as u8, 16, 0, 0, 0, 0]), None);
        assert_eq!(Op::decode(&[OpKind::Escalate as u8, 1, 0, 0, 0, 0]), None);
        assert_eq!(Op::decode(&[OpKind::Delayed as u8, 3, 0x20, 0, 0, 0]), None);
        assert_eq!(Op::decode(&[OpKind::ChainRadius as u8, 4, 0, 0xFF, 0, 0]), None);
        assert_eq!(Op::decode(&[OpKind::ModAfterState as u8, 7, 9, 0, 0, 0]), None);
        assert_eq!(Op::decode(&[0, 0, 1, 0, 0, 0]), None);
        assert_eq!(Op::decode(&[1, 0xFF, 0, 0]), None);
    }

    fn sample() -> ResonanceTables {
        let mut t = ResonanceTables::default();
        for (i, a) in Aspect::ALL.iter().enumerate() {
            t.aspects[i] = AspectEntry {
                family: crate::family_of(*a),
                flags: 0,
                native: Some(StateKind::Burning),
                trait_id: i as u8,
                condition: ResonanceCondition::CanBurn,
            };
            t.traits[i] = TraitEntry {
                present: true,
                hit: Op::NONE,
                template: [Op::new(OpKind::Jump, NATIVE, 0), Op::NONE],
            };
        }
        t.pairs[1][5] = PairEntry {
            condition: ResonanceCondition::CanBeFrozen,
            set: true,
            ops: [
                Op::new(OpKind::StateOnCrit, StateKind::Freeze as u8, 0),
                Op::NONE,
            ],
        };
        t
    }

    #[test]
    fn tables_round_trip() {
        let t = sample();
        let (a, tr, p, x) = t.to_bytes();
        assert_eq!(ResonanceTables::from_bytes(&a, &tr, &p, &x), Ok(t));
        assert_eq!(
            ResonanceTables::from_bytes(&a[..40], &tr, &p, &x),
            Err(TableError::Short)
        );
        assert_eq!(
            ResonanceTables::from_bytes(&a, &tr, &p, &x[..40]),
            Err(TableError::Short)
        );
        let mut bad = p;
        bad[PAIR_RECORD * 3 + 1] = 2;
        assert_eq!(
            ResonanceTables::from_bytes(&a, &tr, &bad, &x),
            Err(TableError::Record(3))
        );
        let mut bad = a;
        bad[ASPECT_RECORD] = 0; // record 1 says aspect 0
        assert_eq!(
            ResonanceTables::from_bytes(&bad, &tr, &p, &x),
            Err(TableError::Record(1))
        );
        let mut bad = tr;
        bad[TRAIT_RECORD + 31] = 1;
        assert_eq!(
            ResonanceTables::from_bytes(&a, &bad, &p, &x),
            Err(TableError::Record(1))
        );
        let mut bad = x;
        bad[TRIPLE_RECORD * 2 + 1] = 2;
        assert_eq!(
            ResonanceTables::from_bytes(&a, &tr, &p, &bad),
            Err(TableError::Record(2))
        );
    }

    #[test]
    fn triple_records_round_trip_in_16_bytes() {
        let e = TripleEntry {
            set: true,
            ops: [
                Op::new(OpKind::ApplyState, StateKind::Burning as u8, 5_000),
                Op::new(OpKind::StateOnCrit, StateKind::Stun as u8, 0).on_crit(),
                Op::new(OpKind::Delayed, StateKind::Bleeding as u8, 3 | 200 << 8),
            ],
        };
        let r = encode_triple(&e).unwrap();
        assert_eq!(r.len(), TRIPLE_RECORD);
        assert_eq!(decode_triple(&r), Some(e));
        // An empty record is neutral and all zeros.
        assert_eq!(encode_triple(&TripleEntry::default()), Some([0; TRIPLE_RECORD]));
        assert_eq!(decode_triple(&[0; TRIPLE_RECORD]), Some(TripleEntry::default()));
        // A value past 16 bits does not fit.
        let big = TripleEntry {
            set: true,
            ops: [Op::new(OpKind::Penetrate, 0, 70_000), Op::NONE, Op::NONE],
        };
        assert_eq!(encode_triple(&big), None);
        // Stray bytes, a missing flag and a bad op are refused.
        for (at, v) in [(0, 1), (1, 0), (1, 2), (14, 1), (15, 1), (3, 9)] {
            let mut bad = r;
            bad[at] = v;
            assert_eq!(decode_triple(&bad), None, "byte {at} = {v}");
        }
        assert_eq!(decode_triple(&r[..8]), None);
        assert_eq!(TRIPLES, 120);
        assert_eq!(TRIPLES_LEN, 1920);
    }

    #[test]
    fn a_triple_is_found_by_its_three_aspects() {
        let mut t = sample();
        let e = TripleEntry {
            set: true,
            ops: [Op::new(OpKind::ForceCrit, 0, 0), Op::NONE, Op::NONE],
        };
        assert!(t.set_triple(Aspect::Fire, Aspect::Cold, Aspect::Blood, &e));
        assert!(!t.set_triple(Aspect::Fire, Aspect::Cold, Aspect::Fire, &e));
        assert_eq!(t.triple(Aspect::Fire, Aspect::Cold, Aspect::Blood), Some(e));
        assert_eq!(t.triple(Aspect::Fire, Aspect::Blood, Aspect::Cold), None);
        assert_eq!(t.triple(Aspect::Fire, Aspect::Cold, Aspect::Fire), None);
        let (a, tr, p, x) = t.to_bytes();
        assert_eq!(ResonanceTables::from_bytes(&a, &tr, &p, &x), Ok(t));
    }

    #[test]
    fn override_wins_otherwise_the_template_is_generated() {
        let t = sample();
        let o = t.pair(Aspect::Cold, Aspect::Piercing);
        assert!(!o.generated);
        assert_eq!(o.condition, ResonanceCondition::CanBeFrozen);
        assert_eq!(o.ops[0].kind, OpKind::StateOnCrit);
        let g = t.pair(Aspect::Cold, Aspect::Lightning);
        assert!(g.generated);
        assert_eq!(g.condition, ResonanceCondition::CanBurn);
        assert_eq!(g.ops[0], Op::new(OpKind::Jump, NATIVE, 0));
        assert_eq!(
            g.ops[0].state(Some(StateKind::Chill)),
            Some(StateKind::Chill)
        );
        // An absent trait generates nothing.
        let mut t = t;
        t.traits[2] = TraitEntry::default();
        assert_eq!(t.pair(Aspect::Cold, Aspect::Lightning).ops, [Op::NONE; 2]);
    }
}
