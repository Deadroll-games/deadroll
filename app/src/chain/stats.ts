// A unit's combat stats with its buffs and debuffs, as the battle screen shows them.
// Pure functions. The numbers mirror programs/session/src/rules.rs (monster_profile,
// player_defense, monster_ap) and crates/combat support.rs (apply_to_attacker /
// apply_to_defender / speed_ap): keep them in sync. TODO(balance): every rules.rs number is a
// placeholder; stats.test.ts pins the current ones.
import { MOD_NAMES } from "./support";
import { Aspect, Class, ModKind, type TimedMod } from "./types";

export const BP = 10_000;
/** crates/combat Balance::resist_cap_bp: a hit never uses more resistance than this. */
export const RESIST_CAP_BP = 30_000;
const ASPECT_COUNT = 6;

export type DefenseProfile = { defense: number; armor: number; resist: readonly [Aspect, number] | null };

/** rules.rs monster_profile: defense, armor and the one special resistance, by P0 bestiary id. */
export const MONSTER_DEFENSE: Readonly<Record<number, DefenseProfile>> = {
  0: { defense: 10, armor: 5, resist: [Aspect.Cold, 5_000] }, // wolf
  1: { defense: 9, armor: 5, resist: null }, // goblin
  2: { defense: 13, armor: 40, resist: [Aspect.Piercing, 5_000] }, // goblin chieftain
  3: { defense: 11, armor: 25, resist: null }, // goblin warrior
  4: { defense: 10, armor: 5, resist: [Aspect.Fire, 5_000] }, // goblin shaman
};

/** rules.rs player_defense: 10 + Dexterity / 5; warriors wear 30 armor, others 5. */
export function playerDefense(cls: Class, stats: readonly number[]): DefenseProfile {
  return { defense: 10 + Math.floor((stats[1] ?? 0) / 5), armor: cls === Class.Warrior ? 30 : 5, resist: null };
}

/** rules.rs monster_ap: the bestiary speed, 1..12. */
export const monsterBaseAp = (speed: number): number => Math.min(12, Math.max(1, speed));

/** support.rs mod_power: strength of `kind` on the unit, 0 if absent. */
export function modPower(mods: readonly TimedMod[], kind: ModKind): number {
  return mods.find((m) => m.turns > 0 && m.kind === kind)?.powerBp ?? 0;
}

/** support.rs speed_ap: Haste multiplies, Slow divides, rounded down, 1..255. */
export function speedAp(base: number, mods: readonly TimedMod[]): number {
  const v = Math.floor((base * (BP + modPower(mods, ModKind.Haste))) / (BP + modPower(mods, ModKind.Slow)));
  return Math.min(255, Math.max(1, v));
}

/** A number before and after the modifiers, with the names of the modifiers that moved it. */
export type StatValue = { base: number; final: number; by: string[] };
export type AspectStat = StatValue & { aspect: Aspect };

export type UnitStats = {
  hp: number;
  hpMax: number;
  /** Action points: `base` from speed, `final` per turn with Haste / Slow, `now` left in the running turn. */
  ap: StatValue & { now: number | null };
  defense: number;
  armor: number;
  /** Resistance per aspect (counterweight, bp), capped at RESIST_CAP_BP like the hit. */
  resist: AspectStat[];
  /** Vulnerability per aspect (multiplier, bp). */
  vuln: AspectStat[];
  /** Outgoing damage: Empowerment (increase, bp) and Weakening (counterweight, bp). */
  increasedBp: number;
  weakenBp: number;
  mods: TimedMod[];
};

export type UnitStatsInput = {
  hp: number;
  hpMax: number;
  apBase: number;
  /** AP left in the unit's running turn; null when it is not its turn. */
  apNow: number | null;
  profile: DefenseProfile;
  mods: readonly TimedMod[];
};

const name = (k: ModKind) => MOD_NAMES[k];

export function unitStats({ hp, hpMax, apBase, apNow, profile, mods }: UnitStatsInput): UnitStats {
  const live = mods.filter((m) => m.turns > 0);
  const haste = modPower(live, ModKind.Haste);
  const slow = modPower(live, ModKind.Slow);
  const apBy = [...(haste ? [name(ModKind.Haste)] : []), ...(slow ? [name(ModKind.Slow)] : [])];
  const protection = modPower(live, ModKind.Protection);
  const vulnMod = live.find((m) => m.kind === ModKind.Vulnerability);
  const resist: AspectStat[] = [];
  const vuln: AspectStat[] = [];
  for (let a = 0; a < ASPECT_COUNT; a++) {
    const aspect = a as Aspect;
    const base = profile.resist && profile.resist[0] === aspect ? profile.resist[1] : 0;
    resist.push({ aspect, base, final: Math.min(RESIST_CAP_BP, base + protection), by: protection ? [name(ModKind.Protection)] : [] });
    const v = vulnMod && vulnMod.aspect === aspect ? vulnMod.powerBp : 0;
    vuln.push({ aspect, base: 0, final: v, by: v ? [name(ModKind.Vulnerability)] : [] });
  }
  return {
    hp,
    hpMax,
    ap: { base: apBase, final: speedAp(apBase, live), by: apBy, now: apNow },
    defense: profile.defense,
    armor: profile.armor,
    resist,
    vuln,
    increasedBp: modPower(live, ModKind.Empowerment),
    weakenBp: modPower(live, ModKind.Weakening),
    mods: live,
  };
}

/** "AP 3 -> 4 (Haste)"; "AP 4" when nothing moved it. */
export function deltaText(label: string, v: StatValue, fmt: (n: number) => string = String): string {
  if (v.base === v.final && v.by.length === 0) return `${label} ${fmt(v.final)}`;
  return `${label} ${fmt(v.base)} -> ${fmt(v.final)}${v.by.length ? ` (${v.by.join(", ")})` : ""}`;
}
