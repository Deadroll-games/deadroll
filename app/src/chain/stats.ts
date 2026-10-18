// A unit's combat stats with its buffs and debuffs, as the battle screen shows them.
// Pure functions. Player defense and armor come from balance.json (deadroll_rules defaults,
// through ./items); a monster's defense, armor and resistances come from the combat module
// (`monsterSheet`: crates/rules over the defenses the session copied from the registry), so
// nothing here repeats the bestiary. The modifier arithmetic mirrors crates/combat support.rs
// (apply_to_defender, speed_ap); stats.test.ts pins it.
import balance from "./balance.json";
import type { CombatWasm, DefenseInput } from "./combatWasm";
import { heroDefense } from "./items";
import { MOD_NAMES } from "./support";
import { Aspect, Class, ModKind, StateKind, type TimedMod } from "./types";

export type LiveState = { kind: StateKind; turns: number; power: number };

export const BP = 10_000;
/** crates/combat Balance::resist_cap_bp: a hit never uses more resistance than this. */
export const RESIST_CAP_BP: number = balance.combat.resist_cap_bp;
const ASPECT_COUNT = 6;
const NO_RESIST: readonly number[] = Array.from({ length: ASPECT_COUNT }, () => 0);

/** Evasion, armor and resistance per aspect (bp, before the cap). */
export type DefenseProfile = { defense: number; armor: number; resist: readonly number[] };

/** deadroll_rules hero::defense: base + Dexterity / div, class armor, plus what `equipment` (the
 * Participation snapshot, ItemBase bytes by slot) adds. Without it: no items. */
export function playerDefense(cls: Class, stats: readonly number[], equipment: ArrayLike<number> = []): DefenseProfile {
  return { ...heroDefense(cls, stats, equipment), resist: NO_RESIST };
}

/** A monster at `level` with the defenses of its registry record (`MonsterState.defense`), from
 * the combat module; null for an unknown bestiary id. */
export function monsterDefense(combat: CombatWasm, bestiaryId: number, level: number, defense: DefenseInput): DefenseProfile | null {
  try {
    const m = combat.monsterSheet({ bestiaryId, level, defense });
    return { defense: m.evasion, armor: m.armor, resist: m.resistBp };
  } catch {
    return null;
  }
}

/** rules.rs monster_ap: the bestiary speed within the scaling's bounds. */
export const monsterBaseAp = (speed: number): number =>
  Math.min(balance.monsters.ap_max, Math.max(balance.monsters.ap_min, speed));

/** support.rs mod_power: strength of `kind` on the unit, 0 if absent. */
export function modPower(mods: readonly TimedMod[], kind: ModKind): number {
  return mods.find((m) => m.turns > 0 && m.kind === kind)?.powerBp ?? 0;
}

/** Chill buildup, times the balance share, is a speed counterweight (COMBAT_MATH 7.4). */
export const CHILL_SLOW_SHARE_BP: number = balance.combat.chill_slow_share_bp;

/** Slow from a live Chill: power x share / 10_000. */
export function chillSlowBp(states: readonly LiveState[]): number {
  const chill = states.find((s) => s.turns > 0 && s.kind === StateKind.Chill);
  if (!chill) return 0;
  return Math.floor((chill.power * CHILL_SLOW_SHARE_BP) / BP);
}

/** support.rs speed_ap: Haste multiplies, Slow and Chill divide, rounded down, 1..255. */
export function speedAp(base: number, mods: readonly TimedMod[], chillBp = 0): number {
  const slow = modPower(mods, ModKind.Slow) + chillBp;
  const v = Math.floor((base * (BP + modPower(mods, ModKind.Haste))) / (BP + slow));
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
  /** Live states: kind and turns left. */
  states: LiveState[];
};

export type UnitStatsInput = {
  hp: number;
  hpMax: number;
  apBase: number;
  /** AP left in the unit's running turn; null when it is not its turn. */
  apNow: number | null;
  profile: DefenseProfile;
  mods: readonly TimedMod[];
  states?: readonly LiveState[];
};

const name = (k: ModKind) => MOD_NAMES[k];

export function unitStats({ hp, hpMax, apBase, apNow, profile, mods, states = [] }: UnitStatsInput): UnitStats {
  const live = mods.filter((m) => m.turns > 0);
  const liveStates = states.filter((s) => s.turns > 0);
  const haste = modPower(live, ModKind.Haste);
  const slow = modPower(live, ModKind.Slow);
  const chill = chillSlowBp(liveStates);
  const apBy = [...(haste ? [name(ModKind.Haste)] : []), ...(slow ? [name(ModKind.Slow)] : []), ...(chill ? ["Chill"] : [])];
  const protection = modPower(live, ModKind.Protection);
  const vulnMod = live.find((m) => m.kind === ModKind.Vulnerability);
  const resist: AspectStat[] = [];
  const vuln: AspectStat[] = [];
  for (let a = 0; a < ASPECT_COUNT; a++) {
    const aspect = a as Aspect;
    const base = profile.resist[a] ?? 0;
    resist.push({ aspect, base, final: Math.min(RESIST_CAP_BP, base + protection), by: protection ? [name(ModKind.Protection)] : [] });
    const v = vulnMod && vulnMod.aspect === aspect ? vulnMod.powerBp : 0;
    vuln.push({ aspect, base: 0, final: v, by: v ? [name(ModKind.Vulnerability)] : [] });
  }
  return {
    hp,
    hpMax,
    ap: { base: apBase, final: speedAp(apBase, live, chill), by: apBy, now: apNow },
    defense: profile.defense,
    armor: profile.armor,
    resist,
    vuln,
    increasedBp: modPower(live, ModKind.Empowerment),
    weakenBp: modPower(live, ModKind.Weakening),
    mods: live,
    states: liveStates,
  };
}

/** "AP 3 -> 4 (Haste)"; "AP 4" when nothing moved it. */
export function deltaText(label: string, v: StatValue, fmt: (n: number) => string = String): string {
  if (v.base === v.final && v.by.length === 0) return `${label} ${fmt(v.final)}`;
  return `${label} ${fmt(v.base)} -> ${fmt(v.final)}${v.by.length ? ` (${v.by.join(", ")})` : ""}`;
}
