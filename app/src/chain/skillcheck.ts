// TypeScript mirror of crates/skillcheck: the skill builder rules the character program enforces.
// Checked against fixtures generated from the Rust crate (crates/skillcheck/tests/ts_fixtures.rs,
// app/src/chain/__fixtures__/skillcheck.json). Change the Rust first, regenerate, then this file.
// Enum values are the on-chain discriminants; the borsh coder accepts them as variant indexes.
// The numbers come from balance.json (deadroll_rules::BalanceConfig::DEFAULT).
import balance from "./balance.json";
import { Aspect, Class } from "./types";

export const Category = { Active: 0, Reactive: 1, Passive: 2 } as const;
export type Category = (typeof Category)[keyof typeof Category];
export const Purpose = { Damage: 0, Buff: 1, Debuff: 2 } as const;
export type Purpose = (typeof Purpose)[keyof typeof Purpose];
/** crates/types Source. */
export const Source = { Magical: 0, Combat: 1 } as const;
export type Source = (typeof Source)[keyof typeof Source];
/** crates/types Form. */
export const Form = { Strike: 0, Projectile: 1 } as const;
export type Form = (typeof Form)[keyof typeof Form];
/** crates/types RangeTag (DAMAGE_TYPES_AND_MIXES 2, the range axis). */
export const RangeTag = { Melee: 0, Ranged: 1 } as const;
export type RangeTag = (typeof RangeTag)[keyof typeof RangeTag];
/** crates/types TimeTag: only a tag for now. */
export const TimeTag = { Instant: 0, OverTime: 1 } as const;
export type TimeTag = (typeof TimeTag)[keyof typeof TimeTag];
export const Target = { Single: 0, Line: 1 } as const;
export type Target = (typeof Target)[keyof typeof Target];
export const Effect = {
  None: 0,
  Weakening: 1,
  Slow: 2,
  Vulnerability: 3,
  Stun: 4,
  Freeze: 5,
  Haste: 6,
  Empowerment: 7,
  Protection: 8,
  Heal: 9,
  /** Patch 9: resistance to the skill's aspects and the bearer's predisposition. */
  Resistance: 10,
} as const;
export type Effect = (typeof Effect)[keyof typeof Effect];
export const Trigger = { None: 0, Hit: 1, CritHit: 2, HpBelow: 3, AllyHpBelow: 4, Controlled: 5, EnemyAdjacent: 6 } as const;
export type Trigger = (typeof Trigger)[keyof typeof Trigger];

export const CATEGORY_NAMES = ["Active", "Reactive", "Passive"] as const;
export const PURPOSE_NAMES = ["Damage", "Buff", "Debuff"] as const;
export const SOURCE_NAMES = ["Magical", "Combat"] as const;
export const FORM_NAMES = ["Strike", "Projectile"] as const;
export const RANGE_NAMES = ["Melee", "Ranged"] as const;
export const TIMING_NAMES = ["Instant", "Over time"] as const;
export const TARGET_NAMES = ["Single", "Line"] as const;
export const EFFECT_NAMES = ["None", "Weakening", "Slow", "Vulnerability", "Stun", "Freeze", "Haste", "Empowerment", "Protection", "Heal", "Resistance"] as const;
/** The effects the builder offers: no Heal until the healing class (docs/CHARACTER.md 8.1). */
export const BUILDER_EFFECTS: readonly Effect[] = [
  Effect.None,
  Effect.Weakening,
  Effect.Slow,
  Effect.Vulnerability,
  Effect.Stun,
  Effect.Freeze,
  Effect.Haste,
  Effect.Empowerment,
  Effect.Protection,
  Effect.Resistance,
];
export const TRIGGER_NAMES = ["None", "Hit", "Crit hit", "HP below", "Ally HP below", "Controlled", "Enemy adjacent"] as const;

export const SKILL_ASPECTS_MAX = 3;
export const AURA_RADIUS_MAX = 3;
export const ACTIVE_MAX = 4;
export const REACTIVE_MAX = 1;
export const PASSIVE_MAX = 1;
export const STARTING_MAX = ACTIVE_MAX + REACTIVE_MAX + PASSIVE_MAX;
/** An empty slot of a map selection. */
export const NO_SKILL = 0xff;
/** Each category's first slot in a map selection and its number of slots (crates/skillcheck SELECTION_SLOTS). */
export const SELECTION_SLOTS: readonly (readonly [Category, number, number])[] = [
  [Category.Active, 0, ACTIVE_MAX],
  [Category.Reactive, ACTIVE_MAX, REACTIVE_MAX],
  [Category.Passive, ACTIVE_MAX + REACTIVE_MAX, PASSIVE_MAX],
];
/** programs/character state/skill_set.rs SKILL_NAME_LEN. */
export const SKILL_NAME_LEN = 24;
/** programs/character state/skill_extras.rs SKILL_DESCRIPTION_LEN. */
export const SKILL_DESCRIPTION_LEN = 200;

const BP = 10_000;
const T = balance.skills;
export const SHARES_BP = T.shares_bp as readonly (readonly number[])[];
export const LINE_COUNTER_BP: number = T.line_counter_bp;
export const EFFECT_COUNTER_BP: number = T.effect_counter_bp;
export const THIRD_ASPECT_COUNTER_BP: number = T.third_aspect_counter_bp;
export const ACTIVE_COOLDOWN: number = T.active_cooldown;
export const REACTIVE_COOLDOWN: number = T.reactive_cooldown;
export const HP_BELOW_MIN_PCT: number = T.hp_below_min_pct;
export const HP_BELOW_MAX_PCT: number = T.hp_below_max_pct;
/** Reach in cells (programs/session rules.rs MELEE_RANGE, RANGED_RANGE). */
export const MELEE_RANGE: number = balance.players.melee_range;
export const RANGED_RANGE: number = balance.players.ranged_range;
/** Action points of the basic attack (an active skill costs its own, `derive().apCost`). */
export const BASIC_ATTACK_AP: number = balance.players.basic_attack_ap;

/** Field order is the borsh order (crates/skillcheck blocks.rs). */
export type SkillBlocks = {
  category: Category;
  purpose: Purpose;
  source: Source;
  form: Form;
  target: Target;
  aspects: [Aspect, Aspect, Aspect];
  aspectCount: number;
  effect: Effect;
  trigger: Trigger;
  auraRadius: number;
};

/** Patch 5 blocks next to SkillBlocks (crates/skillcheck SkillExt), borsh order. */
export type SkillExt = { range: RangeTag; timing: TimeTag; hpBelowPct: number };
export const MELEE_EXT: SkillExt = { range: RangeTag.Melee, timing: TimeTag.Instant, hpBelowPct: 0 };

export type SkillError =
  | "AspectCount"
  | "RepeatedAspect"
  | "UnusedAspectSet"
  | "ThirdAspectTarget"
  | "EffectNeedsOneAspect"
  | "EffectKind"
  | "MissingEffect"
  | "HardControlAspect"
  | "MissingTrigger"
  | "UnexpectedTrigger"
  | "PassiveDamage"
  | "AuraCategory"
  | "AuraEffect"
  | "AuraRadius"
  | "ClassSource"
  | "ClassPurpose"
  | "TooManyActive"
  | "TooManyReactive"
  | "TooManyPassive"
  | "SelectionUnknownSkill"
  | "SelectionRepeated"
  | "SelectionCategory"
  | "SelectionCount"
  | "HealNotAvailable"
  | "MissingThreshold"
  | "UnexpectedThreshold"
  | "ThresholdRange"
  | "PassiveDebuffNeedsAura"
  // Patch 6: monster skills and spawns.
  | "HealCategory"
  | "MonsterSkillUnknown"
  | "MonsterSkillCount";

/** One plain message per rule, for the builder UI. */
export const SKILL_ERROR_MESSAGES: Record<SkillError, string> = {
  AspectCount: "Wrong number of aspects: damage takes 1 to 3, buffs and debuffs exactly 1.",
  RepeatedAspect: "The same aspect is used twice.",
  UnusedAspectSet: "An unused aspect slot must stay empty.",
  ThirdAspectTarget: "A third aspect needs a single target.",
  EffectNeedsOneAspect: "A damage skill with an effect takes exactly one aspect.",
  EffectKind: "This effect does not fit the purpose: buffs take Haste, Empowerment, Protection or Resistance; damage and debuffs take Weakening, Slow, Vulnerability, Stun or Freeze.",
  MissingEffect: "A buff or debuff needs an effect.",
  HardControlAspect: "Stun needs Blunt and Freeze needs Cold as the first aspect.",
  MissingTrigger: "A reactive skill needs a trigger.",
  UnexpectedTrigger: "Only reactive skills have a trigger.",
  PassiveDamage: "A passive skill cannot deal damage: make it a buff or a debuff.",
  AuraCategory: "An active skill cannot be an aura: auras are reactive or passive.",
  AuraEffect: "An aura gives one stat, resistance or increase: a buff or a debuff, not damage, Stun or Freeze.",
  AuraRadius: `Aura radius is at most ${AURA_RADIUS_MAX}.`,
  ClassSource: "Class limit: warriors use combat skills, mages use magical skills.",
  ClassPurpose: "Class limit: support starting skills are buffs or debuffs, not damage.",
  TooManyActive: `At most ${ACTIVE_MAX} active skills.`,
  TooManyReactive: `At most ${REACTIVE_MAX} reactive skill.`,
  TooManyPassive: `At most ${PASSIVE_MAX} passive skill.`,
  SelectionUnknownSkill: "The character has no such skill.",
  SelectionRepeated: "The same skill is chosen twice.",
  SelectionCategory: "A skill is in a slot of another category.",
  SelectionCount: `Take ${ACTIVE_MAX} active, ${REACTIVE_MAX} reactive and ${PASSIVE_MAX} passive skills (all of a kind if you have fewer).`,
  HealNotAvailable: "Healing is not available yet: it comes with its own class.",
  MissingThreshold: "This trigger needs an HP threshold in percent.",
  UnexpectedThreshold: "Only the HP triggers take an HP threshold.",
  ThresholdRange: `The HP threshold is ${HP_BELOW_MIN_PCT} to ${HP_BELOW_MAX_PCT} percent.`,
  PassiveDebuffNeedsAura: "A passive debuff must be an aura: it works on the enemies around.",
  HealCategory: "A heal must be an active or reactive skill, not an aura.",
  MonsterSkillUnknown: "The skill is not in the monster's pool.",
  MonsterSkillCount: "Choose as many skills of each kind as the monster has slots (all of a kind if it has fewer).",
};
/** `SkillError` by its code (the Rust enum's order). */
export const SKILL_ERRORS = Object.keys(SKILL_ERROR_MESSAGES) as SkillError[];

export const isDebuff = (e: Effect): boolean =>
  e === Effect.Weakening || e === Effect.Slow || e === Effect.Vulnerability || e === Effect.Stun || e === Effect.Freeze;
export const isBuff = (e: Effect): boolean =>
  e === Effect.Haste || e === Effect.Empowerment || e === Effect.Protection || e === Effect.Heal || e === Effect.Resistance;
export function hardControlAspect(e: Effect): Aspect | null {
  if (e === Effect.Stun) return Aspect.Blunt;
  if (e === Effect.Freeze) return Aspect.Cold;
  return null;
}
/** `HpBelow` and `AllyHpBelow` take an HP threshold. */
export const usesThreshold = (t: Trigger): boolean => t === Trigger.HpBelow || t === Trigger.AllyHpBelow;

/** Builder rules on the blocks, first failing rule wins (crates/skillcheck rules.rs validate). */
export function validate(s: SkillBlocks): SkillError | null {
  const count = s.aspectCount;
  if (count === 0 || count > SKILL_ASPECTS_MAX) return "AspectCount";
  const used = s.aspects.slice(0, count);
  for (let i = 0; i < used.length; i++) if (used.indexOf(used[i]!) !== i) return "RepeatedAspect";
  if (s.aspects.slice(count).some((a) => a !== Aspect.Fire)) return "UnusedAspectSet";

  if (s.effect === Effect.Heal) return "HealNotAvailable";

  if (s.purpose === Purpose.Damage) {
    if (count === 3 && s.target !== Target.Single) return "ThirdAspectTarget";
    if (s.effect !== Effect.None) {
      if (count !== 1) return "EffectNeedsOneAspect";
      if (!isDebuff(s.effect)) return "EffectKind";
    }
  } else {
    if (count !== 1) return "AspectCount";
    if (s.effect === Effect.None) return "MissingEffect";
    const fits = s.purpose === Purpose.Buff ? isBuff(s.effect) : isDebuff(s.effect);
    if (!fits) return "EffectKind";
  }

  const hard = hardControlAspect(s.effect);
  if (hard !== null && s.aspects[0] !== hard) return "HardControlAspect";

  if (s.category === Category.Reactive) {
    if (s.trigger === Trigger.None) return "MissingTrigger";
  } else if (s.trigger !== Trigger.None) {
    return "UnexpectedTrigger";
  }

  if (s.category === Category.Passive && s.purpose === Purpose.Damage) return "PassiveDamage";
  if (s.category === Category.Passive && s.purpose === Purpose.Debuff && s.auraRadius === 0) return "PassiveDebuffNeedsAura";

  if (s.auraRadius > 0) {
    if (s.category === Category.Active) return "AuraCategory";
    if (s.purpose === Purpose.Damage || hard !== null) return "AuraEffect";
    if (s.auraRadius > AURA_RADIUS_MAX) return "AuraRadius";
  }
  return null;
}

/** validate plus the patch 5 blocks (crates/skillcheck validate_skill): the HP threshold. */
export function validateSkill(s: SkillBlocks, e: SkillExt): SkillError | null {
  const base = validate(s);
  if (base) return base;
  if (usesThreshold(s.trigger)) {
    if (e.hpBelowPct === 0) return "MissingThreshold";
    if (e.hpBelowPct < HP_BELOW_MIN_PCT || e.hpBelowPct > HP_BELOW_MAX_PCT) return "ThresholdRange";
  } else if (e.hpBelowPct !== 0) {
    return "UnexpectedThreshold";
  }
  return null;
}

/** Builder rules plus the class limits on the first six skills. */
export function validateStarting(s: SkillBlocks, e: SkillExt, cls: Class): SkillError | null {
  const base = validateSkill(s, e);
  if (base) return base;
  if (cls === Class.Warrior && s.source !== Source.Combat) return "ClassSource";
  if (cls === Class.Mage && s.source !== Source.Magical) return "ClassSource";
  if (cls === Class.Support && s.purpose === Purpose.Damage) return "ClassPurpose";
  return null;
}

/** At most 4 active, 1 reactive, 1 passive. */
export function checkLoadout(categories: readonly Category[]): SkillError | null {
  const n = (c: Category) => categories.filter((x) => x === c).length;
  if (n(Category.Active) > ACTIVE_MAX) return "TooManyActive";
  if (n(Category.Reactive) > REACTIVE_MAX) return "TooManyReactive";
  if (n(Category.Passive) > PASSIVE_MAX) return "TooManyPassive";
  return null;
}

/**
 * The skills taken onto a map (crates/skillcheck check_map_selection): `owned` is the category of
 * every SkillSet skill by index; slots 0-3 active, 4 reactive, 5 passive, NO_SKILL = empty.
 * Each category must hold exactly min(slots, owned of it).
 */
export function checkMapSelection(owned: readonly Category[], selection: readonly number[]): SkillError | null {
  for (let i = 0; i < selection.length; i++) {
    if (selection[i] !== NO_SKILL && selection.indexOf(selection[i]!, i + 1) !== -1) return "SelectionRepeated";
  }
  for (const [category, start, slots] of SELECTION_SLOTS) {
    let chosen = 0;
    for (const index of selection.slice(start, start + slots)) {
      if (index === NO_SKILL) continue;
      const c = owned[index];
      if (c === undefined) return "SelectionUnknownSkill";
      if (c !== category) return "SelectionCategory";
      chosen++;
    }
    if (chosen !== Math.min(slots, owned.filter((c) => c === category).length)) return "SelectionCount";
  }
  return null;
}

/** A full map selection: the first skills of each category in SkillSet order. */
export function defaultMapSelection(owned: readonly Category[]): number[] {
  const out: number[] = Array.from({ length: STARTING_MAX }, () => NO_SKILL);
  for (const [category, start, slots] of SELECTION_SLOTS) {
    const mine = owned.flatMap((c, i) => (c === category ? [i] : [])).slice(0, slots);
    mine.forEach((index, k) => (out[start + k] = index));
  }
  return out;
}

export type Derived = { sharesBp: [number, number, number]; powerBp: number; fusion: number; cooldown: number; apCost: number };

/** Ordered aspect pair fusion: 1 + first * 6 + second, 0 = none. */
export const fusionId = (first: Aspect, second: Aspect): number => (first === second ? 0 : 1 + first * 6 + second);

/** Counterweight x * BP / (BP + c), floored (u64 in Rust; exact here, values stay far below 2^53). */
const counter = (value: number, c: number): number => Math.floor((value * BP) / (BP + c));

/** Values stored with the skill (crates/skillcheck derive.rs). */
export function derive(s: SkillBlocks): Derived {
  const count = Math.min(Math.max(s.aspectCount, 1), SKILL_ASPECTS_MAX);
  let power = BP;
  if (s.target === Target.Line) power = counter(power, LINE_COUNTER_BP);
  if (s.purpose === Purpose.Damage && s.effect !== Effect.None) power = counter(power, EFFECT_COUNTER_BP);
  if (count === 3) power = counter(power, THIRD_ASPECT_COUNTER_BP);
  const fusion = count === 2 ? fusionId(s.aspects[0], s.aspects[1]) : 0;
  const line = s.target === Target.Line;
  const third = count === 3;
  const hard = hardControlAspect(s.effect) !== null;
  let cooldown = 0;
  let apCost = 0;
  if (s.category === Category.Reactive) cooldown = REACTIVE_COOLDOWN;
  else if (s.category === Category.Active) {
    cooldown = ACTIVE_COOLDOWN + (line ? 1 : 0) + (third ? 1 : 0) + (hard ? 1 : 0);
    apCost =
      T.ap_base +
      (line ? T.ap_line : 0) +
      (third ? T.ap_third_aspect : 0) +
      (s.purpose === Purpose.Damage && s.effect !== Effect.None ? T.ap_effect : 0) +
      (hard ? T.ap_hard_control : 0);
  }
  const shares = SHARES_BP[count - 1]!;
  return { sharesBp: [shares[0]!, shares[1]!, shares[2]!], powerBp: power, fusion, cooldown, apCost: Math.min(apCost, 255) };
}

/** Same rule as programs/character rules.rs name_valid on a 24-byte name: printable ASCII, no leading space. */
export function skillNameValid(name: string): boolean {
  return new RegExp(`^[!-~][ -~]{0,${SKILL_NAME_LEN - 1}}$`).test(name);
}

/** The same rule on a 200-byte description (set_skill_description). */
export function skillDescriptionValid(text: string): boolean {
  return new RegExp(`^[!-~][ -~]{0,${SKILL_DESCRIPTION_LEN - 1}}$`).test(text);
}

/** session `act` takes every Active skill; a buff or debuff without an effect is refused. */
export const usableInCombat = (s: SkillBlocks): boolean =>
  s.category === Category.Active && (s.purpose === Purpose.Damage || s.effect !== Effect.None);
/** Reach of a range tag in cells (programs/session rules.rs `reach`). */
export const rangeReach = (r: RangeTag): number => (r === RangeTag.Melee ? MELEE_RANGE : RANGED_RANGE);

/** Who `act` targets with a skill: heals and buffs go to an ally (self included), the rest to a monster. */
export type Side = "ally" | "enemy";
export const skillSide = (purpose: Purpose): Side => (purpose === Purpose.Buff ? "ally" : "enemy");
/**
 * Reach of `act` at Chebyshev distance `dist` (rules.rs in_range / in_ally_range): allies may
 * share a cell (distance 0), an enemy needs at least 1.
 */
export const inReach = (side: Side, range: RangeTag, dist: number): boolean =>
  (side === "ally" ? dist >= 0 : dist >= 1) && dist <= rangeReach(range);

/** A skill as the builder edits it: blocks, the patch 5 blocks, the player's text. */
export type NamedSkill = { name: string; blocks: SkillBlocks; ext: SkillExt; description: string };

/** Every problem of one starting skill: its first failing rule, the name and the description. */
export function skillProblems(s: NamedSkill, cls: Class): string[] {
  const out: string[] = [];
  if (!skillNameValid(s.name)) out.push(`Name: 1 to ${SKILL_NAME_LEN} printable ASCII characters, not starting with a space.`);
  if (!skillDescriptionValid(s.description))
    out.push(`Description: 1 to ${SKILL_DESCRIPTION_LEN} printable ASCII characters, not starting with a space.`);
  const err = validateStarting(s.blocks, s.ext, cls);
  if (err) out.push(SKILL_ERROR_MESSAGES[err]);
  return out;
}

// ---------------------------------------------------------------------------------------------
// presets (crates/skillcheck presets.rs)

type BlockOpts = { line?: boolean; effect?: Effect; trigger?: Trigger; aura?: number };
export function makeBlocks(category: Category, purpose: Purpose, source: Source, form: Form, aspects: Aspect[], o: BlockOpts = {}): SkillBlocks {
  const a = [...aspects, Aspect.Fire, Aspect.Fire, Aspect.Fire].slice(0, 3) as [Aspect, Aspect, Aspect];
  return {
    category,
    purpose,
    source,
    form,
    target: o.line ? Target.Line : Target.Single,
    aspects: a,
    aspectCount: aspects.length,
    effect: o.effect ?? Effect.None,
    trigger: o.trigger ?? Trigger.None,
    auraRadius: o.aura ?? 0,
  };
}

const { Active, Reactive, Passive } = Category;
const { Damage, Buff, Debuff } = Purpose;
const { Magical, Combat } = Source;
const { Strike, Projectile } = Form;
const ranged: SkillExt = { ...MELEE_EXT, range: RangeTag.Ranged };
const named = (name: string, blocks: SkillBlocks, description: string, ext: SkillExt = MELEE_EXT): NamedSkill => ({ name, blocks, ext, description });

/** The starting sets per class; also the builder's one-click options. */
export const PRESETS: Record<Class, NamedSkill[]> = {
  [Class.Warrior]: [
    named("Piercing Thrust", makeBlocks(Active, Damage, Combat, Strike, [Aspect.Piercing]), "A straight thrust that finds the gap in the armor."),
    named("Shield Bash", makeBlocks(Active, Damage, Combat, Strike, [Aspect.Blunt], { effect: Effect.Stun }), "The shield rim to the head. Sometimes they stay down."),
    named("Javelin", makeBlocks(Active, Damage, Combat, Projectile, [Aspect.Piercing]), "Thrown hard and low, before they close in.", ranged),
    named(
      "Bleeding Cleave",
      makeBlocks(Active, Damage, Combat, Strike, [Aspect.Blood, Aspect.Piercing], { line: true }),
      "A wide arc of steel that opens everyone in a row.",
      ranged,
    ),
    named("Riposte", makeBlocks(Reactive, Damage, Combat, Strike, [Aspect.Piercing], { trigger: Trigger.Hit }), "Hit me once and the blade answers."),
    named("Iron Skin", makeBlocks(Passive, Buff, Combat, Strike, [Aspect.Blunt], { effect: Effect.Protection }), "Years of blows have left a hide like iron."),
  ],
  [Class.Mage]: [
    named("Frost Bolt", makeBlocks(Active, Damage, Magical, Projectile, [Aspect.Cold]), "A shard of ice, thrown from the palm.", ranged),
    named("Frost Burn", makeBlocks(Active, Damage, Magical, Projectile, [Aspect.Cold, Aspect.Fire]), "Cold so deep it burns.", ranged),
    named("Ice Prison", makeBlocks(Active, Damage, Magical, Projectile, [Aspect.Cold], { effect: Effect.Freeze }), "The air around the target turns to ice.", ranged),
    named("Arc", makeBlocks(Active, Damage, Magical, Strike, [Aspect.Lightning], { line: true }), "Lightning runs down the line and spares no one on it.", ranged),
    named(
      "Cold Snap",
      makeBlocks(Reactive, Debuff, Magical, Strike, [Aspect.Cold], { effect: Effect.Slow, trigger: Trigger.EnemyAdjacent }),
      "Step too close and the frost takes your legs.",
    ),
    named("Cold Mind", makeBlocks(Passive, Buff, Magical, Projectile, [Aspect.Cold], { effect: Effect.Empowerment }), "A calm, cold focus behind every spell."),
  ],
  [Class.Support]: [
    named("Quicken", makeBlocks(Active, Buff, Magical, Projectile, [Aspect.Lightning], { effect: Effect.Haste }), "A spark in the blood: move, now.", ranged),
    named("Sap", makeBlocks(Active, Debuff, Magical, Projectile, [Aspect.Blood], { effect: Effect.Weakening }), "Drains the strength out of a foe's arms.", ranged),
    named("Expose", makeBlocks(Active, Debuff, Magical, Projectile, [Aspect.Fire], { effect: Effect.Vulnerability }), "Marks a foe with fire. Fire finds it easier now.", ranged),
    named("Shelter", makeBlocks(Active, Buff, Combat, Strike, [Aspect.Blunt], { effect: Effect.Protection }), "A hand on the shoulder and a shield in front."),
    named(
      "Last Stand",
      makeBlocks(Reactive, Buff, Combat, Strike, [Aspect.Blunt], { effect: Effect.Protection, trigger: Trigger.HpBelow, aura: 2 }),
      "When the blood runs low, everyone near stands behind the shield.",
      { ...MELEE_EXT, hpBelowPct: 40 },
    ),
    named(
      "Cold Ward",
      makeBlocks(Passive, Buff, Magical, Strike, [Aspect.Cold], { effect: Effect.Protection, aura: 2 }),
      "A ring of still, cold air guards those nearby.",
    ),
  ],
};

/** Fixture / debug encoding: discriminants in borsh field order. */
export function blocksToArray(s: SkillBlocks): number[] {
  return [s.category, s.purpose, s.source, s.form, s.target, ...s.aspects, s.aspectCount, s.effect, s.trigger, s.auraRadius];
}
export function blocksFromArray(v: readonly number[]): SkillBlocks {
  return {
    category: v[0] as Category,
    purpose: v[1] as Purpose,
    source: v[2] as Source,
    form: v[3] as Form,
    target: v[4] as Target,
    aspects: [v[5], v[6], v[7]] as [Aspect, Aspect, Aspect],
    aspectCount: v[8]!,
    effect: v[9] as Effect,
    trigger: v[10] as Trigger,
    auraRadius: v[11]!,
  };
}
export const extToArray = (e: SkillExt): number[] => [e.range, e.timing, e.hpBelowPct];
export const extFromArray = (v: readonly number[]): SkillExt => ({ range: v[0] as RangeTag, timing: v[1] as TimeTag, hpBelowPct: v[2]! });
