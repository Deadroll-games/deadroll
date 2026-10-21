//! One attack, in the fixed order of docs/COMBAT_MATH.md, section 6.
//!
//! Roll order (the website replays it from the VRF seed, keep it stable):
//! 1. `d20` hit roll (none if the target is immune to every packet);
//! 2. crit chance roll, only on a hit that is not a natural 20 and with `crit_chance_pct > 0`;
//! 3. tint rolls: bases in skill order, tints in slot order ([`crate::resonance::roll_tints`]);
//! 4. one `d100` for the point in every packet's range;
//! 5. native state rolls, one per packet in packet order (only where a roll is needed);
//! 6. resonance state rolls, bases in skill order, ops in plan order (only the ops that roll:
//!    `ExtraStateChance`, and `LinkedChance` once the native state landed).
//!
//! A miss stops after roll 1. Rolls that are not needed are not made.
//!
//! What a resonance does comes from [`ResonanceTables`]: the plan of one resonance is the
//! pair's ops, the hit part of the best tint's trait, and for a triple the second tint's trait
//! template and hit part (docs/DAMAGE_TYPES_AND_MIXES.md, sections 6.3-6.5). Ops that change
//! damage act in steps 9-14, ops on the native state in step 20, the rest in step 21. What
//! needs the map (jumps to other units, knockback, burning cells) is returned for the caller.

use deadroll_types::{ActiveState, Aspect, Form, Source, StateKind, Tier, BP, STATES_MAX};

use crate::balance::Balance;
use crate::modifiers::{
    armor_cut, counter, effective_armor, is_physical, more, product_matching, ratio, resist_cut,
    sum_matching, Mod,
};
use crate::packet::{
    build_packets, DamageRange, Packets, Skill, ADDED_MAX, PACKETS_MAX, SKILL_ASPECTS_MAX,
};
use crate::resonance::{
    roll_tints, Carrier, Resonance, TargetView, Tint, TintRoll, TINTS_MAX, TINT_ROLLS_MAX,
};
use crate::rng::{chance, d100, d20, RollSource};
use crate::states::{
    apply_chill_for, apply_state, aspect_bit, find, form_bit, freeze_for, power_of, remove,
    source_bit, state_bit, Applied, States, TAGS,
};
use crate::support::{mod_turns, AccuracyBuff, ModKind, TimedMod};
use crate::tables::{Op, OpKind, ResonanceTables, ANY_STATE, PAIR_OPS, TEMPLATE_OPS};

#[cfg(feature = "borsh")]
use borsh::{BorshDeserialize, BorshSerialize};

/// "Increase" modifier slots of an attacker.
pub const INCREASED_MAX: usize = 8;
/// "More" modifier slots of an attacker (crit and resonance are added by the crate).
pub const MORE_MAX: usize = 4;
/// Ops of one resonance: the pair, the best tint's trait, a triple's second trait.
pub const RES_OPS_MAX: usize = PAIR_OPS + 1 + TEMPLATE_OPS + 1;
/// States one hit can pass on to other units.
pub const JUMPS_MAX: usize = 4;
/// Timed modifiers one hit can put on its target.
pub const TARGET_MODS_MAX: usize = 3;

/// Everything about the attacker that matters for one hit.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Attacker {
    /// Turn-order slot, written as the source of applied states.
    pub slot: u8,
    /// Added to the d20.
    pub attack_bonus: u32,
    /// Crit chance besides a natural 20, percent. 0 = only natural 20.
    pub crit_chance_pct: u32,
    pub weapon: DamageRange,
    pub skill: Skill,
    pub added: [DamageRange; ADDED_MAX],
    pub increased: [Mod; INCREASED_MAX],
    pub more: [Mod; MORE_MAX],
    /// Summed Weakenings on the attacker (counterweight on damage).
    pub weaken_bp: u32,
    /// Armor penetration (counterweight on target armor).
    pub penetration_bp: u32,
    /// "Increase" on native state chances and on the strength of Shock, Stun and Penetration.
    /// From the skill's source stat (Strength or Intelligence).
    pub state_chance_bonus_bp: u32,
    pub tints: [Tint; TINTS_MAX],
    /// "Increase" on Chill's chance and strength, from Dexterity (tempo).
    pub tempo_bonus_bp: u32,
}

/// Everything about the target that matters for one hit.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Defender {
    /// Threshold for `d20 + attack_bonus`.
    pub defense: u32,
    pub armor: u32,
    /// Resistance per aspect (counterweight, capped by `Balance::resist_cap_bp`).
    pub resist_bp: [u32; Aspect::COUNT],
    /// Vulnerability per aspect (multiplier).
    pub vuln_bp: [u32; Aspect::COUNT],
    /// Immunity per aspect, bit `1 << aspect`.
    pub immune_aspects: u8,
    /// Immunity per state, bit `1 << state`.
    pub immune_states: u8,
    /// State resistance per state kind (counterweight on the chance).
    pub state_resist_bp: [u32; StateKind::COUNT],
    /// States before the hit.
    pub states: States,
    /// Patch 9. Resistance per defense tag (counterweight, summed with the packet's aspect
    /// resistance before the cap), by [`TAGS`] index.
    pub tag_resist_bp: [u32; TAGS],
    /// Immunity per defense tag, bits [`source_bit`] / [`form_bit`]: every packet of a skill
    /// with that source or form is immune.
    pub immune_tags: u8,
}

impl Defender {
    /// The counterweight on the chance of `kind` landing (patch 9b): the state resistance in
    /// full, plus `Balance::aspect_state_resist_share_bp` of the resistance (capped like a hit)
    /// to the aspect whose native state `kind` is. Freeze is no aspect's native state.
    pub fn state_counter(
        &self,
        kind: StateKind,
        tables: &ResonanceTables,
        balance: &Balance,
    ) -> u32 {
        let aspect = Aspect::ALL
            .iter()
            .filter(|a| tables.native(**a) == Some(kind))
            .map(|a| self.resist_bp[*a as usize].min(balance.resist_cap_bp))
            .max()
            .unwrap_or(0);
        let share = (aspect as u64 * balance.aspect_state_resist_share_bp as u64 / BP as u64)
            .min(u32::MAX as u64) as u32;
        self.state_resist_bp[kind as usize].saturating_add(share)
    }

    /// Aspects a hit with `source` and `form` cannot touch: the aspect immunities, or every
    /// aspect when the target is immune to one of the hit's tags (step 3).
    pub fn immune_mask(&self, source: Source, form: Form) -> u8 {
        if self.immune_tags & (source_bit(source) | form_bit(form)) != 0 {
            u8::MAX
        } else {
            self.immune_aspects
        }
    }

    /// Resistance to a packet of `aspect` from a hit with `source` and `form`, before the cap:
    /// its aspect plus the hit's tags, one summed counterweight (step 15).
    pub fn resist_for(&self, aspect: Aspect, source: Source, form: Form) -> u32 {
        self.resist_bp[aspect as usize]
            .saturating_add(self.tag_resist_bp[source as usize])
            .saturating_add(self.tag_resist_bp[Source::COUNT + form as usize])
    }
}

#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HitInput {
    pub attacker: Attacker,
    pub defender: Defender,
}

#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Outcome {
    #[default]
    Miss,
    Hit,
    /// The target is immune to every packet: no rolls, no damage.
    Immune,
}

/// A state application check.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateRoll {
    pub kind: StateKind,
    /// A die was rolled. False with `success` = guaranteed (e.g. on a crit).
    pub rolled: bool,
    /// Percent. Success when `roll < chance_pct`.
    pub chance_pct: u32,
    pub roll: u16,
    pub success: bool,
    pub applied: Applied,
    /// Patch 9b: the counterweight the chance met (`Defender::state_counter`), bp; 0 when no
    /// die was rolled.
    pub resist_bp: u32,
}

/// One packet after every step. Values scaled by `BP` unless named otherwise.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PacketResult {
    pub aspect: Aspect,
    pub immune: bool,
    /// Step 9: converted base + added.
    pub base_min: u64,
    pub base_max: u64,
    /// Step 10: summed increase, bp.
    pub increased_bp: u32,
    /// Step 11: combined multiplier, bp (`BP` = x1): modifiers x crit x resonance.
    pub more_bp: u32,
    /// Step 12: attacker counterweight, bp.
    pub weaken_bp: u32,
    /// After steps 10-12.
    pub min: u64,
    pub max: u64,
    /// Step 13: the d100 point.
    pub rolled: u64,
    /// Step 14: effective armor after penetration (physical packets, else 0).
    pub armor: u32,
    /// Step 15: applied resistance (after cap), bp.
    pub resist_bp: u32,
    /// Step 16: vulnerability, bp.
    pub vuln_bp: u32,
    /// After target defense.
    pub after_defense: u64,
    /// Step 20: native state check (`rolled == false && !success` = no check).
    pub state: StateRoll,
    /// Step 14: the attacker's penetration (its own, plus Keenness), bp. 0 on elemental packets.
    pub pen_attacker_bp: u32,
    /// Step 14: Penetration already on the target, bp. 0 on elemental packets.
    pub pen_state_bp: u32,
    /// Step 16: Shock already on the target, a second vulnerability multiplier, bp.
    pub vuln_shock_bp: u32,
}

/// Where an op of a resonance comes from.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum OpOrigin {
    /// The pair (its record, or generated from the best tint's trait).
    #[default]
    Pair,
    /// The second tint's trait template: a triple.
    Template,
    /// A tint trait's hit part (Fury, Keenness), best or second tint.
    Trait,
}

/// What one op of a resonance did, for the breakdown.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpResult {
    pub kind: OpKind,
    pub crit_only: bool,
    pub arg: u8,
    pub origin: OpOrigin,
    /// The tint whose tier gives the strength.
    pub tint: Aspect,
    pub carrier: Carrier,
    pub tier: u8,
    /// It acted on this hit (its trigger happened).
    pub acted: bool,
    /// The state it applied, with its roll (`kind` default when none).
    pub state: StateRoll,
    /// What it added: bp (multiplier, penetration, power, modifier), turns, damage, cells, the
    /// d20 bonus or the minimum of an added packet.
    pub amount: u32,
    /// The maximum of an added packet.
    pub amount_max: u32,
}

/// How a state goes on to other units.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum JumpMode {
    /// One adjacent enemy of the target.
    #[default]
    Adjacent,
    /// Adjacent enemies that have `filter`, one after another, up to `limit`.
    Chain,
    /// The enemy right behind the target, seen from the attacker.
    Line,
}

/// A state the caller passes on to other units (it knows the map).
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Jump {
    pub mode: JumpMode,
    /// What lands on each unit it reaches (Chill: the buildup of this hit).
    pub state: ActiveState,
    pub filter: StateKind,
    pub limit: u8,
    /// The base of the resonance.
    pub base: Aspect,
}

/// The full hit breakdown (docs/COMBAT_MATH.md, section 12).
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HitResult {
    pub outcome: Outcome,
    pub d20: u8,
    /// `d20 + attack_bonus`, saturating.
    pub attack_total: u32,
    pub defense: u32,
    pub crit: bool,
    /// The crit chance roll, if one was made.
    pub crit_roll: Option<u16>,
    pub tint_rolls: [TintRoll; TINT_ROLLS_MAX],
    pub tint_roll_count: u8,
    /// Per skill base slot.
    pub resonances: [Option<Resonance>; SKILL_ASPECTS_MAX],
    pub d100: u8,
    pub packets: [PacketResult; PACKETS_MAX],
    pub packet_count: u8,
    /// Per skill base slot: the first state a resonance op applied (the full list is
    /// `res_ops`).
    pub resonance_states: [StateRoll; SKILL_ASPECTS_MAX],
    /// Target states after the hit.
    pub states: States,
    /// Final damage, rounded down once.
    pub damage: u32,
    /// Dice used from the roll source.
    pub rolls_used: u32,
    /// Per skill base slot, the ops of its resonance in plan order (`kind == None` after them).
    pub res_ops: [[OpResult; RES_OPS_MAX]; SKILL_ASPECTS_MAX],
    /// States passed on to other units, `jumps[..jump_count]`.
    pub jumps: [Jump; JUMPS_MAX],
    pub jump_count: u8,
    /// Cells the target is pushed away from the attacker (0 = none).
    pub knockback: u8,
    /// Burning of this power on the cells around the target (0 = none), for
    /// `Balance::burning_cell_rounds`.
    pub ignite_power: u32,
    /// Timed modifiers on the target, `target_mods[..target_mod_count]`.
    pub target_mods: [TimedMod; TARGET_MODS_MAX],
    pub target_mod_count: u8,
    /// A buff on the attacker (empty when `turns == 0`).
    pub accuracy_buff: AccuracyBuff,
    /// `state_bit`s of the states that landed on the target in this hit.
    pub landed: u8,
}

impl HitResult {
    pub fn packets(&self) -> &[PacketResult] {
        &self.packets[..self.packet_count as usize]
    }

    pub fn tint_rolls(&self) -> &[TintRoll] {
        &self.tint_rolls[..self.tint_roll_count as usize]
    }

    pub fn jumps(&self) -> &[Jump] {
        &self.jumps[..self.jump_count as usize]
    }

    pub fn target_mods(&self) -> &[TimedMod] {
        &self.target_mods[..self.target_mod_count as usize]
    }

    /// The ops of base slot `slot` that are set.
    pub fn ops(&self, slot: usize) -> impl Iterator<Item = &OpResult> {
        self.res_ops[slot]
            .iter()
            .take_while(|o| o.kind != OpKind::None)
    }
}

/// Counts the rolls taken from the inner source.
struct Counted<'a, R: RollSource> {
    inner: &'a mut R,
    used: u32,
}

impl<R: RollSource> RollSource for Counted<'_, R> {
    fn next_u64(&mut self) -> u64 {
        self.used += 1;
        self.inner.next_u64()
    }
}

/// One op of a resonance plan with the tint that powers it.
#[derive(Clone, Copy, Debug, Default)]
struct Planned {
    op: Op,
    origin: OpOrigin,
    tint: Tint,
}

#[derive(Clone, Copy, Debug, Default)]
struct Plan {
    ops: [Planned; RES_OPS_MAX],
    len: usize,
}

impl Plan {
    fn push(&mut self, op: Op, origin: OpOrigin, tint: Tint) {
        if !op.is_none() && self.len < RES_OPS_MAX {
            self.ops[self.len] = Planned { op, origin, tint };
            self.len += 1;
        }
    }

    fn of(res: &Resonance, tables: &ResonanceTables) -> Plan {
        let mut plan = Plan::default();
        let best = Tint::new(res.tint, res.tier, res.carrier);
        for op in tables.pair(res.base, res.tint).ops {
            plan.push(op, OpOrigin::Pair, best);
        }
        plan.push(tables.trait_of(res.tint).hit, OpOrigin::Trait, best);
        if let Some(second) = res.second_tint {
            let s = Tint::new(second, res.second_tier, res.second_carrier);
            let t = tables.trait_of(second);
            for op in t.template {
                plan.push(op, OpOrigin::Template, s);
            }
            plan.push(t.hit, OpOrigin::Trait, s);
        }
        plan
    }

    fn as_slice(&self) -> &[Planned] {
        &self.ops[..self.len]
    }
}

/// `curve x value / BP`, saturating.
fn scale(curve: u32, value: u32) -> u32 {
    (curve as u64 * value as u64 / BP as u64).min(u32::MAX as u64) as u32
}

fn tier_of(t: &Tint) -> Tier {
    Tier::new(t.tier).unwrap_or(Tier::WORST)
}

/// Changes a resonance makes to the native state of its base.
#[derive(Clone, Copy, Debug, Default)]
struct NativeMods {
    extra_turns: u8,
    power_bp: u32,
    rate: u32,
}

/// Native-state ops of `plan` that act on this hit (`crit`, target `armor`).
fn native_mods(
    plan: &Plan,
    crit: bool,
    armor: u32,
    balance: &Balance,
    ops: &mut [OpResult; RES_OPS_MAX],
) -> NativeMods {
    let mut m = NativeMods {
        rate: 1,
        ..NativeMods::default()
    };
    for (k, p) in plan.as_slice().iter().enumerate() {
        if p.op.crit_only && !crit {
            continue;
        }
        let tier = tier_of(&p.tint);
        let amount = match p.op.kind {
            OpKind::NativeTurns => {
                let t = scale(balance.res_turns(tier) as u32, p.op.value).min(u8::MAX as u32);
                m.extra_turns = m.extra_turns.saturating_add(t as u8);
                t
            }
            OpKind::NativePower => {
                let bp = scale(balance.res_power(tier), p.op.value);
                m.power_bp = m.power_bp.saturating_add(bp);
                bp
            }
            OpKind::NativePowerArmor => {
                let k = balance.armor_k.max(1) as u64;
                let bp = (scale(balance.res_power(tier), p.op.value) as u64 * armor as u64
                    / (armor as u64 + k)) as u32;
                m.power_bp = m.power_bp.saturating_add(bp);
                bp
            }
            OpKind::NativeFaster => {
                m.rate = m.rate.max(p.op.value.max(1));
                p.op.value
            }
            _ => continue,
        };
        ops[k].amount = amount;
    }
    m
}

/// Resolves one attack. See the module docs for the roll order.
pub fn resolve_hit(
    input: &HitInput,
    balance: &Balance,
    tables: &ResonanceTables,
    src: &mut impl RollSource,
) -> HitResult {
    let mut out = HitResult::default();
    resolve_hit_into(input, balance, tables, src, &mut out);
    out
}

/// Same as [`resolve_hit`], writing into `out` (lets on-chain callers keep the large result
/// off the 4 KB stack frame, e.g. in a `Box`).
pub fn resolve_hit_into(
    input: &HitInput,
    balance: &Balance,
    tables: &ResonanceTables,
    src: &mut impl RollSource,
    out: &mut HitResult,
) {
    *out = HitResult::default();
    let mut src = Counted {
        inner: src,
        used: 0,
    };
    let a = &input.attacker;
    let d = &input.defender;
    out.defense = d.defense;
    out.states = d.states;
    let (source, form) = (a.skill.source, a.skill.form);
    let immune_mask = d.immune_mask(source, form);
    let immune = |aspect: Aspect| immune_mask & aspect_bit(aspect) != 0;

    // Step 3: immunity to every packet (by aspect, or by the hit's tags).
    let mut packets = build_packets(a.weapon, &a.skill, &a.added);
    fill_packets(out, &packets, immune_mask);
    if packets.as_slice().iter().all(|p| immune(p.aspect)) {
        out.outcome = Outcome::Immune;
        return;
    }

    // Steps 4-5: hit and crit.
    out.d20 = d20(&mut src);
    out.attack_total = (out.d20 as u32).saturating_add(a.attack_bonus);
    let hit = match out.d20 {
        1 => false,
        20 => true,
        _ => out.attack_total >= d.defense,
    };
    if !hit {
        out.outcome = Outcome::Miss;
        out.rolls_used = src.used;
        return;
    }
    out.outcome = Outcome::Hit;
    out.crit = out.d20 == 20;
    if !out.crit && a.crit_chance_pct > 0 {
        let (crit, roll) = chance(&mut src, balance.clamp_chance(a.crit_chance_pct));
        out.crit = crit;
        out.crit_roll = Some(roll);
    }
    let crit = out.crit;

    // Steps 6-8: resonance.
    let (bases, base_count) = a.skill.bases(a.weapon.aspect);
    let bases = &bases[..base_count];
    let view = TargetView {
        immune_states: d.immune_states,
        armor: d.armor,
        states: &d.states,
    };
    let tints = roll_tints(
        bases,
        &a.tints,
        immune_mask,
        &view,
        balance,
        tables,
        &mut src,
    );
    out.tint_rolls = tints.rolls;
    out.tint_roll_count = tints.roll_count;
    out.resonances = tints.resonances;
    let mut plans = [Plan::default(); SKILL_ASPECTS_MAX];
    for (slot, res) in tints.resonances.iter().enumerate() {
        let Some(res) = res.filter(|r| r.active) else {
            continue;
        };
        plans[slot] = Plan::of(&res, tables);
        for (k, p) in plans[slot].as_slice().iter().enumerate() {
            out.res_ops[slot][k] = OpResult {
                kind: p.op.kind,
                crit_only: p.op.crit_only,
                arg: p.op.arg,
                origin: p.origin,
                tint: p.tint.aspect,
                carrier: p.tint.carrier,
                tier: p.tint.tier,
                ..OpResult::default()
            };
        }
    }
    let acts = |op: &Op| !op.crit_only || crit;

    // Step 9: Addition from tint traits (Fury).
    for (slot, plan) in plans.iter().enumerate() {
        for (k, p) in plan.as_slice().iter().enumerate() {
            if p.op.kind != OpKind::AddPacket || !acts(&p.op) {
                continue;
            }
            let Ok(aspect) = Aspect::try_from(p.op.arg) else {
                continue;
            };
            let [min, max] = balance.tint_added(tier_of(&p.tint));
            let (min, max) = (scale(min, p.op.value), scale(max, p.op.value));
            packets.add(
                aspect,
                min as u64 * BP as u64,
                max.max(min) as u64 * BP as u64,
            );
            let o = &mut out.res_ops[slot][k];
            (o.acted, o.amount, o.amount_max) = (true, min, max.max(min));
        }
    }
    fill_packets(out, &packets, immune_mask);

    // Steps 10-12: attacker modifiers, per packet. A resonance's multiplier is on its base.
    let mut res_more = [BP as u64; SKILL_ASPECTS_MAX];
    for (slot, plan) in plans.iter().enumerate() {
        for (k, p) in plan.as_slice().iter().enumerate() {
            if p.op.kind != OpKind::Multiplier || !acts(&p.op) {
                continue;
            }
            let required = p.op.arg == ANY_STATE
                || StateKind::try_from(p.op.arg).is_ok_and(|kind| view.has(kind));
            if !required {
                continue;
            }
            let bp = scale(balance.resonance_more(tier_of(&p.tint)), p.op.value);
            res_more[slot] = more(res_more[slot], bp);
            let o = &mut out.res_ops[slot][k];
            (o.acted, o.amount) = (true, bp);
        }
    }
    for r in out
        .packets
        .iter_mut()
        .take(packets.count as usize)
        .filter(|r| !r.immune)
    {
        r.increased_bp = sum_matching(&a.increased, r.aspect, source, form);
        let mut mult = product_matching(&a.more, r.aspect, source, form) as u64;
        if crit {
            mult = more(mult, balance.crit_more_bp);
        }
        for (slot, res) in tints.resonances.iter().enumerate() {
            if res.is_some_and(|res| res.active && res.base == r.aspect) {
                mult = (mult as u128 * res_more[slot] as u128 / BP as u128).min(u64::MAX as u128)
                    as u64;
            }
        }
        r.more_bp = mult.min(u32::MAX as u64) as u32;
        r.weaken_bp = a.weaken_bp;
        let apply = |v: u64| {
            let v = more(v, r.increased_bp);
            let v = (v as u128 * r.more_bp as u128 / BP as u128).min(u64::MAX as u128) as u64;
            counter(v, r.weaken_bp)
        };
        r.min = apply(r.base_min);
        r.max = apply(r.base_max).max(r.min);
    }

    // Keenness: a triggered Piercing tint penetrates on this hit (steps 6-8 are already done).
    let mut pen_attacker = a.penetration_bp;
    for (slot, plan) in plans.iter().enumerate() {
        for (k, p) in plan.as_slice().iter().enumerate() {
            if p.op.kind == OpKind::Penetrate && acts(&p.op) {
                let bp = scale(balance.keenness_pen(tier_of(&p.tint)), p.op.value);
                pen_attacker = pen_attacker.saturating_add(bp);
                let o = &mut out.res_ops[slot][k];
                (o.acted, o.amount) = (true, bp);
            }
        }
    }
    // States already on the target. The ones this hit applies land after defense.
    let pen_state = power_of(&d.states, StateKind::Penetration);
    let shock = power_of(&d.states, StateKind::Shock);
    let pen = pen_attacker.saturating_add(pen_state);

    // Step 13: one d100 for every packet.
    out.d100 = d100(&mut src);
    // Steps 14-16: target defense, per packet.
    let mut total: u128 = 0;
    for r in out
        .packets
        .iter_mut()
        .take(packets.count as usize)
        .filter(|r| !r.immune)
    {
        let span = (r.max - r.min) as u128 * out.d100 as u128 / 100;
        r.rolled = (r.min as u128 + span).min(u64::MAX as u128) as u64;
        let mut v = r.rolled;
        if is_physical(r.aspect) {
            r.armor = effective_armor(d.armor, pen);
            r.pen_attacker_bp = pen_attacker;
            r.pen_state_bp = pen_state;
            v = armor_cut(v, d.armor, pen, balance.armor_k);
        }
        r.resist_bp = d
            .resist_for(r.aspect, source, form)
            .min(balance.resist_cap_bp);
        v = resist_cut(v, r.resist_bp, balance.resist_cap_bp);
        r.vuln_bp = d.vuln_bp[r.aspect as usize];
        r.vuln_shock_bp = shock;
        v = more(v, r.vuln_bp);
        v = more(v, shock);
        r.after_defense = v;
        total += v as u128;
    }

    // Steps 17-18: round down once, minimum damage on a hit.
    let damage = (total / BP as u128).min(u32::MAX as u128) as u32;
    out.damage = damage.max(balance.min_damage);

    let mut lands = Landing {
        can_freeze: d.immune_states & state_bit(StateKind::Freeze) == 0,
        states: d.states,
        landed: 0,
        now: [ActiveState::default(); StateKind::COUNT],
    };

    // Step 20: native states, one check per packet; a resonance on the packet's base changes
    // what lands (longer, stronger, faster).
    for i in 0..packets.count as usize {
        let r = out.packets[i];
        if r.immune {
            continue;
        }
        let Some(kind) = tables.native(r.aspect) else {
            continue;
        };
        if d.immune_states & state_bit(kind) != 0 || a.skill.state_chance_pct == 0 {
            continue;
        }
        let slot = bases.iter().position(|b| *b == r.aspect);
        let pure = slot.map(|s| tints.pure_pair_bp[s]).unwrap_or(0);
        let mods = match slot {
            Some(s) => native_mods(&plans[s], crit, d.armor, balance, &mut out.res_ops[s]),
            None => NativeMods {
                rate: 1,
                ..NativeMods::default()
            },
        };
        let stat_bonus = state_stat_bonus(kind, a);
        let increase = stat_bonus.saturating_add(pure);
        // The state resistance, and a share of the resistance to the state's aspect.
        let resist = d.state_counter(kind, tables, balance);
        let raw = ratio(a.skill.state_chance_pct as u64, increase, resist);
        let chance_pct = balance.clamp_chance(raw.min(u32::MAX as u64) as u32);
        let (success, roll) = chance(&mut src, chance_pct);
        let mut roll_result = StateRoll {
            kind,
            rolled: true,
            chance_pct,
            roll,
            success,
            applied: Applied::None,
            resist_bp: resist,
        };
        if success {
            roll_result.applied = lands.apply(
                kind,
                StateApply {
                    packet_damage: r.after_defense,
                    stat_bonus_bp: stat_bonus,
                    pure_bp: pure,
                    source: a.slot,
                    extra_turns: mods.extra_turns,
                    power_bp: mods.power_bp,
                    rate: mods.rate,
                },
                balance,
            );
            if let Some(s) = slot {
                let applied = roll_result.applied != Applied::None;
                for (k, p) in plans[s].as_slice().iter().enumerate() {
                    let native_op = matches!(
                        p.op.kind,
                        OpKind::NativeTurns
                            | OpKind::NativePower
                            | OpKind::NativePowerArmor
                            | OpKind::NativeFaster
                    );
                    if native_op && acts(&p.op) {
                        out.res_ops[s][k].acted = applied;
                    }
                }
            }
        }
        out.packets[i].state = roll_result;
    }

    // Step 21: the resonance's own states and effects.
    for (slot, res) in tints.resonances.iter().enumerate() {
        let Some(res) = res.filter(|r| r.active) else {
            continue;
        };
        let base_damage = out
            .packets()
            .iter()
            .find(|p| p.aspect == res.base)
            .map(|p| p.after_defense)
            .unwrap_or(0);
        let native = tables.native(res.base);
        let native_landed = native.is_some_and(|k| lands.landed & state_bit(k) != 0);
        let mut removals: [Option<(usize, StateKind)>; RES_OPS_MAX] = [None; RES_OPS_MAX];
        for (k, p) in plans[slot].as_slice().iter().enumerate() {
            if !acts(&p.op) {
                continue;
            }
            let tier = tier_of(&p.tint);
            let state = p.op.state(native);
            let can = |kind: StateKind| d.immune_states & state_bit(kind) == 0;
            let o = &mut out.res_ops[slot][k];
            let apply_of = |kind: StateKind, damage: u64| StateApply {
                packet_damage: damage,
                stat_bonus_bp: state_stat_bonus(kind, a),
                pure_bp: balance.res_power(tier),
                source: a.slot,
                extra_turns: 0,
                power_bp: 0,
                rate: 1,
            };
            match p.op.kind {
                OpKind::StateOnCrit => {
                    let Some(kind) = state.filter(|k| crit && can(*k)) else {
                        continue;
                    };
                    let applied = lands.apply(kind, apply_of(kind, base_damage), balance);
                    o.acted = true;
                    o.state = StateRoll {
                        kind,
                        rolled: false,
                        chance_pct: 100,
                        roll: 0,
                        success: true,
                        applied,
                        resist_bp: 0,
                    };
                }
                OpKind::Linked => {
                    let Some(kind) =
                        state.filter(|k| native_landed && can(*k) && Some(*k) != native)
                    else {
                        continue;
                    };
                    let damage = (base_damage as u128 * p.op.value as u128 / BP as u128)
                        .min(u64::MAX as u128) as u64;
                    let applied = lands.apply(kind, apply_of(kind, damage), balance);
                    o.acted = true;
                    o.state = StateRoll {
                        kind,
                        rolled: false,
                        chance_pct: 100,
                        roll: 0,
                        success: true,
                        applied,
                        resist_bp: 0,
                    };
                }
                OpKind::ExtraStateChance | OpKind::LinkedChance => {
                    let linked = p.op.kind == OpKind::LinkedChance;
                    let Some(kind) = state.filter(|k| can(*k) && (!linked || native_landed)) else {
                        continue;
                    };
                    let stat_bonus = state_stat_bonus(kind, a);
                    let resist = d.state_counter(kind, tables, balance);
                    let base = scale(balance.extra_state_chance_pct, p.op.value);
                    let raw = ratio(base as u64, stat_bonus, resist);
                    let chance_pct = balance.clamp_chance(raw.min(u32::MAX as u64) as u32);
                    let (success, roll) = chance(&mut src, chance_pct);
                    let applied = if success {
                        lands.apply(kind, apply_of(kind, base_damage), balance)
                    } else {
                        Applied::None
                    };
                    o.acted = true;
                    o.state = StateRoll {
                        kind,
                        rolled: true,
                        chance_pct,
                        roll,
                        success,
                        applied,
                        resist_bp: resist,
                    };
                }
                OpKind::LinkedMod => {
                    let Some(kind) = ModKind::ALL.get(p.op.arg as usize).copied() else {
                        continue;
                    };
                    if !native_landed || out.target_mod_count as usize >= TARGET_MODS_MAX {
                        continue;
                    }
                    let stat = match kind {
                        ModKind::Haste | ModKind::Slow => a.tempo_bonus_bp,
                        _ => a.state_chance_bonus_bp,
                    };
                    let power = more(scale(balance.res_mod(tier), p.op.value) as u64, stat)
                        .clamp(1, u32::MAX as u64) as u32;
                    out.target_mods[out.target_mod_count as usize] = TimedMod {
                        kind,
                        aspect: res.base,
                        turns: mod_turns(balance.res_mod_turns),
                        power_bp: power,
                        source: a.slot,
                        ..TimedMod::default()
                    };
                    out.target_mod_count += 1;
                    (o.acted, o.amount) = (true, power);
                }
                OpKind::ThawDamage => {
                    if lands.landed & state_bit(StateKind::Freeze) == 0 {
                        continue;
                    }
                    let Some(i) = find(&lands.states, StateKind::Freeze) else {
                        continue;
                    };
                    let thaw = base_damage as u128 * p.op.value as u128 / (BP as u128 * BP as u128);
                    let thaw = more(thaw.min(u64::MAX as u128) as u64, balance.res_power(tier))
                        .clamp(1, u32::MAX as u64) as u32;
                    let f = &mut lands.states[i];
                    f.power = f.power.max(thaw);
                    (o.acted, o.amount) = (true, f.power);
                }
                OpKind::RemoveState => {
                    if let Some(kind) = state {
                        removals[k] = Some((k, kind));
                    }
                }
                OpKind::Jump | OpKind::JumpLine | OpKind::JumpChain => {
                    let (moving, filter, limit) = match p.op.kind {
                        OpKind::JumpChain => match (native, StateKind::try_from(p.op.arg)) {
                            (Some(n), Ok(f)) => (n, f, p.op.value.min(u8::MAX as u32) as u8),
                            _ => continue,
                        },
                        _ => match state {
                            Some(s) => (s, s, 1),
                            None => continue,
                        },
                    };
                    if lands.landed & state_bit(moving) == 0
                        || out.jump_count as usize >= JUMPS_MAX
                        || limit == 0
                    {
                        continue;
                    }
                    out.jumps[out.jump_count as usize] = Jump {
                        mode: match p.op.kind {
                            OpKind::JumpChain => JumpMode::Chain,
                            OpKind::JumpLine => JumpMode::Line,
                            _ => JumpMode::Adjacent,
                        },
                        state: lands.now[moving as usize],
                        filter,
                        limit,
                        base: res.base,
                    };
                    out.jump_count += 1;
                    (o.acted, o.amount) = (true, limit as u32);
                }
                OpKind::Knockback => {
                    let cells = p.op.value.min(u8::MAX as u32) as u8;
                    out.knockback = out.knockback.max(cells);
                    (o.acted, o.amount) = (true, cells as u32);
                }
                OpKind::IgniteCells => {
                    let damage = base_damage as u128 * p.op.value as u128 / BP as u128;
                    let per_turn =
                        more(damage.min(u64::MAX as u128) as u64, balance.res_power(tier)) as u128
                            * balance.dot_share_bp as u128
                            / (BP as u128 * BP as u128);
                    let power = (per_turn.min(u32::MAX as u128) as u32).max(1);
                    out.ignite_power = out.ignite_power.max(power);
                    (o.acted, o.amount) = (true, power);
                }
                OpKind::AccuracyBuff => {
                    let Ok(aspect) = Aspect::try_from(p.op.arg) else {
                        continue;
                    };
                    let bonus = scale(balance.res_accuracy(tier), p.op.value);
                    out.accuracy_buff.apply(AccuracyBuff {
                        aspect,
                        turns: mod_turns(balance.res_accuracy_turns),
                        bonus,
                    });
                    (o.acted, o.amount) = (true, bonus);
                }
                OpKind::None
                | OpKind::Multiplier
                | OpKind::NativeTurns
                | OpKind::NativePower
                | OpKind::NativeFaster
                | OpKind::NativePowerArmor
                | OpKind::AddPacket
                | OpKind::Penetrate => {}
            }
        }
        // "Removes Freeze" comes after the hit's own states (Cleaving Ice).
        for (k, kind) in removals.iter().flatten() {
            if remove(&mut lands.states, *kind) {
                let o = &mut out.res_ops[slot][*k];
                o.acted = true;
            }
        }
        let first = out
            .ops(slot)
            .find(|o| o.state.success || o.state.rolled)
            .map(|o| o.state)
            .unwrap_or_default();
        out.resonance_states[slot] = first;
    }

    out.states = lands.states;
    out.landed = lands.landed;
    out.rolls_used = src.used;
}

/// Step 9 numbers of every packet (the added ones included).
fn fill_packets(out: &mut HitResult, packets: &Packets, immune_aspects: u8) {
    out.packet_count = packets.count;
    for (r, p) in out.packets.iter_mut().zip(packets.as_slice()) {
        r.aspect = p.aspect;
        r.immune = immune_aspects & aspect_bit(p.aspect) != 0;
        r.base_min = p.min;
        r.base_max = p.max;
    }
}

/// Chill scales with Dexterity. Every other state scales with the skill's source stat.
fn state_stat_bonus(kind: StateKind, attacker: &Attacker) -> u32 {
    if kind == StateKind::Chill {
        attacker.tempo_bonus_bp
    } else {
        attacker.state_chance_bonus_bp
    }
}

/// Numbers a state application reads besides the kind.
struct StateApply {
    packet_damage: u64,
    stat_bonus_bp: u32,
    /// Pure pair, or the tint tier's power for a resonance's own state.
    pure_bp: u32,
    source: u8,
    /// From the resonance on the base: more turns, more power, faster ticks.
    extra_turns: u8,
    power_bp: u32,
    rate: u32,
}

/// The target's states during step 20-21 and what landed on them.
struct Landing {
    /// The target is not immune to Freeze: Chill buildup may freeze it.
    can_freeze: bool,
    states: States,
    /// `state_bit`s.
    landed: u8,
    /// What each kind's last landing put on the target.
    now: [ActiveState; StateKind::COUNT],
}

impl Landing {
    fn apply(&mut self, kind: StateKind, s: StateApply, balance: &Balance) -> Applied {
        let (applied, inst) = apply_kind(&mut self.states, kind, s, self.can_freeze, balance);
        if applied == Applied::None {
            return applied;
        }
        self.landed |= state_bit(kind);
        self.now[kind as usize] = inst;
        if applied == Applied::Froze {
            self.landed |= state_bit(StateKind::Freeze);
            if let Some(i) = find(&self.states, StateKind::Freeze) {
                self.now[StateKind::Freeze as usize] = self.states[i];
            }
        }
        applied
    }
}

/// Applies a state of `kind` and returns what it put on (before merging with an existing
/// instance). Over-time strength is a share of the packet damage (section 7.1), so the aspect
/// resistance weakens it through the damage only. Chill, Shock and Penetration scale with the
/// stat bonus and the pure pair.
fn apply_kind(
    states: &mut [ActiveState; STATES_MAX],
    kind: StateKind,
    apply: StateApply,
    can_freeze: bool,
    balance: &Balance,
) -> (Applied, ActiveState) {
    let StateApply {
        packet_damage,
        stat_bonus_bp,
        pure_bp,
        source,
        extra_turns,
        power_bp,
        rate,
    } = apply;
    let turns = balance
        .turns(kind)
        .saturating_add(extra_turns)
        .min(balance.state_turns_max);
    let scaled = |base: u32| {
        let increase = stat_bonus_bp
            .saturating_add(pure_bp)
            .saturating_add(power_bp);
        more(base as u64, increase).min(u32::MAX as u64) as u32
    };
    let inst = |power: u32, turns: u8| ActiveState {
        kind,
        turns,
        power,
        source,
    };
    match kind {
        StateKind::Chill => {
            let s = inst(scaled(balance.chill_buildup_bp), turns);
            (
                apply_chill_for(states, s.power, turns, source, can_freeze, balance),
                s,
            )
        }
        StateKind::Freeze => (freeze_for(states, turns, source), inst(0, turns)),
        StateKind::Burning | StateKind::Bleeding => {
            let increase = pure_bp.saturating_add(power_bp);
            let per_turn = more(packet_damage, increase) as u128 * balance.dot_share_bp as u128
                / (BP as u128 * BP as u128);
            let rate = rate.max(1);
            let power = (per_turn.saturating_mul(rate as u128).min(u32::MAX as u128) as u32).max(1);
            let turns = (turns as u32).div_ceil(rate).max(1) as u8;
            let s = inst(power, turns);
            (apply_state(states, s), s)
        }
        StateKind::Shock => {
            let s = inst(scaled(balance.shock_vuln_bp), turns);
            (apply_state(states, s), s)
        }
        StateKind::Stun => {
            let s = inst(0, turns);
            (apply_state(states, s), s)
        }
        StateKind::Penetration => {
            let s = inst(scaled(balance.penetration_power_bp), turns);
            (apply_state(states, s), s)
        }
    }
}

/// Damage hint without dice (website preview): total min and max after attacker modifiers
/// and target defense, no crit, no resonance, rounded down. Immune packets count as zero.
pub fn preview_range(input: &HitInput, balance: &Balance) -> (u32, u32) {
    damage_range(input, balance, false, &Extra::NONE)
}

/// One tint candidate in the preview.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreviewTint {
    pub base: Aspect,
    pub tint: Aspect,
    pub carrier: Carrier,
    pub tier: u8,
    /// The tint equals the base: no roll, the native state is stronger.
    pub pure: bool,
    /// Chance to trigger, percent (0 for a pure pair).
    pub chance_pct: u32,
    /// `resonance_id(base, tint)`.
    pub id: u8,
    /// The pair has no record: the tint's trait.
    pub generated: bool,
    /// The target meets the pair's condition.
    pub condition_met: bool,
    /// Damage of a hit without a crit when this tint alone triggers (its added packet,
    /// multiplier and penetration).
    pub min: u32,
    pub max: u32,
}

/// What an attack would do, before any die is rolled (docs/COMBAT_MATH.md, section 13).
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HitPreview {
    /// The target is immune to every packet: no roll is made.
    pub immune: bool,
    /// Chance to hit, bp: the d20 faces that hit (a natural 1 always misses, a natural 20 always
    /// hits), out of 20.
    pub hit_bp: u32,
    /// Chance to crit, bp: a natural 20, or a hit on another face that wins the crit roll.
    pub crit_bp: u32,
    /// Damage of a hit without and with a crit; no resonance (its tints are rolled).
    pub min: u32,
    pub max: u32,
    pub crit_min: u32,
    pub crit_max: u32,
    /// The tint candidates, `tints[..tint_count]`, bases in skill order, tints in slot order.
    pub tints: [PreviewTint; TINT_ROLLS_MAX],
    pub tint_count: u8,
}

/// Chances and damage of `input` without dice, by the same rules as [`resolve_hit`].
pub fn preview_hit(input: &HitInput, balance: &Balance, tables: &ResonanceTables) -> HitPreview {
    let a = &input.attacker;
    let d = &input.defender;
    let packets = build_packets(a.weapon, &a.skill, &a.added);
    let immune_mask = d.immune_mask(a.skill.source, a.skill.form);
    if packets
        .as_slice()
        .iter()
        .all(|p| immune_mask & aspect_bit(p.aspect) != 0)
    {
        return HitPreview {
            immune: true,
            ..HitPreview::default()
        };
    }
    // Faces 2..=19 hit on the threshold; 20 always hits.
    let plain_hits = (2u32..=19)
        .filter(|face| face.saturating_add(a.attack_bonus) >= d.defense)
        .count() as u32;
    let crit_roll = if a.crit_chance_pct > 0 {
        balance.clamp_chance(a.crit_chance_pct).saturating_mul(100)
    } else {
        0
    };
    let (min, max) = damage_range(input, balance, false, &Extra::NONE);
    let (crit_min, crit_max) = damage_range(input, balance, true, &Extra::NONE);
    let mut out = HitPreview {
        immune: false,
        hit_bp: (plain_hits + 1) * BP / 20,
        crit_bp: (BP + plain_hits * crit_roll) / 20,
        min,
        max,
        crit_min,
        crit_max,
        ..HitPreview::default()
    };
    let view = TargetView {
        immune_states: d.immune_states,
        armor: d.armor,
        states: &d.states,
    };
    let (bases, base_count) = a.skill.bases(a.weapon.aspect);
    for (i, &base) in bases[..base_count].iter().enumerate() {
        if immune_mask & aspect_bit(base) != 0 || bases[..i].contains(&base) {
            continue;
        }
        for t in a.tints.iter() {
            let Ok(tier) = Tier::new(t.tier) else {
                continue;
            };
            if out.tint_count as usize >= TINT_ROLLS_MAX {
                break;
            }
            let pure = t.aspect == base;
            let mechanics = tables.pair(base, t.aspect);
            let condition_met = crate::resonance::condition_met(mechanics.condition, &view);
            let (min, max) = if pure || !condition_met {
                (min, max)
            } else {
                let mut extra = Extra::NONE;
                let ops = mechanics
                    .ops
                    .iter()
                    .chain(core::iter::once(&tables.trait_of(t.aspect).hit));
                for op in ops.filter(|op| !op.crit_only) {
                    match op.kind {
                        OpKind::AddPacket => {
                            if let Ok(aspect) = Aspect::try_from(op.arg) {
                                let [lo, hi] = balance.tint_added(tier);
                                extra.added = DamageRange::new(
                                    aspect,
                                    scale(lo, op.value),
                                    scale(hi, op.value),
                                );
                            }
                        }
                        OpKind::Multiplier
                            if op.arg == ANY_STATE
                                || StateKind::try_from(op.arg).is_ok_and(|k| view.has(k)) =>
                        {
                            extra.more_aspect = base;
                            extra.more =
                                more(extra.more, scale(balance.resonance_more(tier), op.value));
                        }
                        OpKind::Penetrate => {
                            extra.pen_bp = extra
                                .pen_bp
                                .saturating_add(scale(balance.keenness_pen(tier), op.value));
                        }
                        _ => {}
                    }
                }
                damage_range(input, balance, false, &extra)
            };
            out.tints[out.tint_count as usize] = PreviewTint {
                base,
                tint: t.aspect,
                carrier: t.carrier,
                tier: t.tier,
                pure,
                chance_pct: if pure {
                    0
                } else {
                    balance.clamp_chance(balance.tint_chance(tier))
                },
                id: crate::resonance::resonance_id(base, t.aspect),
                generated: mechanics.generated,
                condition_met,
                min,
                max,
            };
            out.tint_count += 1;
        }
    }
    out
}

/// What a triggered tint adds to the preview.
struct Extra {
    added: DamageRange,
    more_aspect: Aspect,
    /// Combined resonance multiplier on `more_aspect`, bp (`BP` = x1).
    more: u64,
    pen_bp: u32,
}

impl Extra {
    const NONE: Extra = Extra {
        added: DamageRange::new(Aspect::Fire, 0, 0),
        more_aspect: Aspect::Fire,
        more: BP as u64,
        pen_bp: 0,
    };
}

/// Total min and max of a hit after attacker modifiers (with the crit multiplier when `crit`)
/// and target defense, rounded down once.
fn damage_range(input: &HitInput, balance: &Balance, crit: bool, extra: &Extra) -> (u32, u32) {
    let a = &input.attacker;
    let d = &input.defender;
    let mut packets: Packets = build_packets(a.weapon, &a.skill, &a.added);
    if !extra.added.is_empty() {
        let r = extra.added;
        packets.add(
            r.aspect,
            r.min as u64 * BP as u64,
            r.max.max(r.min) as u64 * BP as u64,
        );
    }
    let pen = a
        .penetration_bp
        .saturating_add(extra.pen_bp)
        .saturating_add(power_of(&d.states, StateKind::Penetration));
    let shock = power_of(&d.states, StateKind::Shock);
    let immune_mask = d.immune_mask(a.skill.source, a.skill.form);
    let (mut lo, mut hi) = (0u128, 0u128);
    for p in packets.as_slice() {
        if immune_mask & aspect_bit(p.aspect) != 0 {
            continue;
        }
        let incr = sum_matching(&a.increased, p.aspect, a.skill.source, a.skill.form);
        let mut mult = product_matching(&a.more, p.aspect, a.skill.source, a.skill.form) as u64;
        if crit {
            mult = more(mult, balance.crit_more_bp);
        }
        if p.aspect == extra.more_aspect {
            mult = (mult as u128 * extra.more as u128 / BP as u128).min(u64::MAX as u128) as u64;
        }
        // As `resolve_hit` stores it (`PacketResult::more_bp`).
        let mult = mult.min(u32::MAX as u64);
        let defend = |v: u64| {
            let v = more(v, incr);
            let v = (v as u128 * mult as u128 / BP as u128).min(u64::MAX as u128) as u64;
            let mut v = counter(v, a.weaken_bp);
            if is_physical(p.aspect) {
                v = armor_cut(v, d.armor, pen, balance.armor_k);
            }
            let resist = d.resist_for(p.aspect, a.skill.source, a.skill.form);
            v = resist_cut(v, resist, balance.resist_cap_bp);
            let v = more(v, d.vuln_bp[p.aspect as usize]);
            more(v, shock) as u128
        };
        lo += defend(p.min);
        hi += defend(p.max.max(p.min));
    }
    let to_u32 = |v: u128| (v / BP as u128).min(u32::MAX as u128) as u32;
    (to_u32(lo), to_u32(hi))
}

#[cfg(test)]
#[path = "hit_tests.rs"]
mod tests;
