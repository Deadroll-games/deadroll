// Patch C: the TS Merkle tree against the Rust one (crates/mapcheck/wasm/tests/secrets_fixture.rs),
// the encrypted copy round trip, the split of a design.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { buildTree, cellLeaf, commit, decodeSecrets, deriveKeys, encodeSecrets, Leaves, node, openSecrets, packLeaf, saltOf, sealSecrets, Secret, splitDesign, threat, emptyRoster, NO_SPAWN, type Roster } from "../src/secrets.ts";

const fx = JSON.parse(readFileSync(new URL("./fixtures/secrets.json", import.meta.url), "utf8"));
const unhex = (h: string) => Uint8Array.from(h.match(/../g)!.map((b) => parseInt(b, 16)));
const hex = (b: Uint8Array) => Buffer.from(b).toString("hex");

test("leaves, nodes, the root and the proofs match the program's", async () => {
  const map = unhex(fx.map);
  const leaves = new Leaves(fx.width, fx.height, fx.packs);
  assert.equal(leaves.count, fx.leafCount);
  const roster: Roster = fx.roster;
  const byLeaf = new Map<number, any>(fx.secrets.map((s: any) => [s.leaf, s]));
  const hashes: Uint8Array[] = [];
  for (let i = 0; i < leaves.count; i++) {
    const salt = unhex(fx.salt[i]);
    if (i < 8) hashes.push(await packLeaf(map, i, i === 1 ? roster : emptyRoster(), salt));
    else hashes.push(await cellLeaf(map, i, byLeaf.get(i) ?? { kind: Secret.None, a: 0, b: 0, c: 0 }, salt));
  }
  assert.deepEqual(hashes.map(hex), fx.leaves);
  assert.equal(hex(await node(hashes[0], hashes[1])), fx.node);
  const tree = await buildTree(hashes);
  assert.equal(hex(tree.root), fx.root);
  for (const p of fx.proofs) assert.deepEqual(tree.proof(p.index).map(hex), p.proof);
  for (let leaf = 0; leaf < leaves.count; leaf++) assert.equal(leaves.of(leaves.board(leaf)!), leaf);
  assert.equal(leaves.board(leaves.count), null);
});

test("the encrypted copy opens with the same signature only", async () => {
  const map = new Uint8Array(32).fill(5);
  const sig = new Uint8Array(64).fill(9);
  const keys = await deriveKeys(sig, map);
  const secrets = { rosters: [emptyRoster(), fx.roster], cells: [{ leaf: 30, secret: { kind: Secret.Trap, a: 4, b: 10, c: 0 } }] } as any;
  const blob = await sealSecrets(keys, map, secrets);
  assert.ok(blob.length <= 800);
  const back = await openSecrets(await deriveKeys(sig, map), map, blob);
  assert.deepEqual(back.cells, secrets.cells);
  assert.deepEqual(back.rosters[1], fx.roster);
  assert.deepEqual(decodeSecrets(encodeSecrets(back)), back);
  const other = new Uint8Array(64).fill(8);
  await assert.rejects(openSecrets(await deriveKeys(other, map), map, blob));
  await assert.rejects(openSecrets(keys, new Uint8Array(32).fill(6), blob));
  // The salts are fixed by the key: another device derives the same ones.
  assert.equal(hex(await saltOf(keys, 7)), hex(await saltOf(await deriveKeys(sig, map), 7)));
  const c = await commit(map, keys, 10, 9, [null, { width: 6, height: 7 }], secrets);
  const reveal = await c.cell(30);
  assert.equal(reveal.secret.kind, Secret.Trap);
  assert.equal(reveal.proof.length, 8);
});

test("a design splits into visible monsters and rosters", () => {
  const s = (pack: number, level = 1, bestiaryId = 0) => ({ bestiaryId, x: 0, y: 0, level, skills: 0, flags: 0, aspects: 0, pack });
  const packs = [null, { visible: 1, x: 3, y: 3, facing: 0, width: 6, height: 6 }, { visible: NO_SPAWN, x: 0, y: 0, facing: 2, width: 6, height: 6 }];
  const out = splitDesign([s(1), s(1, 2), s(2)], packs);
  assert.equal(out.spawns.length, 1);
  assert.equal(out.packs[1]!.visible, 0);
  assert.equal(out.rosters[1].count, 2);
  assert.equal(out.rosters[1].spawns[0].level, 2, "visible first");
  assert.equal(out.rosters[2].count, 1);
  assert.equal(threat([s(1, 2), { ...s(1, 1), flags: 1 }, s(1, 4, 9)], (id) => id === 9), 2 + 2 + 12);
});
