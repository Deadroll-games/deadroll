// TypeScript mirror of crates/skillcheck: the skill builder rules the character program enforces.
// Checked against fixtures generated from the Rust crate (crates/skillcheck/tests/ts_fixtures.rs,
// app/src/chain/__fixtures__/skillcheck.json). Change the Rust first, regenerate, then this file.
// Enum values are the on-chain discriminants; the borsh coder accepts them as variant indexes.
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
} as const;
export type Effect = (typeof Effect)[keyof typeof Effect];
export const Trigger = { None: 0, Hit: 1, CritHit: 2, HpBelow: 3, AllyHpBelow: 4, Controlled: 5, EnemyAdjacent: 6 } as const;
export type Trigger = (typeof Trigger)[keyof typeof Trigger];

export const CATEGORY_NAMES = ["Active", "Reactive", "Passive"] as const;
export const PURPOSE_NAMES = ["Damage", "Buff", "Debuff"] as const;
export const SOURCE_NAMES = ["Magical", "Combat"] as const;
export const FORM_NAMES = ["Strike", "Projectile"] as const;
export const TARGET_NAMES = ["Single", "Line"] as const;
export const EFFECT_NAMES = ["None", "Weakening", "Slow", "Vulnerability", "Stun", "Freeze", "Haste", "Empowerment", "Protection", "Heal"] as const;
export const TRIGGER_NAMES = ["None", "Hit", "Crit hit", "HP below", "Ally HP below", "Controlled", "Enemy adjacent"] as const;

export const SKILL_ASPECTS_MAX = 3;
export const AURA_RADIUS_MAX = 3;
export const ACTIVE_MAX = 4;
export const REACTIVE_MAX = 1;
export const PASSIVE_MAX = 1;
export const STARTING_MAX = ACTIVE_MAX + REACTIVE_MAX + PASSIVE_MAX;
/** programs/character state/skill_set.rs SKILL_NAME_LEN. */
export const SKILL_NAME_LEN = 24;

const BP = 10_000;
export const SHARES_BP = [
  [BP, 0, 0],
  [6_500, 3_500, 0],
  [5_500, 3_000, 1_500],
] as const;
export const LINE_COUNTER_BP = 5_000;
export const EFFECT_COUNTER_BP = 2_500;
export const THIRD_ASPECT_COUNTER_BP = 2_000;
export const ACTIVE_COOLDOWN = 1;
export const REACTIVE_COOLDOWN = 2;

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
  | "TooManyPassive";

/** One plain message per rule, for the builder UI. */
export const SKILL_ERROR_MESSAGES: Record<SkillError, string> = {
  AspectCount: "Wrong number of aspects: damage takes 1 to 3, buffs and debuffs exactly 1.",
  RepeatedAspect: "The same aspect is used twice.",
  UnusedAspectSet: "An unused aspect slot must stay empty.",
  ThirdAspectTarget: "A third aspect needs a single target.",
  EffectNeedsOneAspect: "A damage skill with an effect takes exactly one aspect.",
  EffectKind: "This effect does not fit the purpose: buffs take Haste, Empowerment, Protection or Heal; damage and debuffs take Weakening, Slow, Vulnerability, Stun or Freeze.",
  MissingEffect: "A buff or debuff needs an effect.",
  HardControlAspect: "Stun needs Blunt and Freeze needs Cold as the first aspect.",
  MissingTrigger: "A reactive skill needs a trigger.",
  UnexpectedTrigger: "Only reactive skills have a trigger.",
  PassiveDamage: "A passive skill cannot deal damage: make it a buff or a debuff.",
  AuraCategory: "An active skill cannot be an aura: auras are reactive or passive.",
  AuraEffect: "An aura must be a buff or a debuff, not damage.",
  AuraRadius: `Aura radius is at most ${AURA_RADIUS_MAX}.`,
  ClassSource: "Class limit: warriors use combat skills, mages use magical skills.",
  ClassPurpose: "Class limit: support starting skills are buffs or debuffs, not damage.",
  TooManyActive: `At most ${ACTIVE_MAX} active skills.`,
  TooManyReactive: `At most ${REACTIVE_MAX} reactive skill.`,
  TooManyPassive: `At most ${PASSIVE_MAX} passive skill.`,
};

export const isDebuff = (e: Effect): boolean =>
  e === Effect.Weakening || e === Effect.Slow || e === Effect.Vulnerability || e === Effect.Stun || e === Effect.Freeze;
export const isBuff = (e: Effect): boolean =>
  e === Effect.Haste || e === Effect.Empowerment || e === Effect.Protection || e === Effect.Heal;
export function hardControlAspect(e: Effect): Aspect | null {
  if (e === Effect.Stun) return Aspect.Blunt;
  if (e === Effect.Freeze) return Aspect.Cold;
  return null;
}

/** Builder rules for any skill, first failing rule wins (crates/skillcheck rules.rs validate). */
export function validate(s: SkillBlocks): SkillError | null {
  const count = s.aspectCount;
  if (count === 0 || count > SKILL_ASPECTS_MAX) return "AspectCount";
  const used = s.aspects.slice(0, count);
  for (let i = 0; i < used.length; i++) if (used.indexOf(used[i]!) !== i) return "RepeatedAspect";
  if (s.aspects.slice(count).some((a) => a !== Aspect.Fire)) return "UnusedAspectSet";

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

  if (s.auraRadius > 0) {
    if (s.category === Category.Active) return "AuraCategory";
    if (s.purpose === Purpose.Damage) return "AuraEffect";
    if (s.auraRadius > AURA_RADIUS_MAX) return "AuraRadius";
  }
  return null;
}

/** Builder rules plus the class limits on the first six skills. */
export function validateStarting(s: SkillBlocks, cls: Class): SkillError | null {
  const base = validate(s);
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

export type Derived = { sharesBp: [number, number, number]; powerBp: number; fusion: number; cooldown: number };

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
  let cooldown = 0;
  if (s.category === Category.Reactive) cooldown = REACTIVE_COOLDOWN;
  else if (s.category === Category.Active)
    cooldown = ACTIVE_COOLDOWN + (s.target === Target.Line ? 1 : 0) + (count === 3 ? 1 : 0) + (hardControlAspect(s.effect) !== null ? 1 : 0);
  const shares = SHARES_BP[count - 1]!;
  return { sharesBp: [shares[0], shares[1], shares[2]], powerBp: power, fusion, cooldown };
}

/** Same rule as programs/character rules.rs name_valid on a 24-byte name: printable ASCII, no leading space. */
export function skillNameValid(name: string): boolean {
  return new RegExp(`^[!-~][ -~]{0,${SKILL_NAME_LEN - 1}}$`).test(name);
}

/** session `act` accepts only Active + Damage skills today. */
export const usableInCombat = (s: SkillBlocks): boolean => s.category === Category.Active && s.purpose === Purpose.Damage;
/** Range by form (programs/session rules.rs): Strike 1, Projectile 6. */
export const formRange = (f: Form): number => (f === Form.Strike ? 1 : 6);

/** Every problem of one starting skill: its first failing rule and the name check. */
export function skillProblems(s: { name: string; blocks: SkillBlocks }, cls: Class): string[] {
  const out: string[] = [];
  if (!skillNameValid(s.name)) out.push(`Name: 1 to ${SKILL_NAME_LEN} printable ASCII characters, not starting with a space.`);
  const err = validateStarting(s.blocks, cls);
  if (err) out.push(SKILL_ERROR_MESSAGES[err]);
  return out;
}

// ---------------------------------------------------------------------------------------------
// presets (crates/skillcheck presets.rs)

export type NamedSkill = { name: string; blocks: SkillBlocks };

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

/** The starting sets per class; also the builder's one-click options. */
export const PRESETS: Record<Class, NamedSkill[]> = {
  [Class.Warrior]: [
    { name: "Piercing Thrust", blocks: makeBlocks(Active, Damage, Combat, Strike, [Aspect.Piercing]) },
    { name: "Shield Bash", blocks: makeBlocks(Active, Damage, Combat, Strike, [Aspect.Blunt], { effect: Effect.Stun }) },
    { name: "Javelin", blocks: makeBlocks(Active, Damage, Combat, Projectile, [Aspect.Piercing]) },
    { name: "Bleeding Cleave", blocks: makeBlocks(Active, Damage, Combat, Strike, [Aspect.Blood, Aspect.Piercing], { line: true }) },
    { name: "Riposte", blocks: makeBlocks(Reactive, Damage, Combat, Strike, [Aspect.Piercing], { trigger: Trigger.Hit }) },
    { name: "Iron Skin", blocks: makeBlocks(Passive, Buff, Combat, Strike, [Aspect.Blunt], { effect: Effect.Protection }) },
  ],
  [Class.Mage]: [
    { name: "Frost Bolt", blocks: makeBlocks(Active, Damage, Magical, Projectile, [Aspect.Cold]) },
    { name: "Frost Burn", blocks: makeBlocks(Active, Damage, Magical, Projectile, [Aspect.Cold, Aspect.Fire]) },
    { name: "Ice Prison", blocks: makeBlocks(Active, Damage, Magical, Projectile, [Aspect.Cold], { effect: Effect.Freeze }) },
    { name: "Arc", blocks: makeBlocks(Active, Damage, Magical, Strike, [Aspect.Lightning], { line: true }) },
    { name: "Cold Snap", blocks: makeBlocks(Reactive, Debuff, Magical, Strike, [Aspect.Cold], { effect: Effect.Slow, trigger: Trigger.EnemyAdjacent }) },
    { name: "Cold Mind", blocks: makeBlocks(Passive, Buff, Magical, Projectile, [Aspect.Cold], { effect: Effect.Empowerment }) },
  ],
  [Class.Support]: [
    { name: "Quicken", blocks: makeBlocks(Active, Buff, Magical, Projectile, [Aspect.Lightning], { effect: Effect.Haste }) },
    { name: "Sap", blocks: makeBlocks(Active, Debuff, Magical, Projectile, [Aspect.Blood], { effect: Effect.Weakening }) },
    { name: "Expose", blocks: makeBlocks(Active, Debuff, Magical, Projectile, [Aspect.Fire], { effect: Effect.Vulnerability }) },
    { name: "Shelter", blocks: makeBlocks(Active, Buff, Combat, Strike, [Aspect.Blunt], { effect: Effect.Protection }) },
    { name: "Last Stand", blocks: makeBlocks(Reactive, Buff, Combat, Strike, [Aspect.Blunt], { effect: Effect.Protection, trigger: Trigger.HpBelow, aura: 2 }) },
    { name: "Cold Ward", blocks: makeBlocks(Passive, Buff, Magical, Strike, [Aspect.Cold], { effect: Effect.Protection, aura: 2 }) },
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
