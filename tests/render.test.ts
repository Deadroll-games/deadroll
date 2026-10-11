// programs/render on a local validator: upload the real rasterizer WASM, palette and sprite book,
// read them back, render with the on-chain WASM (demo step 1), and every rejection.
//
// Before `anchor test --validator legacy --provider.wallet .anchor/test-wallet.json`:
//   node tests/fixtures/make-config.mjs           (GameConfig fixture + test wallet)
//   bash crates/render/wasm/build.sh               (target/render-wasm/deadroll_render.wasm)
//   cargo test -p deadroll-render --test golden    (target/render-golden: palette, book, expected hash)
// Instructions are encoded by hand until the Codama sdk exists (L8.1).
import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import {
  AccountRole,
  address,
  airdropFactory,
  appendTransactionMessageInstructions,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  createTransactionMessage,
  generateKeyPairSigner,
  getAddressEncoder,
  getProgramDerivedAddress,
  lamports,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
} from "@solana/kit";

const root = new URL("../", import.meta.url);
const read = (p: string) => readFileSync(new URL(p, root));

const PROGRAM = address("DgQCsXAe7Q7h8zdtToiTvk8XqNjjbXuD6xith2BmEiRB");
const SYSTEM = address("11111111111111111111111111111111");
const CONFIG = address("HYGYnuyssMfrHr2gT3AC3hkQpD6xt37a5hrCc2Z97JHN");
const BODY_OFFSET = 116;
const GROW_STEP = 10 * 1024;
const CHUNK = 900;

// RenderError codes (Anchor offset 6000, declaration order in programs/render/src/errors.rs).
const E = {
  Unauthorized: 6000,
  InvalidLength: 6001,
  AlreadyFinalized: 6002,
  OutOfBounds: 6003,
  NotFullyAllocated: 6004,
  AlreadyFullyAllocated: 6005,
  HashMismatch: 6006,
  InvalidBody: 6007,
  WrongAccount: 6008,
  WrongConfig: 6009,
};

const url = process.env.ANCHOR_PROVIDER_URL ?? "http://127.0.0.1:8899";
const rpc = createSolanaRpc(url);
const wsUrl = new URL(url);
wsUrl.protocol = "ws:";
wsUrl.port = String(Number(wsUrl.port || 8899) + 1);
const rpcSubscriptions = createSolanaRpcSubscriptions(wsUrl.toString());
const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
const airdrop = airdropFactory({ rpc, rpcSubscriptions });

const walletPath = process.env.ANCHOR_WALLET ?? new URL(".anchor/test-wallet.json", root).pathname.slice(1);
const authority = await createKeyPairSignerFromBytes(new Uint8Array(JSON.parse(readFileSync(walletPath, "utf8"))));
await airdrop({ recipientAddress: authority.address, lamports: lamports(50_000_000_000n), commitment: "confirmed" });

const sha256 = (b: Uint8Array) => new Uint8Array(createHash("sha256").update(b).digest());
const disc = (name: string) => sha256(new TextEncoder().encode(`global:${name}`)).subarray(0, 8);
const u16 = (n: number) => new Uint8Array(new Uint16Array([n]).buffer);
const u32 = (n: number) => new Uint8Array(new Uint32Array([n]).buffer);
const cat = (...parts: Uint8Array[]) => {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
};

type Signer = Awaited<ReturnType<typeof generateKeyPairSigner>>;
type Kind = "rasterizer" | "palette" | "sprite_book";
const SEED: Record<Kind, string> = { rasterizer: "rasterizer", palette: "palette", sprite_book: "sprites" };

async function pda(kind: Kind, id: number) {
  const [a] = await getProgramDerivedAddress({
    programAddress: PROGRAM,
    seeds: [new TextEncoder().encode(SEED[kind]), u16(id)],
  });
  return a;
}

const signerMeta = (s: Signer, writable: boolean) => ({
  address: s.address,
  role: writable ? AccountRole.WRITABLE_SIGNER : AccountRole.READONLY_SIGNER,
  signer: s,
});
const writable = (a: string) => ({ address: address(a), role: AccountRole.WRITABLE });
const readonly = (a: string) => ({ address: address(a), role: AccountRole.READONLY });

const ix = {
  init: async (kind: Kind, s: Signer, id: number, len: number, hash: Uint8Array, config = CONFIG) => ({
    programAddress: PROGRAM,
    accounts: [signerMeta(s, true), readonly(config), writable(await pda(kind, id)), readonly(SYSTEM)],
    data: cat(disc(`init_${kind}`), u16(id), u32(len), hash),
  }),
  extend: (s: Signer, target: string) => ({
    programAddress: PROGRAM,
    accounts: [signerMeta(s, true), writable(target), readonly(SYSTEM)],
    data: disc("extend"),
  }),
  write: (s: Signer, target: string, offset: number, bytes: Uint8Array) => ({
    programAddress: PROGRAM,
    accounts: [signerMeta(s, false), writable(target)],
    data: cat(disc("write_chunk"), u32(offset), u32(bytes.length), bytes),
  }),
  finalize: (kind: Kind, s: Signer, target: string) => ({
    programAddress: PROGRAM,
    accounts: [signerMeta(s, false), writable(target)],
    data: disc(`finalize_${kind}`),
  }),
};

async function send(payer: Signer, ...instructions: any[]) {
  const { value: blockhash } = await rpc.getLatestBlockhash().send();
  const msg = pipe(
    createTransactionMessage({ version: 0 }),
    (m) => setTransactionMessageFeePayerSigner(payer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(blockhash, m),
    (m) => appendTransactionMessageInstructions(instructions, m),
  );
  const tx = await signTransactionMessageWithSigners(msg);
  try {
    await sendAndConfirm(tx as any, { commitment: "confirmed" });
  } catch (e: any) {
    // RENDER_TEST_LOGS=1 prints the simulation logs of every failed transaction.
    if (process.env.RENDER_TEST_LOGS) console.error((e?.context?.logs ?? e?.cause?.context?.logs ?? [e?.message]).join("\n"));
    throw e;
  }
}

/** The custom program error code of a failed send. */
async function fails(p: Promise<unknown>): Promise<number | undefined> {
  try {
    await p;
  } catch (e) {
    const seen = new Set();
    const text = JSON.stringify(e, (k, v) => {
      if (typeof v === "bigint") return Number(v);
      if (typeof v === "object" && v !== null) {
        if (seen.has(v)) return undefined;
        seen.add(v);
        if (v instanceof Error) return { ...v, message: v.message, cause: (v as any).cause, context: (v as any).context };
      }
      return v;
    });
    const m = text.match(/"Custom":(\d+)/) ?? text.match(/custom program error: 0x([0-9a-f]+)/);
    if (!m) return -1;
    return m[0].includes("0x") ? parseInt(m[1], 16) : Number(m[1]);
  }
  assert.fail("transaction should have failed");
}

async function data(a: string): Promise<Uint8Array> {
  const { value } = await rpc.getAccountInfo(address(a), { encoding: "base64", commitment: "confirmed" }).send();
  assert.ok(value, `account ${a} exists`);
  return new Uint8Array(Buffer.from(value.data[0], "base64"));
}

async function init(kind: Kind, id: number, body: Uint8Array) {
  await send(authority, await ix.init(kind, authority, id, body.length, sha256(body)));
  return pda(kind, id);
}

async function grow(target: string, len: number) {
  // Exactly the missing 10 KB steps, up to 4 extends per transaction (one more would fail the tx).
  let missing = Math.ceil((BODY_OFFSET + len - (await data(target)).length) / GROW_STEP);
  while (missing > 0) {
    const n = Math.min(missing, 4);
    await send(authority, ...Array.from({ length: n }, () => ix.extend(authority, target)));
    missing -= n;
  }
}

async function writeAll(target: string, body: Uint8Array) {
  const sends = [];
  for (let o = 0; o < body.length; o += CHUNK) sends.push(send(authority, ix.write(authority, target, o, body.subarray(o, o + CHUNK))));
  await Promise.all(sends);
}

async function upload(kind: Kind, id: number, body: Uint8Array) {
  const target = await init(kind, id, body);
  await grow(target, body.length);
  await writeAll(target, body);
  await send(authority, ix.finalize(kind, authority, target));
  return target;
}

/** Splits target/render-golden/input.bin (FORMAT.md 7) into its parts. */
function goldenInput() {
  const b = read("target/render-golden/input.bin");
  let o = 0;
  const take = () => {
    const n = b.readUInt32LE(o);
    const part = new Uint8Array(b.subarray(o + 4, o + 4 + n));
    o += 4 + n;
    return part;
  };
  const request = take();
  o += 32;
  const palette = take();
  const book = take();
  return { request, palette, book, expected: read("target/render-golden/expected.sha256").toString().trim() };
}

function renderWithWasm(wasm: Uint8Array, input: Uint8Array) {
  const { exports: x } = new WebAssembly.Instance(new WebAssembly.Module(wasm), {}) as any;
  const ptr = x.alloc(input.length);
  new Uint8Array(x.memory.buffer, ptr, input.length).set(input);
  const out = x.render(ptr, input.length, 1, 8);
  assert.notEqual(out, 0, `render error ${x.last_error()}`);
  return new Uint8Array(x.memory.buffer, out, x.result_len()).slice();
}

const wasm = new Uint8Array(read("target/render-wasm/deadroll_render.wasm"));
const golden = goldenInput();
const header = (d: Uint8Array) => ({ version: d[8], kind: d[9], finalized: d[10], len: new DataView(d.buffer).getUint32(12, true) });

test("demo step 1: rasterizer, palette and sprite book from chain render the golden PNG", async () => {
  const r = await upload("rasterizer", 1, wasm);
  const p = await upload("palette", 1, golden.palette);
  const b = await upload("sprite_book", 1, golden.book);

  const rd = await data(r);
  assert.deepEqual(header(rd), { version: 1, kind: 1, finalized: 1, len: wasm.length });
  assert.deepEqual(rd.subarray(84, 116), sha256(wasm), "sha256 stored at the documented offset");
  const chainWasm = rd.subarray(BODY_OFFSET);
  assert.deepEqual(chainWasm, wasm);

  const pal = (await data(p)).subarray(BODY_OFFSET);
  const book = (await data(b)).subarray(BODY_OFFSET);
  const input = cat(u32(golden.request.length), golden.request, new Uint8Array(32), u32(pal.length), pal, u32(book.length), book);
  const png = renderWithWasm(chainWasm, input);
  assert.equal(Buffer.from(sha256(png)).toString("hex"), golden.expected);
});

test("init rejections", async () => {
  const stranger = await generateKeyPairSigner();
  await airdrop({ recipientAddress: stranger.address, lamports: lamports(2_000_000_000n), commitment: "confirmed" });
  const h = sha256(wasm);
  assert.equal(await fails(send(stranger, await ix.init("rasterizer", stranger, 20, 100, h))), E.Unauthorized);
  // A program-owned account that is not the registry config.
  assert.equal(await fails(send(authority, await ix.init("rasterizer", authority, 20, 100, h, await pda("rasterizer", 1)))), E.WrongConfig);
  assert.equal(await fails(send(authority, await ix.init("rasterizer", authority, 20, 0, h))), E.InvalidLength);
  assert.equal(await fails(send(authority, await ix.init("rasterizer", authority, 20, 128 * 1024 + 1, h))), E.InvalidLength);
  assert.equal(await fails(send(authority, await ix.init("palette", authority, 20, 8 * 1024 + 1, h))), E.InvalidLength);
  assert.equal(await fails(send(authority, await ix.init("sprite_book", authority, 20, 64 * 1024 + 1, h))), E.InvalidLength);
  // Same version twice: the PDA already exists (system program error, not ours).
  await send(authority, await ix.init("rasterizer", authority, 21, 100, h));
  assert.notEqual(await fails(send(authority, await ix.init("rasterizer", authority, 21, 200, h))), undefined);
});

test("extend and write rejections", async () => {
  const stranger = await generateKeyPairSigner();
  await airdrop({ recipientAddress: stranger.address, lamports: lamports(2_000_000_000n), commitment: "confirmed" });
  const t = await init("rasterizer", 30, wasm);
  assert.equal((await data(t)).length, GROW_STEP, "init allocates one step");
  assert.equal(await fails(send(stranger, ix.extend(stranger, t))), E.Unauthorized);
  assert.equal(await fails(send(stranger, ix.write(stranger, t, 0, new Uint8Array([1])))), E.Unauthorized);
  assert.equal(await fails(send(authority, ix.write(authority, t, GROW_STEP - BODY_OFFSET, new Uint8Array([1])))), E.OutOfBounds);
  assert.equal(await fails(send(authority, ix.write(authority, t, 0, new Uint8Array()))), E.InvalidLength);
  await grow(t, wasm.length);
  assert.equal(await fails(send(authority, ix.extend(authority, t))), E.AlreadyFullyAllocated);
  assert.equal(await fails(send(authority, ix.write(authority, t, wasm.length - 1, new Uint8Array([1, 2])))), E.OutOfBounds);
  assert.equal(await fails(send(authority, ix.write(authority, t, 0xffffffff, new Uint8Array([1])))), E.OutOfBounds);
  // Not a blob of this program.
  assert.equal(await fails(send(authority, ix.write(authority, CONFIG, 0, new Uint8Array([1])))), E.WrongAccount);
  // After finalize nothing writes or grows.
  await writeAll(t, wasm);
  await send(authority, ix.finalize("rasterizer", authority, t));
  assert.equal(await fails(send(authority, ix.write(authority, t, 0, new Uint8Array([1])))), E.AlreadyFinalized);
  assert.equal(await fails(send(authority, ix.extend(authority, t))), E.AlreadyFinalized);
});

test("finalize rejections", async () => {
  const stranger = await generateKeyPairSigner();
  await airdrop({ recipientAddress: stranger.address, lamports: lamports(2_000_000_000n), commitment: "confirmed" });
  const r = await init("rasterizer", 40, wasm);
  assert.equal(await fails(send(authority, ix.finalize("rasterizer", authority, r))), E.NotFullyAllocated);
  await grow(r, wasm.length);
  assert.equal(await fails(send(authority, ix.finalize("rasterizer", authority, r))), E.HashMismatch);
  await writeAll(r, wasm);
  assert.equal(await fails(send(stranger, ix.finalize("rasterizer", stranger, r))), E.Unauthorized);
  // Wrong account type for the instruction: Anchor's discriminator check (not a RenderError).
  const wrongType = await fails(send(authority, ix.finalize("palette", authority, r)));
  assert.ok(wrongType !== undefined && wrongType < 6000, `anchor error, got ${wrongType}`);
  await send(authority, ix.finalize("rasterizer", authority, r));
  assert.equal(await fails(send(authority, ix.finalize("rasterizer", authority, r))), E.AlreadyFinalized);

  // Hash matches, content is wrong for the kind.
  const notWasm = new Uint8Array(64).fill(9);
  const t = await init("rasterizer", 41, notWasm);
  await writeAll(t, notWasm);
  assert.equal(await fails(send(authority, ix.finalize("rasterizer", authority, t))), E.InvalidBody);
  // Palette body of version 1 stored as version 2.
  const p = await init("palette", 2, golden.palette);
  await writeAll(p, golden.palette);
  assert.equal(await fails(send(authority, ix.finalize("palette", authority, p))), E.InvalidBody);
  // Sprite book with id 1 stored as book 5.
  const b = await init("sprite_book", 5, golden.book);
  await writeAll(b, golden.book);
  assert.equal(await fails(send(authority, ix.finalize("sprite_book", authority, b))), E.InvalidBody);
});
