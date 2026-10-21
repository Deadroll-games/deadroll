import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  ASPECT_SIZE,
  RESONANCE_SIZE,
  MONSTER_SIZE,
  TILE_SIZE,
  TableKind,
  registryAsset,
  encodeAspects,
  encodeResonances,
  encodeBestiary,
  encodeTileset,
  encodePrefabs,
  encodeTraits,
  TRAIT_SIZE,
  OP_KIND,
  NATIVE,
  CRIT_ONLY,
  encodeMonster,
  encodeTile,
  validate_record,
  validate_write,
  loadEncodedTables,
  encodeMonsterSkills,
  MONSTER_SKILL_SIZE,
} from "./registry-records.ts";

function load(name: string): unknown {
  return JSON.parse(readFileSync(registryAsset(name), "utf8"));
}

test("JSON files encode to the exact table sizes", () => {
  const aspects = encodeAspects(load("aspects.json"));
  const resonances = encodeResonances(load("resonances.json"));
  const bestiary = encodeBestiary(load("bestiary.json"));
  const tileset = encodeTileset(load("tileset.json"));
  const prefabs = encodePrefabs(load("prefabs.json"));
  const traits = encodeTraits(load("traits.json"));
  assert.equal(traits.length, 6 * TRAIT_SIZE);
  assert.equal(aspects.length, 48);
  assert.equal(aspects.length, 6 * ASPECT_SIZE);
  assert.equal(resonances.length, 576);
  assert.equal(resonances.length, 36 * RESONANCE_SIZE);
  assert.equal(bestiary.length, 5 * 48);
  assert.equal(bestiary.length, 240);
  assert.equal(tileset.length, 56);
  assert.equal(tileset.length, 7 * TILE_SIZE);
  assert.equal(prefabs.length, 0);
});

test("encoded bytes pass validators that mirror validate_record", () => {
  const tables = loadEncodedTables();
  for (const t of tables) {
    if (t.bytes.length === 0) continue;
    assert.equal(validate_write(t.kind, 0, t.bytes), true, t.table);
    const size = t.recordSize;
    for (let i = 0; i < t.bytes.length / size; i++) {
      assert.equal(
        validate_record(t.kind, i, t.bytes.subarray(i * size, (i + 1) * size)),
        true,
        `${t.table} record ${i}`,
      );
    }
  }
});

test("unknown role name throws", () => {
  const json = load("bestiary.json") as { records: Array<Record<string, unknown>> };
  json.records[0].role = "Ninja";
  assert.throws(() => encodeBestiary(json), /unknown role/);
});

test("damage min above max throws", () => {
  const json = load("bestiary.json") as { records: Array<Record<string, unknown>> };
  json.records[0].damage_min = 9;
  json.records[0].damage_max = 1;
  assert.throws(() => encodeMonster(json.records[0]), /damage min above max/);
});

test("speed 0 throws", () => {
  const json = load("bestiary.json") as { records: Array<Record<string, unknown>> };
  json.records[0].speed = 0;
  assert.throws(() => encodeMonster(json.records[0]), /speed 0/);
});

test("monster defenses by tag (patch 9)", () => {
  const json = load("bestiary.json") as { records: Record<string, unknown>[] };
  const wolf = { ...json.records[0], tag_resistance: ["Magical"], tag_immunity: ["Projectile"] };
  const r = encodeMonster(wolf);
  assert.equal(r[42], 0b0001);
  assert.equal(r[43], 0b1000);
  assert.ok(validate_record(TableKind.Bestiary, 0, r));
  r[43] = 0b1_0000;
  assert.ok(!validate_record(TableKind.Bestiary, 0, r));
  assert.throws(() => encodeMonster({ ...wolf, tag_immunity: ["Fire"] }), /tag/);
});

test("passable tile with cost 0 throws", () => {
  const json = load("tileset.json") as { records: Array<Record<string, unknown>> };
  json.records[0].cost = 0;
  assert.throws(() => encodeTile(json.records[0]), /passable tile with cost 0/);
});

test("Piercing+Cold (Icy Point): an override, Penetration chills", () => {
  const bytes = encodeResonances(load("resonances.json"));
  const index = 5 * 6 + 1;
  const rec = bytes.subarray(index * RESONANCE_SIZE, (index + 1) * RESONANCE_SIZE);
  assert.equal(rec.length, 16);
  // condition HasArmor, override flag
  assert.equal(rec[0], 8);
  assert.equal(rec[1], 1);
  // op 0 at bytes 2..8: Linked(Chill, 10000)
  assert.equal(rec[2], OP_KIND.Linked);
  assert.equal(rec[3], 1);
  assert.equal(rec[4] | (rec[5] << 8) | (rec[6] << 16) | (rec[7] << 24), 10000);
  // op 1 empty
  assert.deepEqual([...rec.subarray(8, 16)], [0, 0, 0, 0, 0, 0, 0, 0]);
});

test("wolf is record 0 with role 1", () => {
  const bytes = encodeBestiary(load("bestiary.json"));
  const wolf = bytes.subarray(0, MONSTER_SIZE);
  assert.equal(wolf[0], 1);
  assert.equal(wolf[1], 1);
  assert.equal(validate_record(TableKind.Bestiary, 0, wolf), true);
});

test("the encoder gives the bytes deadroll-rules reads (sim and program)", () => {
  const fixture = JSON.parse(
    readFileSync(new URL("./__fixtures__/registry-tables.json", import.meta.url), "utf8"),
  ) as Record<string, string>;
  const hex = (b: Uint8Array) => Buffer.from(b).toString("hex");
  assert.equal(hex(encodeAspects(load("aspects.json"))), fixture.aspects);
  assert.equal(hex(encodeTraits(load("traits.json"))), fixture.traits);
  assert.equal(hex(encodeResonances(load("resonances.json"))), fixture.resonances);
  assert.equal(hex(encodeBestiary(load("bestiary.json"))), fixture.bestiary);
  assert.equal(hex(encodeMonsterSkills(load("monster_skills.json"))), fixture.monster_skills);
});

test("patch 6: monster skill records and the bestiary pools", () => {
  const skills = encodeMonsterSkills(load("monster_skills.json"));
  assert.equal(skills.length % MONSTER_SKILL_SIZE, 0);
  assert.equal(validate_write(TableKind.MonsterSkills, 0, skills), true);
  const first = skills.slice(0, MONSTER_SKILL_SIZE);
  const bad = (at: number, v: number) => {
    const x = first.slice();
    x[at] = v;
    return validate_record(TableKind.MonsterSkills, 0, x);
  };
  assert.equal(bad(0, 2), false, "present flag");
  assert.equal(bad(1, 3), false, "unknown category");
  assert.equal(bad(16, 2), false, "unknown flag");
  assert.equal(bad(31, 1), false, "reserved byte");
  assert.equal(validate_record(TableKind.MonsterSkills, 0, new Uint8Array(MONSTER_SKILL_SIZE)), true, "empty slot");
  // The chieftain: its pool and its kin.
  const beasts = encodeBestiary(load("bestiary.json"));
  const chief = beasts.subarray(2 * MONSTER_SIZE, 3 * MONSTER_SIZE);
  assert.deepEqual([chief[39], chief[40], chief[41]], [6, 5, 1]);
  const pool = beasts.slice(0, MONSTER_SIZE);
  pool[40] = 17;
  assert.equal(validate_record(TableKind.Bestiary, 0, pool), false, "pool past 16");
  assert.throws(() => encodeMonsterSkills({ records: [{ id: 0, name: "x", category: "Active", purpose: "Damage", source: "Combat", form: "Strike", aspects: [], range: "Melee" }] }));
});

test("ops: args by kind, crit flag, bad ops refused", () => {
  const r = new Uint8Array(RESONANCE_SIZE);
  r[0] = 1;
  r[1] = 1;
  r[2] = OP_KIND.Jump;
  r[3] = NATIVE;
  r[8] = OP_KIND.NativeTurns | CRIT_ONLY;
  assert.equal(validate_record(TableKind.Resonances, 0, r), true);
  const bad = (at: number, v: number) => {
    const x = r.slice();
    x[at] = v;
    return validate_record(TableKind.Resonances, 0, x);
  };
  assert.equal(bad(9, 1), false, "NativeTurns takes no arg");
  assert.equal(bad(2, 21), false, "unknown op");
  assert.equal(bad(1, 2), false, "unknown flag");
  assert.equal(bad(3, 7), false, "unknown state");
  assert.throws(() =>
    encodeResonances({
      records: Array.from({ length: 36 }, (_, i) => ({
        base: ["Fire", "Cold", "Lightning", "Blood", "Blunt", "Piercing"][Math.floor(i / 6)],
        tint: ["Fire", "Cold", "Lightning", "Blood", "Blunt", "Piercing"][i % 6],
        override: true,
        condition: "Always",
        ops: [{ kind: "Linked" }],
      })),
    }),
  );
});
