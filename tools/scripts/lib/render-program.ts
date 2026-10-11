// Client for the on-chain render blobs: rasterizer WASM, palette, sprite book.
// Account layout and instruction bytes match programs/render (see its README).
import { createHash } from "node:crypto";
import {
  AccountRole,
  address,
  appendTransactionMessageInstructions,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  createTransactionMessage,
  getProgramDerivedAddress,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
} from "@solana/kit";

export const RENDER_PROGRAM = address("DgQCsXAe7Q7h8zdtToiTvk8XqNjjbXuD6xith2BmEiRB");
export const GAME_CONFIG = address("HYGYnuyssMfrHr2gT3AC3hkQpD6xt37a5hrCc2Z97JHN");
const SYSTEM_PROGRAM = address("11111111111111111111111111111111");
const COMPUTE_BUDGET_PROGRAM = address("ComputeBudget111111111111111111111111111111");

/** Bytes before the body: discriminator + BlobHeader. */
export const BODY_OFFSET = 116;
/** init allocates at most this many bytes; each extend grows by at most this many. */
export const GROW_STEP = 10 * 1024;
/** Body bytes in one write_chunk transaction. */
export const CHUNK = 900;
/** Solana rejects a transaction with more than this many account grows. */
export const MAX_EXTENDS_PER_TX = 4;
export const FINALIZE_CU = 400_000;
export const WRITE_BATCH = 8;
export const WRITE_ATTEMPTS = 3;

export const WASM_MAGIC = new Uint8Array([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

export type Kind = "rasterizer" | "palette" | "sprite_book";

const SEED: Record<Kind, string> = {
  rasterizer: "rasterizer",
  palette: "palette",
  sprite_book: "sprites",
};
const KIND_BYTE: Record<Kind, number> = {
  rasterizer: 1,
  palette: 2,
  sprite_book: 3,
};
const MAX_LEN: Record<Kind, number> = {
  rasterizer: 128 * 1024,
  palette: 8 * 1024,
  sprite_book: 64 * 1024,
};

function fromHex(hex: string): Uint8Array {
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  return out;
}

/** First 8 bytes of sha256("global:<name>"), pinned so a rename cannot drift quietly. */
export const DISC = {
  init_rasterizer: fromHex("82bff24f597b42d2"),
  init_palette: fromHex("7ed0d30cb6daa922"),
  init_sprite_book: fromHex("2f2e5ac9c2467b79"),
  extend: fromHex("e47f0001e39a36a8"),
  write_chunk: fromHex("5d8da70fd1858933"),
  finalize_rasterizer: fromHex("2680cb91554cc31e"),
  finalize_palette: fromHex("2201740fd300a563"),
  finalize_sprite_book: fromHex("80761daaae3cf1fe"),
};

const INIT_DISC: Record<Kind, Uint8Array> = {
  rasterizer: DISC.init_rasterizer,
  palette: DISC.init_palette,
  sprite_book: DISC.init_sprite_book,
};
const FINALIZE_DISC: Record<Kind, Uint8Array> = {
  rasterizer: DISC.finalize_rasterizer,
  palette: DISC.finalize_palette,
  sprite_book: DISC.finalize_sprite_book,
};

function requireKind(kind: Kind): void {
  if (SEED[kind] === undefined) throw new Error(`unknown blob kind ${String(kind)}`);
}

export function u16(n: number): Uint8Array {
  const out = new Uint8Array(2);
  out[0] = n & 0xff;
  out[1] = (n >>> 8) & 0xff;
  return out;
}

export function u32(n: number): Uint8Array {
  const out = new Uint8Array(4);
  out[0] = n & 0xff;
  out[1] = (n >>> 8) & 0xff;
  out[2] = (n >>> 16) & 0xff;
  out[3] = (n >>> 24) & 0xff;
  return out;
}

function cat(...parts: Uint8Array[]): Uint8Array {
  let n = 0;
  for (const part of parts) n += part.length;
  const out = new Uint8Array(n);
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

export function toHex(bytes: Uint8Array): string {
  let out = "";
  for (const byte of bytes) out += byte.toString(16).padStart(2, "0");
  return out;
}

export function sha256(bytes: Uint8Array): Uint8Array {
  return new Uint8Array(createHash("sha256").update(bytes).digest());
}

function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

export function samePrefix(body: Uint8Array, prefix: Uint8Array): boolean {
  if (body.length < prefix.length) return false;
  for (let i = 0; i < prefix.length; i++) if (body[i] !== prefix[i]) return false;
  return true;
}

export type ParsedHeader = {
  version: number;
  kind: number;
  finalized: number;
  bump: number;
  len: number;
  id: number;
  authority: Uint8Array;
  expectedSha256: Uint8Array;
  sha256: Uint8Array;
};

/** Reads BlobHeader at the documented offsets. Ignores the 8-byte discriminator. */
export function parseHeader(data: Uint8Array): ParsedHeader {
  if (data.length < BODY_OFFSET) {
    throw new Error(`account data is ${data.length} bytes, header needs ${BODY_OFFSET}`);
  }
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  return {
    version: data[8],
    kind: data[9],
    finalized: data[10],
    bump: data[11],
    len: view.getUint32(12, true),
    id: view.getUint16(16, true),
    authority: data.slice(20, 52),
    expectedSha256: data.slice(52, 84),
    sha256: data.slice(84, 116),
  };
}

export async function pda(kind: Kind, id: number) {
  requireKind(kind);
  const [derived] = await getProgramDerivedAddress({
    programAddress: RENDER_PROGRAM,
    seeds: [new TextEncoder().encode(SEED[kind]), u16(id)],
  });
  return derived;
}

export type UploadPlan = {
  kind: Kind;
  id: number;
  address: string;
  len: number;
  sha256: string;
  initSize: number;
  extends: number;
  chunks: number;
};

/** How many init / extend / write steps this body needs. No RPC. */
export async function planUpload(kind: Kind, id: number, body: Uint8Array): Promise<UploadPlan> {
  requireKind(kind);
  const len = body.length;
  const max = MAX_LEN[kind];
  if (len <= 0 || len > max) {
    throw new Error(`body length ${len} is outside 1..${max} for ${kind}`);
  }
  const full = BODY_OFFSET + len;
  const initSize = Math.min(full, GROW_STEP);
  const extendCount = Math.ceil((full - initSize) / GROW_STEP);
  const chunks = Math.ceil(len / CHUNK);
  return {
    kind,
    id,
    address: await pda(kind, id),
    len,
    sha256: toHex(sha256(body)),
    initSize,
    extends: extendCount,
    chunks,
  };
}

type Signer = { address: string };

const signerMeta = (signer: Signer, writable: boolean) => ({
  address: signer.address,
  role: writable ? AccountRole.WRITABLE_SIGNER : AccountRole.READONLY_SIGNER,
  signer,
});
const writable = (account: string) => ({ address: address(account), role: AccountRole.WRITABLE });
const readonly = (account: string) => ({ address: address(account), role: AccountRole.READONLY });

export function initData(kind: Kind, id: number, len: number, hash: Uint8Array): Uint8Array {
  requireKind(kind);
  if (hash.length !== 32) throw new Error("sha256 must be 32 bytes");
  return cat(INIT_DISC[kind], u16(id), u32(len), hash);
}

export function writeChunkData(offset: number, bytes: Uint8Array): Uint8Array {
  return cat(DISC.write_chunk, u32(offset), u32(bytes.length), bytes);
}

/** ComputeBudget SetComputeUnitLimit. Opcode 2, then units as u32 little-endian. No accounts. */
export function setComputeUnitLimitData(units: number): Uint8Array {
  return cat(new Uint8Array([2]), u32(units));
}

export function initInstruction(
  kind: Kind,
  authority: Signer,
  config: string,
  target: string,
  id: number,
  len: number,
  hash: Uint8Array,
) {
  return {
    programAddress: RENDER_PROGRAM,
    accounts: [signerMeta(authority, true), readonly(config), writable(target), readonly(SYSTEM_PROGRAM)],
    data: initData(kind, id, len, hash),
  };
}

export function extendInstruction(authority: Signer, target: string) {
  return {
    programAddress: RENDER_PROGRAM,
    accounts: [signerMeta(authority, true), writable(target), readonly(SYSTEM_PROGRAM)],
    data: DISC.extend,
  };
}

export function writeChunkInstruction(authority: Signer, target: string, offset: number, bytes: Uint8Array) {
  return {
    programAddress: RENDER_PROGRAM,
    accounts: [signerMeta(authority, false), writable(target)],
    data: writeChunkData(offset, bytes),
  };
}

export function finalizeInstruction(kind: Kind, authority: Signer, target: string) {
  requireKind(kind);
  return {
    programAddress: RENDER_PROGRAM,
    accounts: [signerMeta(authority, false), writable(target)],
    data: FINALIZE_DISC[kind],
  };
}

export function setComputeUnitLimitInstruction(units: number) {
  return {
    programAddress: COMPUTE_BUDGET_PROGRAM,
    accounts: [],
    data: setComputeUnitLimitData(units),
  };
}

/** RPC URL with the ws: protocol and the port plus one (local validator subscriptions). */
export function wsUrl(rpcUrl: string): string {
  const url = new URL(rpcUrl);
  url.protocol = "ws:";
  url.port = String(Number(url.port || 8899) + 1);
  return url.toString();
}

export function connect(rpcUrl: string) {
  const rpc = createSolanaRpc(rpcUrl);
  const rpcSubscriptions = createSolanaRpcSubscriptions(wsUrl(rpcUrl));
  return { rpc, rpcSubscriptions };
}

async function fetchAccount(rpc: any, target: string): Promise<Uint8Array | null> {
  const { value } = await rpc
    .getAccountInfo(address(target), { encoding: "base64", commitment: "confirmed" })
    .send();
  if (!value) return null;
  const raw = value.data;
  if (typeof raw === "string") return new Uint8Array(Buffer.from(raw, "base64"));
  if (Array.isArray(raw) && typeof raw[0] === "string") return new Uint8Array(Buffer.from(raw[0], "base64"));
  if (raw instanceof Uint8Array) return new Uint8Array(raw);
  throw new Error(`account ${target} returned no base64 data`);
}

async function send(rpc: any, rpcSubscriptions: any, payer: Signer, instructions: any[]): Promise<void> {
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
  const { value: blockhash } = await rpc.getLatestBlockhash().send();
  const message = pipe(
    createTransactionMessage({ version: 0 }),
    (current) => setTransactionMessageFeePayerSigner(payer, current),
    (current) => setTransactionMessageLifetimeUsingBlockhash(blockhash, current),
    (current) => appendTransactionMessageInstructions(instructions, current),
  );
  const tx = await signTransactionMessageWithSigners(message);
  await sendAndConfirm(tx as any, { commitment: "confirmed" });
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function retry(attempts: number, fn: () => Promise<void>): Promise<void> {
  let last: unknown;
  for (let attempt = 1; attempt <= attempts; attempt++) {
    try {
      await fn();
      return;
    } catch (err) {
      last = err;
      if (attempt < attempts) await sleep(200 * attempt);
    }
  }
  throw last;
}

function chunkOffsets(len: number): number[] {
  const offsets: number[] = [];
  for (let offset = 0; offset < len; offset += CHUNK) offsets.push(offset);
  return offsets;
}

function differingOffsets(data: Uint8Array, body: Uint8Array): number[] {
  const offsets: number[] = [];
  for (const offset of chunkOffsets(body.length)) {
    const end = Math.min(offset + CHUNK, body.length);
    const onChain = data.subarray(BODY_OFFSET + offset, BODY_OFFSET + end);
    const file = body.subarray(offset, end);
    if (!sameBytes(onChain, file)) offsets.push(offset);
  }
  return offsets;
}

/**
 * Create, grow, write and finalize one blob. Safe to run again: a matching finalized
 * account returns, a different body throws, and chunks already on-chain are skipped.
 */
export async function uploadBlob(args: {
  rpc: any;
  rpcSubscriptions: any;
  authority: Signer;
  kind: Kind;
  id: number;
  body: Uint8Array;
  config: string;
}): Promise<void> {
  const { rpc, rpcSubscriptions, authority, kind, id, body, config } = args;
  requireKind(kind);
  const hash = sha256(body);
  const target = await pda(kind, id);
  const full = BODY_OFFSET + body.length;
  if (body.length <= 0 || body.length > MAX_LEN[kind]) {
    throw new Error(`body length ${body.length} is outside 1..${MAX_LEN[kind]} for ${kind}`);
  }

  let data = await fetchAccount(rpc, target);
  if (!data) {
    await send(rpc, rpcSubscriptions, authority, [
      initInstruction(kind, authority, config, target, id, body.length, hash),
    ]);
    data = await fetchAccount(rpc, target);
    if (!data) throw new Error(`init did not create ${target}`);
  } else {
    const header = parseHeader(data);
    if (header.kind !== KIND_BYTE[kind] || header.id !== id) {
      throw new Error(
        `account ${target} is kind ${header.kind} id ${header.id}; a new version id is needed`,
      );
    }
    if (header.finalized !== 0 && sameBytes(header.sha256, hash)) {
      console.log("already finalized");
      return;
    }
    if (header.finalized !== 0) {
      throw new Error(
        `account ${target} is finalized with another hash ${toHex(header.sha256)}; a new version id is needed`,
      );
    }
    if (header.len !== body.length || !sameBytes(header.expectedSha256, hash)) {
      throw new Error(
        `account ${target} has len ${header.len} and a different expected hash; a new version id is needed`,
      );
    }
  }

  let guard = 0;
  while (data.length < full) {
    if (guard++ > 64) throw new Error(`extend did not reach ${full} bytes for ${target}`);
    const missing = Math.ceil((full - data.length) / GROW_STEP);
    const count = Math.min(missing, MAX_EXTENDS_PER_TX);
    const instructions = [];
    for (let i = 0; i < count; i++) instructions.push(extendInstruction(authority, target));
    await send(rpc, rpcSubscriptions, authority, instructions);
    const next = await fetchAccount(rpc, target);
    if (!next) throw new Error(`account ${target} missing after extend`);
    if (next.length <= data.length) throw new Error(`extend left ${target} at ${data.length} bytes`);
    data = next;
  }
  if (data.length !== full) {
    throw new Error(`account ${target} is ${data.length} bytes, expected ${full}`);
  }

  const writeOffsets = async (offsets: number[]) => {
    for (let i = 0; i < offsets.length; i += WRITE_BATCH) {
      const batch = offsets.slice(i, i + WRITE_BATCH);
      await Promise.all(
        batch.map((offset) =>
          retry(WRITE_ATTEMPTS, () =>
            send(rpc, rpcSubscriptions, authority, [
              writeChunkInstruction(
                authority,
                target,
                offset,
                body.subarray(offset, Math.min(offset + CHUNK, body.length)),
              ),
            ]),
          ),
        ),
      );
    }
  };

  await writeOffsets(differingOffsets(data, body));
  data = await fetchAccount(rpc, target);
  if (!data) throw new Error(`account ${target} missing after write`);
  let mismatch = differingOffsets(data, body);
  if (mismatch.length > 0) {
    await writeOffsets(mismatch);
    data = await fetchAccount(rpc, target);
    if (!data) throw new Error(`account ${target} missing after rewrite`);
    mismatch = differingOffsets(data, body);
    if (mismatch.length > 0) {
      throw new Error(
        `body still differs at ${mismatch.length} chunk(s) starting at offset ${mismatch[0]}`,
      );
    }
  }

  await send(rpc, rpcSubscriptions, authority, [
    setComputeUnitLimitInstruction(FINALIZE_CU),
    finalizeInstruction(kind, authority, target),
  ]);
  console.log(`${target} ${toHex(hash)}`);
}
