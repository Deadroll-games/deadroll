// Monsters in the app (docs/BESTIARY.md 3, patch 6): their skills, names and auras as the session
// copied them from the registry. Every number comes from the combat module over the copied bytes
// (`MonsterState.records`, `skill_table`); this file only finds and labels them.
import { useEffect, useState } from "react";
import { DATA_SOURCE } from "../config";
import { auraInputOf, type BearerInput, type CombatWasm, type MonsterSkillSheet, type SpawnSkillsResult } from "./combatWasm";
import { registryBestiary, registryMonsterSkills, registryRewards, type Party } from "./game";
import { mockTable } from "./mock";
import { MONSTER_SKILL_NAMES } from "./names";
import type { Spawn } from "./types";

/** Monster skill slots (`deadroll_rules::monster`): `0..5` active, `5` reactive, `6` passive. */
export const ACTIVE_SLOTS = 5;
export const REACTIVE_SLOT = 5;
export const PASSIVE_SLOT = 6;
/** `monster_act` action of a skill slot (0 = the natural attack). */
export const skillAction = (slot: number) => 1 + slot;
/** Bestiary record bytes (`deadroll_rules::monster::record`): size, pool, kin. */
const RECORD_SIZE = 48;
const POOL_START_AT = 39;
const POOL_LEN_AT = 40;
const KIN_AT = 41;
/** Monster skill records (`deadroll_skillcheck::monster`): size, the category byte. */
const SKILL_RECORD_SIZE = 32;
const SKILL_CATEGORY_AT = 1;
/** `Spawn.flags` bit: an elite spawn (`deadroll_types::SPAWN_ELITE`). */
export const SPAWN_ELITE = 1;

/** A monster's skill as the DM panel shows it. */
export type MonsterSkill = { slot: number; name: string; sheet: MonsterSkillSheet; copy: Uint8Array };

/** The registry `monster_skills` body, read once per page: the skill names' indices. */
let tableLoad: Promise<Uint8Array | null> | null = null;
export function useMonsterSkillTable(): Uint8Array | null {
  const [table, setTable] = useState<Uint8Array | null>(null);
  useEffect(() => {
    let live = true;
    tableLoad ??= registryMonsterSkills().catch(() => null);
    void tableLoad.then((t) => live && setTable(t));
    return () => {
      live = false;
    };
  }, []);
  return table;
}

/** The registry name of a session skill copy: the record whose bytes `1..17` it is (names are
 * off-chain text, by registry index). */
export function monsterSkillName(copy: Uint8Array, table: Uint8Array | null): string {
  if (table) {
    for (let i = 0; i * 32 + 17 <= table.length; i++) {
      const r = table.subarray(i * 32 + 1, i * 32 + 17);
      if (r.every((b, j) => b === copy[j])) return MONSTER_SKILL_NAMES[i] ?? `skill ${i}`;
    }
  }
  return "monster skill";
}

/** Monster `i`'s skills by slot, decoded by the combat module. */
export function monsterSkillsOf(party: Party, i: number, combat: CombatWasm, table: Uint8Array | null): MonsterSkill[] {
  const out: MonsterSkill[] = [];
  (party.monsterSkills[i] ?? []).forEach((copy, slot) => {
    if (!copy) return;
    try {
      out.push({ slot, name: monsterSkillName(copy, table), sheet: combat.monsterSkill(copy), copy });
    } catch {
      // A copy the module refuses never happens (the registry validated it); skip it.
    }
  });
  return out;
}

/** The kin of monster `i` (its bestiary record). */
export const kinOf = (party: Party, i: number) => party.monsterRecords[i]?.[KIN_AT] ?? 0xff;

/** The living monsters that bear an aura (`MonsterState.auras`), for the module's party input. */
export function monsterBearersOf(party: Party): BearerInput[] {
  return party.monsters.flatMap((m, i) => {
    const auras = party.monsterAuras[i] ?? [];
    if (!m.alive || !auras.some((a) => a.aura.turns > 0)) return [];
    return [{ index: i, cell: [m.x, m.y] as const, kin: kinOf(party, i), auras: auras.map((a) => ({ aura: auraInputOf(a.aura), kinOnly: a.kinOnly })) }];
  });
}

/** The registry tables the map builder reads: from the chain, or in mock mode the bundled
 * encoding of the same JSON. */
/** Patch 11: `rewards` is the registry reward tables body (chests and the boss table). */
export type BuilderRegistry = { bestiary: Uint8Array; skills: Uint8Array; rewards: Uint8Array };
export function useBuilderRegistry(): BuilderRegistry | null {
  const [reg, setReg] = useState<BuilderRegistry | null>(null);
  useEffect(() => {
    let live = true;
    const load = async (): Promise<BuilderRegistry | null> => {
      if (DATA_SOURCE !== "chain") return { bestiary: mockTable("bestiary"), skills: mockTable("monster_skills"), rewards: mockTable("rewards") };
      const [bestiary, skills, rewards] = await Promise.all([registryBestiary(), registryMonsterSkills(), registryRewards()]);
      return bestiary && skills && rewards ? { bestiary, skills, rewards } : null;
    };
    load().then(
      (r) => live && setReg(r),
      () => live && setReg(null),
    );
    return () => {
      live = false;
    };
  }, []);
  return reg;
}

/** One skill of a template's pool: its bit in `Spawn.skills`, registry index and category. */
export type PoolSkill = { bit: number; index: number; name: string; category: number };
/** A spawn as the builder shows it: the pool, the slots and the module's verdict. */
export type SpawnView = { record: Uint8Array; pool: PoolSkill[]; check: SpawnSkillsResult | null; boss: boolean };

/** The bestiary record of a template id, or null. */
export function bestiaryRecord(reg: BuilderRegistry, id: number): Uint8Array | null {
  const r = reg.bestiary.subarray(id * RECORD_SIZE, (id + 1) * RECORD_SIZE);
  return r.length === RECORD_SIZE && r[0] === 1 ? r : null;
}

/** A spawn's pool, slots and verdict (`check_monster_selection` through the combat module, as
 * `finalize_map` runs it). */
export function spawnView(combat: CombatWasm, reg: BuilderRegistry, spawn: Pick<Spawn, "bestiaryId" | "flags" | "skills">): SpawnView | null {
  const record = bestiaryRecord(reg, spawn.bestiaryId);
  if (!record) return null;
  const start = record[POOL_START_AT]!;
  const pool: PoolSkill[] = Array.from({ length: record[POOL_LEN_AT]! }, (_, bit) => {
    const index = start + bit;
    return { bit, index, name: MONSTER_SKILL_NAMES[index] ?? `skill ${index}`, category: reg.skills[index * SKILL_RECORD_SIZE + SKILL_CATEGORY_AT] ?? 0 };
  });
  let check: SpawnSkillsResult | null = null;
  let boss = false;
  try {
    check = combat.spawnSkills({ record, flags: spawn.flags, categories: pool.map((k) => k.category), mask: spawn.skills });
    boss = combat.monsterSheet({ record, level: 1, flags: 0 }).boss;
  } catch {
    // A record the module refuses: no verdict, finalize_map has the last word.
  }
  return { record, pool, check, boss };
}
