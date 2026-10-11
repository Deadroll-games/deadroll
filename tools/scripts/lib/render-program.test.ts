import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  DISC,
  planUpload,
  parseHeader,
  writeChunkData,
  setComputeUnitLimitData,
  FINALIZE_CU,
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

test("discriminators match the render program", () => {
  for (const [name, hex] of Object.entries(EXPECTED)) {
    const bytes = DISC[name as keyof typeof DISC];
    assert.equal(bytes.length, 8, name);
    assert.equal(Buffer.from(bytes).toString("hex"), hex, name);
  }
});

test("write_chunk data for offset 900 and three bytes", () => {
  const data = writeChunkData(900, new Uint8Array([1, 2, 3]));
  assert.equal(Buffer.from(data).toString("hex"), "5d8da70fd18589338403000003000000010203");
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
