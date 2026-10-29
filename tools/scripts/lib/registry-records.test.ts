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
  encodeTriples,
  tripleOf,
  TRIPLE_SIZE,
  TRIPLES,
  ASPECT,
  check_range,
  TRAIT_SIZE,
  OP_KIND,
  MOD_KIND,
  NATIVE,
  CRIT_ONLY,
  encodeMonster,
  encodeTile,
  validate_record,
  validate_write,
  loadEncodedTables,
  encodeMonsterSkills,
  MONSTER_SKILL_SIZE,
  encodeRewards,
  validate_reward,
  REWARD_SIZE,
  NO_TABLE,
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
  assert.equal(tileset.length, 88);
  assert.equal(tileset.length, 11 * TILE_SIZE);
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
  json.records[1].cost = 0;
  assert.throws(() => encodeTile(json.records[1]), /passable tile with cost 0/);
});

test("Piercing+Cold (Brittle Mark): an override, the target is exposed to crits", () => {
  const bytes = encodeResonances(load("resonances.json"));
  const index = 5 * 6 + 1;
  const rec = bytes.subarray(index * RESONANCE_SIZE, (index + 1) * RESONANCE_SIZE);
  assert.equal(rec.length, 16);
  // condition HasArmor, override flag
  assert.equal(rec[0], 8);
  assert.equal(rec[1], 1);
  // op 0 at bytes 2..8: ApplyMod(Exposed, 10000)
  assert.equal(rec[2], OP_KIND.ApplyMod);
  assert.equal(rec[3], MOD_KIND.Exposed);
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
  assert.equal(hex(encodeTriples(load("triples.json"))), fixture.triples);
  assert.equal(hex(encodeBestiary(load("bestiary.json"))), fixture.bestiary);
  assert.equal(hex(encodeMonsterSkills(load("monster_skills.json"))), fixture.monster_skills);
  assert.equal(hex(encodeRewards(load("reward_tables.json"))), fixture.rewards);
  assert.equal(hex(encodeTileset(load("tileset.json"))), fixture.tileset);
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

test("reward tables: stub file, validator mirrors LootTable::from_record", () => {
  const bytes = encodeRewards(load("reward_tables.json"));
  assert.equal(bytes.length % REWARD_SIZE, 0);
  assert.equal(validate_write(TableKind.Rewards, 0, bytes), true);
  const first = bytes.slice(0, REWARD_SIZE);
  assert.equal(validate_reward(new Uint8Array(REWARD_SIZE)), true, "empty id");
  const bad = (at: number, v: number) => {
    const r = first.slice();
    r[at] = v;
    return !validate_reward(r);
  };
  assert.ok(bad(0, 2), "present");
  assert.ok(bad(3, 1), "reserved");
  assert.ok(bad(1, 0), "base 0");
  assert.ok(bad(2, 9), "more than 4 levels");
  assert.ok(bad(8, 0), "a used level without entries");
  assert.ok(bad(9, 9), "unknown kind");
  assert.ok(bad(12, 0), "qty 0");
  // The bestiary points at a body table, the helpers at none.
  const bestiary = encodeBestiary(load("bestiary.json"));
  assert.equal(bestiary[44], 4);
  assert.equal(bestiary[4 * MONSTER_SIZE + 44], NO_TABLE);
});

test("triples pass: 120 records of 16 bytes, every one with ops, all valid", () => {
  const bytes = encodeTriples(load("triples.json"));
  assert.equal(bytes.length, TRIPLES * TRIPLE_SIZE);
  assert.equal(validate_write(TableKind.Triples, 0, bytes), true);
  for (let i = 0; i < TRIPLES; i++) {
    const rec = bytes.subarray(i * TRIPLE_SIZE, (i + 1) * TRIPLE_SIZE);
    assert.equal(rec[1], 1, `triple ${i} has ops`);
    assert.notEqual(rec[2], 0, `triple ${i} has a first op`);
  }
  // The index order is base-major over the aspects still free.
  assert.deepEqual(tripleOf(0), [0, 1, 2]);
  assert.deepEqual(tripleOf(119), [5, 4, 3]);
  const seen = new Set<string>();
  for (let i = 0; i < TRIPLES; i++) {
    const t = tripleOf(i);
    assert.equal(new Set(t).size, 3);
    seen.add(t.join());
  }
  assert.equal(seen.size, TRIPLES);
});

test("triples pass: Fire + Cold + Lightning jumps the Burning; record bytes", () => {
  const bytes = encodeTriples(load("triples.json"));
  const rec = bytes.subarray(0, TRIPLE_SIZE);
  // [0, set, Jump(Burning, 0), two empty ops, 0, 0]
  assert.deepEqual([...rec.subarray(0, 6)], [0, 1, OP_KIND.Jump, 0, 0, 0]);
  assert.deepEqual([...rec.subarray(6, 16)], [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
});

test("triples pass: Delayed and ChainRadius pack two numbers in the value", () => {
  const triples = load("triples.json") as { records: { base: string; tint: string; second: string; ops: unknown[] }[] };
  const find = (b: string, t: string, s: string) => triples.records.findIndex((r) => r.base === b && r.tint === t && r.second === s);
  const bytes = encodeTriples(triples);
  const at = (b: string, t: string, s: string) => bytes.subarray(find(b, t, s) * TRIPLE_SIZE, (find(b, t, s) + 1) * TRIPLE_SIZE);
  // Cold + Fire + Blood: [ApplyMod Weakening, Delayed Bleeding after Freeze]
  let rec = at("Cold", "Fire", "Blood");
  assert.equal(rec[6], OP_KIND.Delayed);
  assert.equal(rec[7], 3);
  assert.equal(rec[8] | (rec[9] << 8), 2);
  // Cold + Piercing + Blood: Delayed Bleeding after Freeze, 200 percent
  rec = at("Cold", "Piercing", "Blood");
  assert.equal(rec[4] | (rec[5] << 8), 2 | (200 << 8));
  // Fire + Blood + Lightning: [ApplyState Shock, ChainRadius Shock radius 2 filter Bleeding]
  rec = at("Fire", "Blood", "Lightning");
  assert.equal(rec[6], OP_KIND.ChainRadius);
  assert.equal(rec[7], 4);
  assert.equal(rec[8] | (rec[9] << 8), 2 | (3 << 8));
});

test("triples pass: bad records and bad values are refused", () => {
  const good = encodeTriples(load("triples.json")).slice(0, TRIPLE_SIZE);
  const bad = (at: number, v: number) => {
    const x = good.slice();
    x[at] = v;
    return validate_record(TableKind.Triples, 0, x);
  };
  assert.equal(validate_record(TableKind.Triples, 0, new Uint8Array(TRIPLE_SIZE)), true);
  assert.equal(bad(0, 1), false, "reserved byte");
  assert.equal(bad(1, 2), false, "unknown flag");
  assert.equal(bad(14, 1), false, "reserved byte");
  assert.equal(bad(2, 41), false, "unknown op kind");
  assert.equal(validate_record(TableKind.Triples, 0, good.subarray(0, 15)), false);
  assert.equal(check_range(TableKind.Triples, 0, TRIPLES * TRIPLE_SIZE), TRIPLES * TRIPLE_SIZE);
  assert.equal(check_range(TableKind.Triples, 0, (TRIPLES + 1) * TRIPLE_SIZE), "OutOfBounds");
  // A value past 16 bits does not fit a triple op; a Dot op takes Burning or Bleeding only.
  const one = (op: unknown) => ({
    records: Array.from({ length: TRIPLES }, (_, i) => {
      const [b, t, s] = tripleOf(i);
      const names = Object.keys(ASPECT);
      return { base: names[b], tint: names[t], second: names[s], ops: i === 0 ? [op] : [] };
    }),
  });
  assert.throws(() => encodeTriples(one({ kind: "Penetrate", value: 70000 })), /16 bits/);
  assert.throws(() => encodeTriples(one({ kind: "Escalate", arg: "Shock" })), /Burning or Bleeding/);
  assert.throws(() => encodeTriples(one({ kind: "Delayed", arg: "Bleeding" })), /unknown state/);
  assert.throws(() => encodeTriples(one({ kind: "ChainRadius", arg: "Shock", filter: "Any" })), /radius/);
  assert.throws(() => encodeTriples({ records: [] }), /120 records/);
  const out = encodeTriples(one({ kind: "ApplyMod", arg: "Dulled", value: 10000 }));
  assert.equal(out[2], OP_KIND.ApplyMod);
  assert.equal(out[3], MOD_KIND.Dulled);
});
