// Client for the on-chain render blobs: rasterizer WASM, palette, sprite book.
// Instructions, the header layout and the PDAs come from the sdk (generated from the render IDL).
import { createHash } from "node:crypto";
import {
  address,
  appendTransactionMessageInstructions,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  createTransactionMessage,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  type Instruction,
  type TransactionSigner,
} from "@solana/kit";
import { COMPUTE_BUDGET_PROGRAM, PROGRAMS } from "@deadroll/sdk";
import { findConfigPda } from "@deadroll/sdk/registry";
import {
  findPalettePda,
  findRasterizerPda,
  findRenderAuthorityPda,
  findSpriteBookPda,
  getBlobHeaderDecoder,
  getExtendInstruction,
  getFinalizePaletteInstruction,
  getFinalizeRasterizerInstruction,
  getFinalizeSpriteBookInstruction,
  getInitPaletteInstruction,
  getInitRasterizerInstruction,
  getInitSpriteBookInstruction,
  getWriteChunkInstruction,
} from "@deadroll/sdk/render";

export const RENDER_PROGRAM = PROGRAMS.render;
export const REGISTRY_PROGRAM = PROGRAMS.registry;
export const GAME_CONFIG = (await findConfigPda())[0];

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

function requireKind(kind: Kind): void {
  if (KIND_BYTE[kind] === undefined) throw new Error(`unknown blob kind ${String(kind)}`);
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

/** Reads the BlobHeader after the 8-byte discriminator (the sdk's decoder). */
export function parseHeader(data: Uint8Array): ParsedHeader {
  if (data.length < BODY_OFFSET) {
    throw new Error(`account data is ${data.length} bytes, header needs ${BODY_OFFSET}`);
  }
  const h = getBlobHeaderDecoder().decode(data, 8);
  return {
    version: h.version,
    kind: h.kind,
    finalized: h.finalized,
    bump: h.bump,
    len: h.len,
    id: h.id,
    authority: data.slice(20, 52),
    expectedSha256: Uint8Array.from(h.expectedSha256),
    sha256: Uint8Array.from(h.sha256),
  };
}

export async function pda(kind: Kind, id: number): Promise<string> {
  requireKind(kind);
  if (kind === "rasterizer") return (await findRasterizerPda({ version: id }))[0];
  if (kind === "palette") return (await findPalettePda({ version: id }))[0];
  return (await findSpriteBookPda({ bookId: id }))[0];
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

type Signer = TransactionSigner;

/** ComputeBudget SetComputeUnitLimit. Opcode 2, then units as u32 little-endian. No accounts. */
export function setComputeUnitLimitData(units: number): Uint8Array {
  const out = new Uint8Array(5);
  out[0] = 2;
  out.set(u32(units), 1);
  return out;
}

export function initInstruction(kind: Kind, authority: Signer, config: string, target: string, id: number, len: number, hash: Uint8Array): Instruction {
  requireKind(kind);
  if (hash.length !== 32) throw new Error("sha256 must be 32 bytes");
  const accounts = { authority, config: address(config) };
  if (kind === "rasterizer") return getInitRasterizerInstruction({ ...accounts, rasterizer: address(target), version: id, len, sha256: hash });
  if (kind === "palette") return getInitPaletteInstruction({ ...accounts, palette: address(target), version: id, len, sha256: hash });
  return getInitSpriteBookInstruction({ ...accounts, spriteBook: address(target), bookId: id, len, sha256: hash });
}

export function extendInstruction(authority: Signer, target: string): Instruction {
  return getExtendInstruction({ authority, target: address(target) });
}

export function writeChunkInstruction(authority: Signer, target: string, offset: number, bytes: Uint8Array): Instruction {
  return getWriteChunkInstruction({ authority, target: address(target), offset, bytes });
}

/** The render `["authority"]` PDA: signs `finalize_rasterizer`'s CPI into the registry. */
export async function renderAuthorityPda(): Promise<string> {
  return (await findRenderAuthorityPda())[0];
}

/**
 * `finalize_*`. The rasterizer one also writes `GameConfig` (CPI into the registry), so it takes
 * the config, the render authority PDA and the registry program (programs/render/README.md).
 */
export async function finalizeInstruction(kind: Kind, authority: Signer, target: string, config: string = GAME_CONFIG): Promise<Instruction> {
  requireKind(kind);
  if (kind === "rasterizer") {
    return getFinalizeRasterizerInstruction({
      authority,
      rasterizer: address(target),
      config: address(config),
      renderAuthority: address(await renderAuthorityPda()),
      registryProgram: REGISTRY_PROGRAM,
    });
  }
  if (kind === "palette") return getFinalizePaletteInstruction({ authority, palette: address(target) });
  return getFinalizeSpriteBookInstruction({ authority, spriteBook: address(target) });
}

export function setComputeUnitLimitInstruction(units: number): Instruction {
  return { programAddress: COMPUTE_BUDGET_PROGRAM, accounts: [], data: setComputeUnitLimitData(units) };
}

/** Subscription URL: a local validator uses ws: on the port plus one; a remote RPC uses wss: on the same host and port. */
export function wsUrl(rpcUrl: string): string {
  // Some providers (Alchemy) lack signatureSubscribe; DEADROLL_WS_URL points subscriptions elsewhere.
  if (process.env.DEADROLL_WS_URL) return process.env.DEADROLL_WS_URL;
  const url = new URL(rpcUrl);
  if (url.hostname === "127.0.0.1" || url.hostname === "localhost") {
    url.protocol = "ws:";
    url.port = String(Number(url.port || 8899) + 1);
  } else {
    url.protocol = url.protocol === "http:" ? "ws:" : "wss:";
  }
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
 * Whether `body` can go to the blob account holding `data` (null: no account yet) under the same
 * rules as `uploadBlob`: a missing account, the same body finalized, or an unfinished upload of
 * the same body. A finalized account with another body needs a new id.
 */
export function blobFits(kind: Kind, id: number, body: Uint8Array, data: Uint8Array | null): boolean {
  requireKind(kind);
  if (!data) return true;
  const header = parseHeader(data);
  if (header.kind !== KIND_BYTE[kind] || header.id !== id) return false;
  const hash = sha256(body);
  if (header.finalized !== 0) return sameBytes(header.sha256, hash);
  return header.len === body.length && sameBytes(header.expectedSha256, hash);
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
    await finalizeInstruction(kind, authority, target, config),
  ]);
  console.log(`${target} ${toHex(hash)}`);
}
