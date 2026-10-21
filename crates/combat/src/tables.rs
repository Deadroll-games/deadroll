//! Resonance tables as data (docs/DAMAGE_TYPES_AND_MIXES.md, sections 6 and 10.1).
//!
//! The registry stores three tables; this module owns their byte format, so the registry
//! (validation), the session (the copy taken at `open_session`), the simulator and the website
//! (WASM) all decode the same bytes with the same code.
//!
//! | Table        | Record | Bytes                                                                 |
//! |--------------|--------|-----------------------------------------------------------------------|
//! | `aspects`    | 8      | aspect, family, flags, native state (`NO_STATE`), trait, condition, 0, 0 |
//! | `traits`     | 32     | present (1), 0, hit op, template op 0, template op 1, 12 x 0           |
//! | `resonances` | 16     | condition, flags (`PAIR_OVERRIDE`), op 0, op 1, 0, 0                    |
//!
//! An op is 6 bytes: `kind | CRIT_ONLY`, `arg`, `value` (u32 LE). `arg` is a state, a modifier
//! kind or an aspect depending on the kind, and [`NATIVE`] stands for the base's native state.
//! `value` is a scale in bp (`BP` = x1) of the `Balance` number the kind names, or a count
//! (`NativeFaster` ticks, `JumpChain` targets, `Knockback` cells).
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
/// One tint trait per aspect, in `Aspect` order.
pub const TRAITS: usize = Aspect::COUNT;
pub const ASPECTS_LEN: usize = Aspect::COUNT * ASPECT_RECORD;
pub const TRAITS_LEN: usize = TRAITS * TRAIT_RECORD;
pub const PAIRS_LEN: usize = Aspect::COUNT * Aspect::COUNT * PAIR_RECORD;
/// Ops of a pair record and of a trait template.
pub const PAIR_OPS: usize = 2;
pub const TEMPLATE_OPS: usize = 2;

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
}

impl OpKind {
    pub const COUNT: u8 = 21;

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
            _ => return Option::None,
        })
    }

    /// What `arg` names for this kind.
    pub const fn arg(self) -> ArgKind {
        use OpKind::*;
        match self {
            None | NativeTurns | NativePower | NativeFaster | NativePowerArmor | ThawDamage
            | Knockback | IgniteCells | Penetrate => ArgKind::Zero,
            Multiplier => ArgKind::StateOrAny,
            StateOnCrit | ExtraStateChance | Linked | LinkedChance | RemoveState | Jump
            | JumpLine => ArgKind::State,
            JumpChain => ArgKind::StateOnly,
            LinkedMod => ArgKind::Mod,
            AddPacket | AccuracyBuff => ArgKind::Aspect,
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
            ArgKind::StateOrAny => arg == ANY_STATE || state(arg),
            // A resonance has no aspect weights to give a Resistance.
            ArgKind::Mod => (arg as usize) < ModKind::COUNT && arg != ModKind::Resistance as u8,
            ArgKind::Aspect => (arg as usize) < Aspect::COUNT,
        };
        if !arg_ok {
            return None;
        }
        Some(Op {
            kind,
            crit_only,
            arg,
            value,
        })
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

/// All three tables, decoded.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResonanceTables {
    pub aspects: [AspectEntry; Aspect::COUNT],
    pub traits: [TraitEntry; TRAITS],
    /// `pairs[base][tint]`.
    pub pairs: [[PairEntry; Aspect::COUNT]; Aspect::COUNT],
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

impl ResonanceTables {
    /// Decodes the three table bodies (each at least its full length).
    pub fn from_bytes(
        aspects: &[u8],
        traits: &[u8],
        pairs: &[u8],
    ) -> Result<ResonanceTables, TableError> {
        let mut t = ResonanceTables::default();
        t.decode_into(aspects, traits, pairs)?;
        Ok(t)
    }

    /// [`Self::from_bytes`] in place (an on-chain caller keeps the ~1 KB value in a `Box`, off
    /// the 4 KB stack frame). On an error `self` is left partly written.
    pub fn decode_into(
        &mut self,
        aspects: &[u8],
        traits: &[u8],
        pairs: &[u8],
    ) -> Result<(), TableError> {
        if aspects.len() < ASPECTS_LEN || traits.len() < TRAITS_LEN || pairs.len() < PAIRS_LEN {
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
        Ok(())
    }

    /// The three table bodies, in the registry format.
    pub fn to_bytes(&self) -> ([u8; ASPECTS_LEN], [u8; TRAITS_LEN], [u8; PAIRS_LEN]) {
        let mut a = [0u8; ASPECTS_LEN];
        let mut t = [0u8; TRAITS_LEN];
        let mut p = [0u8; PAIRS_LEN];
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
        (a, t, p)
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
        ];
        for op in ops {
            assert_eq!(Op::decode(&op.encode()), Some(op));
        }
        // Unknown kind, bad args, a stray byte on None, a short slice.
        assert_eq!(Op::decode(&[21, 0, 0, 0, 0, 0]), None);
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
        let (a, tr, p) = t.to_bytes();
        assert_eq!(ResonanceTables::from_bytes(&a, &tr, &p), Ok(t));
        assert_eq!(
            ResonanceTables::from_bytes(&a[..40], &tr, &p),
            Err(TableError::Short)
        );
        let mut bad = p;
        bad[PAIR_RECORD * 3 + 1] = 2;
        assert_eq!(
            ResonanceTables::from_bytes(&a, &tr, &bad),
            Err(TableError::Record(3))
        );
        let mut bad = a;
        bad[ASPECT_RECORD] = 0; // record 1 says aspect 0
        assert_eq!(
            ResonanceTables::from_bytes(&bad, &tr, &p),
            Err(TableError::Record(1))
        );
        let mut bad = tr;
        bad[TRAIT_RECORD + 31] = 1;
        assert_eq!(
            ResonanceTables::from_bytes(&a, &bad, &p),
            Err(TableError::Record(1))
        );
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
