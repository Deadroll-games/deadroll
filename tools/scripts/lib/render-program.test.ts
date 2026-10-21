import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createNoopSigner } from "@solana/kit";
import * as render from "@deadroll/sdk/render";
import {
  blobFits,
  planUpload,
  parseHeader,
  writeChunkInstruction,
  setComputeUnitLimitData,
  FINALIZE_CU,
  GAME_CONFIG,
  REGISTRY_PROGRAM,
  finalizeInstruction,
  renderAuthorityPda,
} from "./render-program.ts";

const EXPECTED: Record<string, string> = {
  init_rasterizer: "82bff24f597b42d2",
  init_palette: "7ed0d30cb6daa922",
  init_sprite_book: "2f2e5ac9c2467b79",
  extend: "e47f0001e39a36a8",
  write_chunk: "5d8da70fd1858933",
  finalize_rasterizer: "2680cb91554cc31e",
  finalize_palette: "2201740fd300a563",
  finalize_sprite_book: "80761daaae3cf1fe",
};

test("the sdk's discriminators match the render program", () => {
  for (const [name, hex] of Object.entries(EXPECTED)) {
    const bytes = (render as Record<string, unknown>)[`${name.toUpperCase()}_DISCRIMINATOR`] as Uint8Array;
    assert.equal(bytes.length, 8, name);
    assert.equal(Buffer.from(bytes).toString("hex"), hex, name);
  }
});

test("write_chunk data for offset 900 and three bytes", () => {
  const signer = createNoopSigner("11111111111111111111111111111112" as never);
  const ix = writeChunkInstruction(signer, "11111111111111111111111111111113", 900, new Uint8Array([1, 2, 3]));
  assert.equal(Buffer.from(ix.data!).toString("hex"), "5d8da70fd18589338403000003000000010203");
});

test("finalize asks for 400_000 compute units", () => {
  assert.equal(Buffer.from(setComputeUnitLimitData(FINALIZE_CU)).toString("hex"), "02801a0600");
});

test("planUpload sizes", async () => {
  const bigBody = new Uint8Array(20257);
  const big = await planUpload("rasterizer", 1, bigBody);
  assert.equal(big.len, 20257);
  assert.equal(big.initSize, 10240);
  assert.equal(big.extends, 1);
  assert.equal(big.chunks, 23);
  assert.equal(big.sha256, createHash("sha256").update(bigBody).digest("hex"));
  assert.equal(typeof big.address, "string");
  assert.ok(big.address.length > 0);

  const small = await planUpload("palette", 1, new Uint8Array(219));
  assert.equal(small.len, 219);
  assert.equal(small.initSize, 335);
  assert.equal(small.extends, 0);
  assert.equal(small.chunks, 1);
});

test("parseHeader reads the documented offsets", () => {
  const data = new Uint8Array(116);
  data.fill(0xab, 0, 8);
  data[8] = 1;
  data[9] = 3;
  data[10] = 1;
  data[11] = 255;
  data[12] = 0x04;
  data[13] = 0x03;
  data[14] = 0x02;
  data[15] = 0x01;
  data[16] = 0x06;
  data[17] = 0x05;
  data[18] = 0x7e;
  data[19] = 0x7e;
  data.fill(0x11, 20, 52);
  data.fill(0x22, 52, 84);
  data.fill(0x33, 84, 116);

  const header = parseHeader(data);
  assert.equal(header.version, 1);
  assert.equal(header.kind, 3);
  assert.equal(header.finalized, 1);
  assert.equal(header.bump, 255);
  assert.equal(header.len, 0x01020304);
  assert.equal(header.id, 0x0506);
  assert.equal(header.authority.length, 32);
  assert.ok(header.authority.every((byte) => byte === 0x11));
  assert.equal(header.expectedSha256.length, 32);
  assert.ok(header.expectedSha256.every((byte) => byte === 0x22));
  assert.equal(header.sha256.length, 32);
  assert.ok(header.sha256.every((byte) => byte === 0x33));
});

test("parseHeader rejects a short account", () => {
  assert.throws(() => parseHeader(new Uint8Array(115)), /header needs 116/);
});

/** A sprite book account header (kind 3) for `blobFits`. */
function bookAccount(id: number, finalized: boolean, len: number, expected: Uint8Array, hash: Uint8Array): Uint8Array {
  const data = new Uint8Array(116);
  data[8] = 1;
  data[9] = 3;
  data[10] = finalized ? 1 : 0;
  new DataView(data.buffer).setUint32(12, len, true);
  new DataView(data.buffer).setUint16(16, id, true);
  data.set(expected, 52);
  data.set(hash, 84);
  return data;
}

test("blobFits: a missing account, the same body finalized or half written", () => {
  const body = new Uint8Array([1, 2, 3, 4]);
  const hash = new Uint8Array(createHash("sha256").update(body).digest());
  assert.equal(blobFits("sprite_book", 2, body, null), true);
  assert.equal(blobFits("sprite_book", 2, body, bookAccount(2, true, 4, hash, hash)), true);
  assert.equal(blobFits("sprite_book", 2, body, bookAccount(2, false, 4, hash, new Uint8Array(32))), true);
});

test("blobFits: a frozen book with other sprites, another id or another unfinished body needs a new id", () => {
  const body = new Uint8Array([1, 2, 3, 4]);
  const hash = new Uint8Array(createHash("sha256").update(body).digest());
  const other = new Uint8Array(32).fill(7);
  assert.equal(blobFits("sprite_book", 2, body, bookAccount(2, true, 4, other, other)), false);
  assert.equal(blobFits("sprite_book", 2, body, bookAccount(3, true, 4, hash, hash)), false);
  assert.equal(blobFits("sprite_book", 2, body, bookAccount(2, false, 4, other, new Uint8Array(32))), false);
  assert.equal(blobFits("sprite_book", 2, body, bookAccount(2, false, 5, hash, new Uint8Array(32))), false);
});

test("finalize_rasterizer carries the GameConfig CPI accounts, the others do not", async () => {
  const signer = createNoopSigner("11111111111111111111111111111112" as never);
  const target = "11111111111111111111111111111113";
  const r = await finalizeInstruction("rasterizer", signer, target);
  assert.deepEqual(
    r.accounts.map((a: any) => [a.address, a.role]),
    [
      [signer.address, 2],
      [target, 1],
      [GAME_CONFIG, 1],
      [await renderAuthorityPda(), 0],
      [REGISTRY_PROGRAM, 0],
    ],
  );
  for (const kind of ["palette", "sprite_book"] as const) {
    assert.equal((await finalizeInstruction(kind, signer, target)).accounts.length, 2, kind);
  }
});
