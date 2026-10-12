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
  encodeMonster,
  encodeTile,
  validate_record,
  validate_write,
  loadEncodedTables,
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

test("passable tile with cost 0 throws", () => {
  const json = load("tileset.json") as { records: Array<Record<string, unknown>> };
  json.records[0].cost = 0;
  assert.throws(() => encodeTile(json.records[0]), /passable tile with cost 0/);
});

test("Piercing+Cold ExtraStateChance(Chill, 3000) at effect 1", () => {
  const bytes = encodeResonances(load("resonances.json"));
  const index = 5 * 6 + 1;
  const rec = bytes.subarray(index * RESONANCE_SIZE, (index + 1) * RESONANCE_SIZE);
  assert.equal(rec.length, 16);
  // effect 0 at bytes 2..8: Multiplier 10000
  assert.equal(rec[2], 1);
  assert.equal(rec[3], 0);
  assert.equal(rec[4] | (rec[5] << 8) | (rec[6] << 16) | (rec[7] << 24), 10000);
  // effect 1 at bytes 8..14: ExtraStateChance(Chill, 3000)
  assert.equal(rec[8], 3);
  assert.equal(rec[9], 1);
  assert.equal(rec[10] | (rec[11] << 8) | (rec[12] << 16) | (rec[13] << 24), 3000);
});

test("wolf is record 0 with role 1", () => {
  const bytes = encodeBestiary(load("bestiary.json"));
  const wolf = bytes.subarray(0, MONSTER_SIZE);
  assert.equal(wolf[0], 1);
  assert.equal(wolf[1], 1);
  assert.equal(validate_record(TableKind.Bestiary, 0, wolf), true);
});
