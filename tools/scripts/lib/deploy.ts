// The whole deploy pipeline (programs, GameConfig, registry tables, render blobs, demo map),
// shared by deploy-local.ts and deploy-devnet.ts. Every step is idempotent: a rerun skips what is
// already on chain and finishes what is not.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  AccountRole,
  address,
  appendTransactionMessageInstructions,
  createKeyPairSignerFromBytes,
  createTransactionMessage,
  getAddressDecoder,
  getAddressEncoder,
  getProgramDerivedAddress,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  type Address,
  type KeyPairSigner,
} from "@solana/kit";
import { GAME_CONFIG, connect, pda as blobPda, parseHeader } from "./render-program.ts";
import { MAP_STRIDE, layoutBytes, loadMapPreset, type MapPreset } from "./demo-map.ts";

export const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const SYSTEM_PROGRAM = address("11111111111111111111111111111111");
const BPF_UPGRADEABLE = "BPFLoaderUpgradeab1e11111111111111111111111";
/** UpgradeableLoaderState::ProgramData header before the program bytes. */
const PROGRAMDATA_HEADER = 45;
/** Below this a `.so` is a `no-entrypoint` build, not a program (the smallest real one is ~48 KB). */
const MIN_PROGRAM_SIZE = 16 * 1024;

/** `ProgramIdx` order (crates/types): the order `init_config` expects. */
export const PROGRAMS = [
  { key: "registry", name: "deadroll_registry" },
  { key: "render", name: "deadroll_render_program" },
  { key: "character", name: "deadroll_character" },
  { key: "items", name: "deadroll_items" },
  { key: "map", name: "deadroll_map" },
  { key: "session", name: "deadroll_session" },
  { key: "city", name: "deadroll_city" },
] as const;

/** `programs/session/src/vrf.rs`: only a mock-vrf build references the mock identity. */
const MOCK_VRF_IDENTITY = "5RotSMue7vx9xBPp3L8iQEKbKSc3ezbY4GqiJxDEe6sN";
const VRF_IDENTITY = "Bj7XBktwDmAg6J4FohS6oRHbevD6MMd8jnCZKvzKPiKV";

/** `GameConfig` byte offsets (discriminator included), programs/registry README. */
const CFG = { authority: 9, programs: 41, rasterizerVersion: 265, rasterizer: 267, sha: 299, palette: 331, uri: 363, spriteBooks: 428 };
/** `GameConfig` account size before patch 7 (discriminator + `GameConfigData`) and now (+ `sprite_books: [Pubkey; 4]`). */
const CONFIG_LEGACY_LEN = 8 + 420;
const CONFIG_LEN = CONFIG_LEGACY_LEN + 4 * 32;
const URI_BASE_LEN = 64;
const EMPTY_ADDRESS = "11111111111111111111111111111111";
/** Rasterizer version and sprite book id of sprite format 2 (crates/render/FORMAT.md); version 1 and book 1 hold format 1. */
export const RASTERIZER_VERSION = 2;
export const BOOK_ID = 2;
/** `Map` byte offsets: discriminator, version, dm, index u32, width, height, status. */
const MAP_STATUS = 8 + 1 + 32 + 4 + 1 + 1;
const MAP_STATUS_NAMES = ["Draft", "Finalized", "InSession", "Finished"];

export type Cluster = "local" | "devnet";
export type DeployOptions = {
  cluster: Cluster;
  url: string;
  wallet: string;
  mockVrf: boolean;
  skipBuild: boolean;
  uriBase: string;
  uriBaseExplicit: boolean;
  rasterizerVersion: number;
  bookId: number;
  mapIndex: number;
  computeUnitPrice?: number;
  /** Called with the cost table before anything is sent; return false to stop. */
  confirmCost?: (lines: string[], totalLamports: bigint, balance: bigint) => Promise<boolean>;
};

// ---------------------------------------------------------------------------------------------
// small helpers

const enc = new TextEncoder();
const sha256 = (bytes: Uint8Array) => new Uint8Array(createHash("sha256").update(bytes).digest());
const sol = (lamports: bigint | number) => (Number(lamports) / 1e9).toFixed(4);

function cat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}
const u8 = (n: number) => new Uint8Array([n & 0xff]);
const u16 = (n: number) => new Uint8Array([n & 0xff, (n >>> 8) & 0xff]);
const u32 = (n: number) => new Uint8Array([n & 0xff, (n >>> 8) & 0xff, (n >>> 16) & 0xff, (n >>> 24) & 0xff]);
const vec = (bytes: Uint8Array) => cat(u32(bytes.length), bytes);

function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

function contains(hay: Uint8Array, needle: Uint8Array): boolean {
  outer: for (let i = 0; i + needle.length <= hay.length; i++) {
    for (let j = 0; j < needle.length; j++) if (hay[i + j] !== needle[j]) continue outer;
    return true;
  }
  return false;
}

const addrBytes = (a: string) => new Uint8Array(getAddressEncoder().encode(address(a)));
const addrAt = (data: Uint8Array, offset: number) => getAddressDecoder().decode(data.subarray(offset, offset + 32));

function step(title: string) {
  console.log(`\n== ${title}`);
}

// ---------------------------------------------------------------------------------------------
// CLIs

function firstExisting(paths: string[]): string | undefined {
  return paths.find((p) => existsSync(p));
}

function onPath(exe: string): string | undefined {
  const pathKey = Object.keys(process.env).find((k) => k.toUpperCase() === "PATH") ?? "PATH";
  const dirs = (process.env[pathKey] ?? "").split(delimiter);
  const names = process.platform === "win32" ? [`${exe}.exe`, `${exe}.cmd`] : [exe];
  for (const d of dirs) for (const n of names) if (d && existsSync(join(d, n))) return join(d, n);
  return undefined;
}

/** `SOLANA_BIN` / `ANCHOR_BIN` / `BASH_BIN`, then PATH, then the usual install folders. */
export function resolveBins() {
  const home = homedir();
  const exe = process.platform === "win32" ? ".exe" : "";
  const releases = join(home, ".local/share/solana/install/releases");
  const releaseBins = existsSync(releases)
    ? readdirSync(releases).map((r) => join(releases, r, "solana-release/bin", `solana${exe}`))
    : [];
  const solana =
    process.env.SOLANA_BIN ??
    onPath("solana") ??
    firstExisting([join(home, `.local/share/solana/install/active_release/bin/solana${exe}`), ...releaseBins]);
  const anchor =
    process.env.ANCHOR_BIN ??
    onPath("anchor") ??
    firstExisting([join(home, `.local/bin/anchor${exe}`), join(home, `.avm/bin/anchor${exe}`), join(home, `.cargo/bin/anchor${exe}`)]);
  // On Windows a bare `bash` can be WSL's; the wasm build wants Git Bash next to this Node.
  const bash =
    process.env.BASH_BIN ??
    (process.platform === "win32"
      ? firstExisting(["C:/Program Files/Git/bin/bash.exe", "C:/Program Files/Git/usr/bin/bash.exe"])
      : "bash");
  const cargo = process.env.CARGO_BIN ?? onPath("cargo") ?? firstExisting([join(home, `.cargo/bin/cargo${exe}`)]);
  if (!solana) throw new Error("solana CLI not found; set SOLANA_BIN");
  if (!anchor) throw new Error("anchor CLI not found; set ANCHOR_BIN");
  if (!bash) throw new Error("bash not found; set BASH_BIN");
  if (!cargo) throw new Error("cargo not found; set CARGO_BIN");
  // anchor build runs `cargo build-sbf`, which lives next to the solana CLI.
  const pathKey = Object.keys(process.env).find((k) => k.toUpperCase() === "PATH") ?? "PATH";
  process.env[pathKey] = [dirname(solana), dirname(anchor), dirname(cargo), process.env[pathKey]].join(delimiter);
  return { solana, anchor, bash, cargo };
}

function run(cmd: string, args: string[], opts: { env?: Record<string, string> } = {}): void {
  console.log(`$ ${[cmd, ...args].join(" ")}`);
  const res = spawnSync(cmd, args, {
    cwd: ROOT,
    stdio: "inherit",
    env: { ...process.env, ...opts.env },
  });
  if (res.error) throw res.error;
  if (res.status !== 0) throw new Error(`${cmd} exited with ${res.status}`);
}

// ---------------------------------------------------------------------------------------------
// wallet, ids, IDL

export async function loadKeypair(path: string): Promise<KeyPairSigner> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(readFileSync(path, "utf8"));
  } catch {
    throw new Error(`cannot read keypair ${path}`);
  }
  if (!Array.isArray(parsed) || parsed.length !== 64) throw new Error(`${path} is not a 64-byte keypair`);
  return createKeyPairSignerFromBytes(Uint8Array.from(parsed as number[]));
}

/** Program ids from `[programs.<cluster>]` in Anchor.toml. */
export function anchorIds(cluster: Cluster): Record<string, string> {
  const toml = readFileSync(join(ROOT, "Anchor.toml"), "utf8");
  const section = cluster === "local" ? "localnet" : "devnet";
  const body = toml.split(`[programs.${section}]`)[1]?.split(/^\[/m)[0];
  if (!body) throw new Error(`Anchor.toml has no [programs.${section}]`);
  const ids: Record<string, string> = {};
  for (const m of body.matchAll(/^(\w+)\s*=\s*"(\w+)"/gm)) ids[m[1]] = m[2];
  return ids;
}

/**
 * Where deploys come from: each program's `.so` copied right after its own `anchor build -p`.
 * target/deploy itself is not safe: a later `-p` build overwrites the programs it depends on there
 * with their `cpi` (no entrypoint) builds.
 */
const STAGE = join(ROOT, "target/deploy-stage");
const soPath = (name: string) => join(STAGE, `${name}.so`);
const keypairPath = (name: string) => join(ROOT, "target/deploy", `${name}-keypair.json`);

type IdlIx = { name: string; discriminator: number[]; accounts: { name: string; writable?: boolean; signer?: boolean; optional?: boolean; address?: string }[] };
const idlCache = new Map<string, IdlIx[]>();
function idlIx(program: string, ix: string): IdlIx {
  if (!idlCache.has(program)) {
    const path = join(ROOT, "target/idl", `${program}.json`);
    if (!existsSync(path)) throw new Error(`${path} missing; build first`);
    idlCache.set(program, JSON.parse(readFileSync(path, "utf8")).instructions);
  }
  const found = idlCache.get(program)!.find((i) => i.name === ix);
  if (!found) throw new Error(`${program} IDL has no instruction ${ix}`);
  return found;
}

/** An instruction from the IDL's account list (names in snake_case) and Borsh-encoded args. */
function ixFromIdl(
  programId: string,
  program: string,
  name: string,
  accounts: Record<string, string | KeyPairSigner>,
  args: Uint8Array = new Uint8Array(),
) {
  const ix = idlIx(program, name);
  return {
    programAddress: address(programId),
    accounts: ix.accounts.map((a) => {
      const given = accounts[a.name] ?? a.address;
      if (given === undefined) throw new Error(`${name}: account ${a.name} not given`);
      const signer = typeof given === "string" ? undefined : given;
      const addr = typeof given === "string" ? address(given) : given.address;
      const role = a.signer
        ? a.writable
          ? AccountRole.WRITABLE_SIGNER
          : AccountRole.READONLY_SIGNER
        : a.writable
          ? AccountRole.WRITABLE
          : AccountRole.READONLY;
      return signer ? { address: addr, role, signer } : { address: addr, role };
    }),
    data: cat(new Uint8Array(ix.discriminator), args),
  };
}

// ---------------------------------------------------------------------------------------------
// RPC

type Rpc = ReturnType<typeof connect>;

async function fetchData(rpc: any, target: string): Promise<Uint8Array | null> {
  const { value } = await rpc.getAccountInfo(address(target), { encoding: "base64", commitment: "confirmed" }).send();
  if (!value) return null;
  return new Uint8Array(Buffer.from(value.data[0], "base64"));
}

async function fetchInfo(rpc: any, target: string) {
  const { value } = await rpc.getAccountInfo(address(target), { encoding: "base64", commitment: "confirmed" }).send();
  if (!value) return null;
  return { owner: value.owner as string, executable: value.executable as boolean, data: new Uint8Array(Buffer.from(value.data[0], "base64")) };
}

async function send(conn: Rpc, payer: KeyPairSigner, instructions: any[]): Promise<void> {
  const sendAndConfirm = sendAndConfirmTransactionFactory(conn as any);
  const { value: blockhash } = await conn.rpc.getLatestBlockhash().send();
  const message = pipe(
    createTransactionMessage({ version: 0 }),
    (m) => setTransactionMessageFeePayerSigner(payer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(blockhash, m),
    (m) => appendTransactionMessageInstructions(instructions, m),
  );
  const tx = await signTransactionMessageWithSigners(message);
  await sendAndConfirm(tx as any, { commitment: "confirmed" });
}

async function rentFor(rpc: any, bytes: number): Promise<bigint> {
  return BigInt(await rpc.getMinimumBalanceForRentExemption(BigInt(bytes)).send());
}

// ---------------------------------------------------------------------------------------------
// steps

export function build(bins: ReturnType<typeof resolveBins>, mockVrf: boolean): void {
  step("build");
  // One `-p` build per program, staged at once (see STAGE): a dependent program's build replaces
  // its dependencies in target/deploy with their `cpi` builds (no entrypoint, a few hundred bytes).
  mkdirSync(STAGE, { recursive: true });
  for (const p of PROGRAMS) {
    const features = p.key === "session" && mockVrf ? ["--", "--features", "mock-vrf"] : [];
    run(bins.anchor, ["build", "--tools-version", "v1.54", "-p", p.name, ...features]);
    copyFileSync(join(ROOT, "target/deploy", `${p.name}.so`), soPath(p.name));
  }
  run(bins.bash, ["crates/render/wasm/build.sh"]);
  run(bins.cargo, ["run", "-q", "-p", "art", "--", "pack", "--out", "target/render-assets"]);
}

/**
 * True if the SBF binary embeds `key`: either as bytes in rodata, or as the four `lddw` loads the
 * compiler emits for a constant (each 8-byte word split into imm32 low at +4 and imm32 high at +12).
 */
function embedsKey(so: Uint8Array, key: Uint8Array): boolean {
  if (contains(so, key)) return true;
  for (let w = 0; w < 32; w += 8) {
    const lo = key.subarray(w, w + 4);
    const hi = key.subarray(w + 4, w + 8);
    let found = false;
    for (let i = 0; i + 8 + 4 <= so.length && !found; i++) {
      found = sameBytes(so.subarray(i, i + 4), lo) && sameBytes(so.subarray(i + 8, i + 12), hi);
    }
    if (!found) return false;
  }
  return true;
}

/** "mock" when the built session accepts the mock VRF identity, "real" for the oracle identity. */
export function sessionVrfMode(): "mock" | "real" | "unknown" {
  if (!existsSync(soPath("deadroll_session"))) return "unknown";
  const so = readFileSync(soPath("deadroll_session"));
  const mock = embedsKey(so, addrBytes(MOCK_VRF_IDENTITY));
  const real = embedsKey(so, addrBytes(VRF_IDENTITY));
  if (mock && !real) return "mock";
  if (real && !mock) return "real";
  return "unknown";
}

function checkArtifacts(opts: DeployOptions): void {
  for (const p of PROGRAMS) {
    if (!existsSync(soPath(p.name))) throw new Error(`${soPath(p.name)} missing; run without --skip-build`);
    const size = readFileSync(soPath(p.name)).length;
    if (size < MIN_PROGRAM_SIZE) {
      throw new Error(`${p.name}.so is ${size} bytes, a no-entrypoint (cpi) build; rebuild with anchor build -p ${p.name}`);
    }
  }
  const mode = sessionVrfMode();
  if (opts.mockVrf && mode !== "mock") throw new Error(`session build is "${mode}", expected the mock-vrf build`);
  if (!opts.mockVrf && mode !== "real") {
    throw new Error(`refusing: target/deploy/deadroll_session.so is a "${mode}" VRF build, not the real-oracle build`);
  }
  for (const f of ["target/render-wasm/deadroll_render.wasm", "target/render-assets/palette.bin", "target/render-assets/book.bin"]) {
    if (!existsSync(join(ROOT, f))) throw new Error(`${f} missing; run without --skip-build`);
  }
}

async function preflight(conn: Rpc, opts: DeployOptions, wallet: KeyPairSigner, bins: ReturnType<typeof resolveBins>) {
  step("preflight");
  const version = await conn.rpc.getVersion().send().catch(() => null);
  if (!version) throw new Error(`no RPC at ${opts.url}`);
  console.log(`rpc ${opts.url} (solana-core ${version["solana-core"]})`);
  const ids = anchorIds(opts.cluster);
  const programIds: Record<string, string> = {};
  for (const p of PROGRAMS) {
    if (!existsSync(keypairPath(p.name))) throw new Error(`${keypairPath(p.name)} missing; program keypairs are never generated here`);
    const kp = await loadKeypair(keypairPath(p.name));
    if (ids[p.name] !== kp.address) {
      throw new Error(`${p.name}: keypair is ${kp.address} but Anchor.toml says ${ids[p.name]}`);
    }
    programIds[p.key] = kp.address;
  }
  const balance = (await conn.rpc.getBalance(wallet.address).send()).value;
  console.log(`wallet ${wallet.address}, ${sol(balance)} SOL`);
  console.log(`solana ${bins.solana}\nanchor ${bins.anchor}`);
  return { programIds, balance: BigInt(balance) };
}

/** True when the deployed program bytes equal the local `.so` (trailing zero padding allowed). */
async function programMatches(rpc: any, id: string, so: Uint8Array): Promise<{ deployed: boolean; same: boolean; capacity: number }> {
  const program = await fetchInfo(rpc, id);
  if (!program || !program.executable || program.owner !== BPF_UPGRADEABLE) return { deployed: false, same: false, capacity: 0 };
  const programData = addrAt(program.data, 4);
  const pd = await fetchData(rpc, programData);
  if (!pd) return { deployed: false, same: false, capacity: 0 };
  const body = pd.subarray(PROGRAMDATA_HEADER);
  const same = body.length >= so.length && sameBytes(body.subarray(0, so.length), so) && body.subarray(so.length).every((b) => b === 0);
  return { deployed: true, same, capacity: body.length };
}

async function estimateCost(conn: Rpc, opts: DeployOptions, programIds: Record<string, string>, wallet: KeyPairSigner) {
  const lines: string[] = [];
  let total = 0n;
  const add = (label: string, lamports: bigint) => {
    total += lamports;
    lines.push(`${label.padEnd(52)} ${sol(lamports).padStart(10)} SOL`);
  };
  let writeTxs = 0;
  for (const p of PROGRAMS) {
    const so = readFileSync(soPath(p.name));
    const state = await programMatches(conn.rpc, programIds[p.key], so);
    if (state.same) {
      add(`${p.name}: already deployed, identical`, 0n);
      continue;
    }
    writeTxs += Math.ceil(so.length / 1000) + 3;
    const buffer = await rentFor(conn.rpc, 37 + so.length);
    if (state.deployed) {
      const grow = so.length > state.capacity ? (await rentFor(conn.rpc, PROGRAMDATA_HEADER + so.length)) - (await rentFor(conn.rpc, PROGRAMDATA_HEADER + state.capacity)) : 0n;
      add(`${p.name}: upgrade buffer (refunded after deploy)`, buffer);
      if (grow > 0n) add(`${p.name}: programdata extension`, grow);
    } else {
      add(`${p.name}: programdata ${so.length} B`, await rentFor(conn.rpc, PROGRAMDATA_HEADER + so.length));
      add(`${p.name}: program account`, await rentFor(conn.rpc, 36));
      add(`${p.name}: buffer during deploy (refunded)`, buffer);
    }
  }
  const cfgNow = await fetchData(conn.rpc, GAME_CONFIG);
  if (!cfgNow) add("GameConfig", await rentFor(conn.rpc, CONFIG_LEN));
  else if (cfgNow.length < CONFIG_LEN) {
    add(`GameConfig resize ${cfgNow.length} -> ${CONFIG_LEN} B`, (await rentFor(conn.rpc, CONFIG_LEN)) - (await rentFor(conn.rpc, cfgNow.length)));
  }
  for (const seed of ["aspects", "resonances", "bestiary", "tileset", "prefabs", "traits", "monster_skills"]) {
    const [table] = await getProgramDerivedAddress({ programAddress: address(programIds.registry), seeds: [enc.encode(seed)] });
    if (!(await fetchData(conn.rpc, table))) add(`registry table ${seed}`, await rentFor(conn.rpc, 16 + 4096));
  }
  const blobs: [string, "rasterizer" | "palette" | "sprite_book", number][] = [
    ["target/render-wasm/deadroll_render.wasm", "rasterizer", opts.rasterizerVersion],
    ["target/render-assets/palette.bin", "palette", paletteVersion()],
    ["target/render-assets/book.bin", "sprite_book", opts.bookId],
  ];
  for (const [file, kind, id] of blobs) {
    const len = readFileSync(join(ROOT, file)).length;
    if (!(await fetchData(conn.rpc, await blobPda(kind, id)))) {
      add(`render ${kind} ${id} (${len} B)`, await rentFor(conn.rpc, 116 + len));
      writeTxs += Math.ceil(len / 900) + 4;
    }
  }
  const { map } = await mapAddresses(programIds.map, wallet.address, opts.mapIndex);
  if (!(await fetchData(conn.rpc, map))) add("demo map + layout", (await rentFor(conn.rpc, 200)) + (await rentFor(conn.rpc, 1400)));
  const fees = BigInt(writeTxs + 40) * 5000n + BigInt(opts.computeUnitPrice ?? 0) * BigInt(writeTxs) * 200n / 1_000_000n;
  add(`transaction fees (~${writeTxs + 40} txs)`, fees);
  add("safety margin", 200_000_000n);
  return { lines, total };
}

async function deployPrograms(conn: Rpc, opts: DeployOptions, programIds: Record<string, string>, bins: ReturnType<typeof resolveBins>) {
  step("programs");
  for (const p of PROGRAMS) {
    const so = readFileSync(soPath(p.name));
    const state = await programMatches(conn.rpc, programIds[p.key], so);
    if (state.same) {
      console.log(`${p.name} ${programIds[p.key]}: up to date`);
      continue;
    }
    console.log(`${p.name} ${programIds[p.key]}: ${state.deployed ? "upgrade" : "deploy"} (${so.length} bytes)`);
    const args = [
      "program", "deploy", soPath(p.name),
      "--program-id", keypairPath(p.name),
      "--url", opts.url,
      "--keypair", opts.wallet,
      "--commitment", "confirmed",
      "--max-sign-attempts", "50",
    ];
    if (opts.computeUnitPrice) args.push("--with-compute-unit-price", String(opts.computeUnitPrice));
    if (opts.cluster === "devnet") args.push("--use-rpc");
    run(bins.solana, args);
  }
}

function uriBytes(uri: string): Uint8Array {
  const raw = enc.encode(uri);
  if (raw.length > URI_BASE_LEN) throw new Error(`--uri-base is longer than ${URI_BASE_LEN} bytes`);
  const out = new Uint8Array(URI_BASE_LEN);
  out.set(raw);
  return out;
}

async function ensureConfig(conn: Rpc, opts: DeployOptions, wallet: KeyPairSigner, programIds: Record<string, string>) {
  step("GameConfig");
  const list = cat(...PROGRAMS.map((p) => addrBytes(programIds[p.key])));
  let cfg = await fetchData(conn.rpc, GAME_CONFIG);
  if (!cfg) {
    await send(conn, wallet, [
      ixFromIdl(programIds.registry, "deadroll_registry", "init_config", { authority: wallet, config: GAME_CONFIG }, cat(list, uriBytes(opts.uriBase))),
    ]);
    console.log(`init_config: ${GAME_CONFIG}, authority ${wallet.address}`);
    return;
  }
  const authority = addrAt(cfg, CFG.authority);
  if (authority !== wallet.address) {
    throw new Error(`GameConfig authority is ${authority}, not this wallet; reset the validator or use that wallet`);
  }
  if (cfg.length < CONFIG_LEN) {
    // Patch 7 appended `sprite_books`: an older account must grow before anything can load it.
    await send(conn, wallet, [ixFromIdl(programIds.registry, "deadroll_registry", "resize_config", { authority: wallet, config: GAME_CONFIG })]);
    console.log(`resize_config: ${cfg.length} -> ${CONFIG_LEN} bytes`);
  }
  if (!sameBytes(cfg.subarray(CFG.programs, CFG.programs + list.length), list)) {
    await send(conn, wallet, [ixFromIdl(programIds.registry, "deadroll_registry", "set_programs", { authority: wallet, config: GAME_CONFIG }, list)]);
    console.log("set_programs: program list updated");
  } else {
    console.log(`exists: ${GAME_CONFIG}, programs match`);
  }
  cfg = (await fetchData(conn.rpc, GAME_CONFIG))!;
  if (opts.uriBaseExplicit && !sameBytes(cfg.subarray(CFG.uri, CFG.uri + URI_BASE_LEN), uriBytes(opts.uriBase))) {
    await send(conn, wallet, [ixFromIdl(programIds.registry, "deadroll_registry", "set_uri_base", { authority: wallet, config: GAME_CONFIG }, uriBytes(opts.uriBase))]);
    console.log(`set_uri_base: ${opts.uriBase}`);
  }
}

function script(name: string, args: string[], opts: DeployOptions) {
  run(process.execPath, ["--experimental-strip-types", "--no-warnings", join(ROOT, "tools/scripts", name), ...args, "--url", opts.url, "--wallet", opts.wallet]);
}

function paletteVersion(): number {
  const pal = readFileSync(join(ROOT, "target/render-assets/palette.bin"));
  return pal[1] | (pal[2] << 8);
}

async function uploadRender(conn: Rpc, opts: DeployOptions, wallet: KeyPairSigner, programIds: Record<string, string>) {
  step("registry tables");
  script("seed-registry.ts", [], opts);
  // open_session copies the bestiary records, the spawns' monster skills, the tile costs and the
  // resonance tables (aspects, traits, resonances) into the session; finalize_map reads the
  // bestiary and the monster skills (patch 6).
  for (const seed of ["bestiary", "tileset", "aspects", "traits", "resonances", "monster_skills"]) {
    const [table] = await getProgramDerivedAddress({ programAddress: address(programIds.registry), seeds: [enc.encode(seed)] });
    const data = await fetchData(conn.rpc, table);
    if (!data || data.subarray(16).every((b) => b === 0)) throw new Error(`registry ${seed} table ${table} is not seeded`);
  }

  step("render blobs");
  const palVersion = paletteVersion();
  // Books first: finalize_rasterizer points GameConfig at the new rasterizer right away.
  script(
    "upload-sprites.ts",
    [
      "--palette", join(ROOT, "target/render-assets/palette.bin"), "--palette-version", String(palVersion),
      "--book", join(ROOT, "target/render-assets/book.bin"), "--book-id", String(opts.bookId),
    ],
    opts,
  );
  script("upload-rasterizer.ts", ["--version", String(opts.rasterizerVersion), "--wasm", join(ROOT, "target/render-wasm/deadroll_render.wasm")], opts);

  step("GameConfig pointers");
  const rasterizer = await blobPda("rasterizer", opts.rasterizerVersion);
  const palette = await blobPda("palette", palVersion);
  const raster = parseHeader((await fetchData(conn.rpc, rasterizer))!);
  const cfg = (await fetchData(conn.rpc, GAME_CONFIG))!;
  const accounts = { authority: wallet, config: GAME_CONFIG };
  if (addrAt(cfg, CFG.rasterizer) === rasterizer && sameBytes(cfg.subarray(CFG.sha, CFG.sha + 32), raster.sha256)) {
    console.log(`rasterizer: already ${rasterizer}`);
  } else {
    await send(conn, wallet, [ixFromIdl(programIds.registry, "deadroll_registry", "set_rasterizer", { ...accounts, blob: rasterizer })]);
    console.log(`set_rasterizer: ${rasterizer}`);
  }
  if (addrAt(cfg, CFG.palette) === palette) {
    console.log(`palette: already ${palette}`);
  } else {
    await send(conn, wallet, [ixFromIdl(programIds.registry, "deadroll_registry", "set_palette", { ...accounts, blob: palette })]);
    console.log(`set_palette: ${palette}`);
  }
  const book = await blobPda("sprite_book", opts.bookId);
  const books = [0, 1, 2, 3].map((i) => addrAt(cfg, CFG.spriteBooks + i * 32));
  if (books[0] === book && books.slice(1).every((b) => b === EMPTY_ADDRESS)) {
    console.log(`sprite books: already [${book}]`);
  } else {
    const ix = ixFromIdl(programIds.registry, "deadroll_registry", "set_sprite_books", accounts);
    ix.accounts.push({ address: address(book), role: AccountRole.READONLY });
    await send(conn, wallet, [ix]);
    console.log(`set_sprite_books: [${book}]`);
  }
  return { rasterizer, palette, paletteVersion: palVersion, book };
}

export async function mapAddresses(mapProgram: string, dm: string, index: number) {
  const programAddress = address(mapProgram);
  const [map] = await getProgramDerivedAddress({ programAddress, seeds: [enc.encode("map"), addrBytes(dm), u32(index)] });
  const [layout] = await getProgramDerivedAddress({ programAddress, seeds: [enc.encode("layout"), addrBytes(map)] });
  return { map, layout };
}

/** `write_spawns(entrance, exit, spawns)` args: Cell, Cell, Vec<Spawn> (9 bytes each). */
function spawnsArgs(p: MapPreset): Uint8Array {
  const spawns = p.spawns.map((s) =>
    cat(u8(s.bestiaryId), u8(s.x), u8(s.y), u8(s.level), u16(s.skills), u8(s.flags), u8(s.aspects), u8(s.pack)),
  );
  return cat(u8(p.entrance.x), u8(p.entrance.y), u8(p.exit.x), u8(p.exit.y), u32(spawns.length), ...spawns);
}

async function ensureDemoMap(conn: Rpc, opts: DeployOptions, wallet: KeyPairSigner, programIds: Record<string, string>) {
  step("demo map");
  const preset = loadMapPreset();
  const { map, layout } = await mapAddresses(programIds.map, wallet.address, opts.mapIndex);
  const prog = programIds.map;
  const edit = { dm: wallet, map, layout };
  let data = await fetchData(conn.rpc, map);
  if (!data) {
    await send(conn, wallet, [
      ixFromIdl(prog, "deadroll_map", "create_map", edit, cat(u32(opts.mapIndex), u8(preset.width), u8(preset.height), u8(preset.tier))),
    ]);
    console.log(`create_map: ${map} (index ${opts.mapIndex})`);
    data = (await fetchData(conn.rpc, map))!;
  }
  const status = data[MAP_STATUS];
  if (status !== 0) {
    const name = MAP_STATUS_NAMES[status] ?? String(status);
    if (status === 1) console.log(`${map}: already Finalized`);
    else console.log(`${map}: status ${name}; a session used it. For a fresh demo map pass --map-index ${opts.mapIndex + 1}`);
    return { map, layout, status: name, preset };
  }
  if (MAP_STRIDE !== 32) throw new Error("unexpected stride");
  await send(conn, wallet, [ixFromIdl(prog, "deadroll_map", "write_layout", edit, cat(u16(0), vec(layoutBytes(preset))))]);
  await send(conn, wallet, [ixFromIdl(prog, "deadroll_map", "write_spawns", edit, spawnsArgs(preset))]);
  // No secret cells in the demo preset; a fixed root keeps reruns identical. The builder rules
  // read the registry bestiary and monster skills (roles, pools, slots).
  const table = async (seed: string) =>
    (await getProgramDerivedAddress({ programAddress: address(programIds.registry), seeds: [enc.encode(seed)] }))[0];
  const finalize = { ...edit, bestiary: await table("bestiary"), monster_skills: await table("monster_skills") };
  await send(conn, wallet, [ixFromIdl(prog, "deadroll_map", "finalize_map", finalize, sha256(enc.encode(`deadroll:${preset.name}:no-secrets`)))]);
  console.log(`write_layout, write_spawns, finalize_map: ${map} Finalized`);
  return { map, layout, status: "Finalized", preset };
}

// ---------------------------------------------------------------------------------------------

export async function deploy(opts: DeployOptions): Promise<void> {
  const bins = resolveBins();
  const wallet = await loadKeypair(opts.wallet);
  const conn = connect(opts.url);
  const { programIds, balance } = await preflight(conn, opts, wallet, bins);

  if (!opts.skipBuild) build(bins, opts.mockVrf);
  checkArtifacts(opts);
  console.log(`session VRF: ${opts.mockVrf ? "mock-vrf (localnet only)" : "real MagicBlock oracle"}`);

  if (opts.confirmCost) {
    step("cost estimate");
    const { lines, total } = await estimateCost(conn, opts, programIds, wallet);
    if (!(await opts.confirmCost(lines, total, balance))) {
      console.log("stopped before sending anything");
      return;
    }
  }

  await deployPrograms(conn, opts, programIds, bins);
  await ensureConfig(conn, opts, wallet, programIds);
  const render = await uploadRender(conn, opts, wallet, programIds);
  const demo = await ensureDemoMap(conn, opts, wallet, programIds);

  step("summary");
  for (const p of PROGRAMS) console.log(`${p.key.padEnd(10)} ${programIds[p.key]}`);
  console.log(`GameConfig ${GAME_CONFIG} (authority ${wallet.address})`);
  console.log(`rasterizer ${render.rasterizer} (version ${opts.rasterizerVersion})`);
  console.log(`palette    ${render.palette} (version ${render.paletteVersion})`);
  console.log(`book       ${render.book} (id ${opts.bookId}, GameConfig.sprite_books[0])`);
  console.log(`demo map   ${demo.map} (DM ${wallet.address}, index ${opts.mapIndex}, ${demo.status})`);
  console.log(`layout     ${demo.layout}`);
  const after = (await conn.rpc.getBalance(wallet.address).send()).value;
  console.log(`wallet     ${sol(after)} SOL (spent ${sol(balance - BigInt(after))})`);
}

// ---------------------------------------------------------------------------------------------
// flags

export function parseFlags(argv: string[], known: Record<string, "flag" | "value">, usage: string): Record<string, string | true> {
  const out: Record<string, string | true> = {};
  for (let i = 0; i < argv.length; i++) {
    const kind = known[argv[i]];
    if (!kind) {
      console.error(`unknown argument ${argv[i]}\n\n${usage}`);
      process.exit(1);
    }
    if (kind === "flag") {
      out[argv[i]] = true;
      continue;
    }
    const value = argv[++i];
    if (value === undefined || value.startsWith("--")) {
      console.error(`${argv[i - 1]} needs a value\n\n${usage}`);
      process.exit(1);
    }
    out[argv[i - 1]] = value;
  }
  return out;
}

export function intFlag(value: string | true | undefined, fallback: number, max: number, name: string): number {
  if (value === undefined) return fallback;
  if (value === true || !/^\d+$/.test(value) || Number(value) > max) throw new Error(`${name} must be an integer 0..${max}`);
  return Number(value);
}

export const defaultWallet = () => join(homedir(), ".config/solana/id.json");
