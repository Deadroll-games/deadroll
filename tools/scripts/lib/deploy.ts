// The whole deploy pipeline (programs, GameConfig, registry tables, render blobs, demo map, city),
// shared by deploy-local.ts and deploy-devnet.ts. Every step is idempotent: a rerun skips what is
// already on chain and finishes what is not.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, utimesSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { maskText, maskUrl } from "./mask-url.ts";
import {
  AccountRole,
  address,
  appendTransactionMessageInstructions,
  createKeyPairSignerFromBytes,
  createTransactionMessage,
  getAddressDecoder,
  getAddressEncoder,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  type Address,
  type KeyPairSigner,
} from "@solana/kit";
import { PROGRAM_CRATES, PROGRAMS as SDK_PROGRAMS, type ProgramName } from "@deadroll/sdk";
import { MapStatus, findLayoutPda, findMapPda, getCreateMapInstruction, getFinalizeMapInstruction, getMapDecoder, getWriteLayoutInstruction, getWritePointsInstruction, getWriteSpawnsInstruction } from "@deadroll/sdk/map";
import {
  findTablePda,
  getGameConfigDataDecoder,
  getGameConfigDecoder,
  getInitConfigInstruction,
  getResizeConfigInstruction,
  getSetPaletteInstruction,
  getSetProgramsInstruction,
  getSetRasterizerInstruction,
  getSetSpriteBooksInstruction,
  getSetUriBaseInstruction,
} from "@deadroll/sdk/registry";
import { GAME_CONFIG, blobFits, connect, pda as blobPda, parseHeader } from "./render-program.ts";
import { MAP_STRIDE, layoutBytes, loadMapPreset, presetPoints, type MapPreset } from "./demo-map.ts";
import { GATED_SYSCALLS, featureActive, syscallProblems, type GatedSyscall } from "./syscalls.ts";

export const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const SYSTEM_PROGRAM = address("11111111111111111111111111111111");
const BPF_UPGRADEABLE = "BPFLoaderUpgradeab1e11111111111111111111111";
/** UpgradeableLoaderState::ProgramData header before the program bytes. */
const PROGRAMDATA_HEADER = 45;
/** Below this a `.so` is a `no-entrypoint` build, not a program (the smallest real one is ~48 KB). */
const MIN_PROGRAM_SIZE = 16 * 1024;

/** `ProgramIdx` order (crates/types): the order `init_config` expects. Ids and crates from the sdk. */
export const PROGRAMS = (Object.keys(SDK_PROGRAMS) as ProgramName[]).map((key) => ({ key, name: PROGRAM_CRATES[key] }));

/** `programs/session/src/vrf.rs`: only a mock-vrf build references the mock identity. */
const MOCK_VRF_IDENTITY = "5RotSMue7vx9xBPp3L8iQEKbKSc3ezbY4GqiJxDEe6sN";
const VRF_IDENTITY = "Bj7XBktwDmAg6J4FohS6oRHbevD6MMd8jnCZKvzKPiKV";

/** `GameConfig` account size before patch 7 (discriminator + `GameConfigData`) and now (+ `sprite_books: [Pubkey; 4]`). */
const CONFIG_LEGACY_LEN = 8 + 420;
const CONFIG_LEN = CONFIG_LEGACY_LEN + 4 * 32;
const URI_BASE_LEN = 64;
const EMPTY_ADDRESS = "11111111111111111111111111111111";
/** Rasterizer version of sprite format 2 (crates/render/FORMAT.md); version 1 and book 1 hold format 1. */
export const RASTERIZER_VERSION = 2;
/** First sprite book id of sprite format 2; book 1 holds format 1. */
export const FIRST_BOOK_ID = 2;
const ASSETS = "target/render-assets";

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
  /** First sprite book id; omitted: the first id whose accounts take the packed books. */
  bookId?: number;
  mapIndex: number;
  /** CI: the programs were loaded at validator genesis (`--upgradeable-program`), so the ids come
   * from the sdk, no program keypair is read and nothing is deployed. */
  preloaded?: boolean;
  computeUnitPrice?: number;
  /** Called with the cost table before anything is sent; return false to stop. */
  confirmCost?: (lines: string[], totalLamports: bigint, balance: bigint) => Promise<boolean>;
};

// ---------------------------------------------------------------------------------------------
// small helpers

const enc = new TextEncoder();
const sha256 = (bytes: Uint8Array) => new Uint8Array(createHash("sha256").update(bytes).digest());
const sol = (lamports: bigint | number) => (Number(lamports) / 1e9).toFixed(4);

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

/** `SOLANA_BIN` / `BASH_BIN` / `CARGO_BIN`, then PATH, then the usual install folders. */
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
  // On Windows a bare `bash` can be WSL's; the wasm build wants Git Bash next to this Node.
  const bash =
    process.env.BASH_BIN ??
    (process.platform === "win32"
      ? firstExisting(["C:/Program Files/Git/bin/bash.exe", "C:/Program Files/Git/usr/bin/bash.exe"])
      : "bash");
  const cargo = process.env.CARGO_BIN ?? onPath("cargo") ?? firstExisting([join(home, `.cargo/bin/cargo${exe}`)]);
  if (!solana) throw new Error("solana CLI not found; set SOLANA_BIN");
  if (!bash) throw new Error("bash not found; set BASH_BIN");
  if (!cargo) throw new Error("cargo not found; set CARGO_BIN");
  // `cargo build-sbf` lives next to the solana CLI.
  const pathKey = Object.keys(process.env).find((k) => k.toUpperCase() === "PATH") ?? "PATH";
  process.env[pathKey] = [dirname(solana), dirname(cargo), process.env[pathKey]].join(delimiter);
  return { solana, bash, cargo };
}

function run(cmd: string, args: string[], opts: { env?: Record<string, string>; cwd?: string } = {}): void {
  console.log(`$ ${maskText([cmd, ...args].join(" "))}`);
  const res = spawnSync(cmd, args, {
    cwd: opts.cwd ?? ROOT,
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

/**
 * Where deploys come from: each program's `.so` copied right after its own build, so a
 * mock-vrf session never has to stay in target/deploy.
 */
const STAGE = join(ROOT, "target/deploy-stage");
const soPath = (name: string) => join(STAGE, `${name}.so`);
const keypairPath = (name: string) => join(ROOT, "target/deploy", `${name}-keypair.json`);
const SESSION_SRC = join(ROOT, "programs/session/src/lib.rs");
/** cargo target dir of the deploy builds (see build). */
const DEPLOY_CACHE = join(ROOT, "target/sbf-deploy");

/** Sets the file's mtime to now (contents unchanged), so cargo sees the crate as changed. */
function touch(path: string): void {
  const now = new Date();
  utimesSync(path, now, now);
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
  mkdirSync(STAGE, { recursive: true });
  for (const p of PROGRAMS) {
    const mock = p.key === "session" && mockVrf;
    // The mock-vrf and the plain session builds share one output file (a cdylib has no hash in its
    // name), and cargo would take either for fresh. A newer source makes it rebuild the session crate
    // only, before this build and after a mock one (so the next plain build is real again).
    if (p.key === "session") touch(SESSION_SRC);
    // cargo-build-sbf strips into target/deploy only when the file there is older than its build, so
    // a newer one from `anchor build` would stay and be staged instead of this build.
    const built = join(ROOT, "target/deploy", `${p.name}.so`);
    rmSync(built, { force: true });
    // What `anchor build -p` runs, minus its IDL step (the IDL comes from @deadroll/sdk), in a cache of
    // its own. Not through anchor: on Windows it starts cargo in a canonicalized `\\?\D:\...` folder,
    // and cargo then mixes `\\?\` and plain source paths, rebuilds the shared crates every time and
    // puts other panic paths into the .so (same code, other bytes, a needless upgrade on devnet). Only
    // this function writes DEPLOY_CACHE, so the bytes depend on the code alone.
    run(bins.cargo, ["build-sbf", "--tools-version", "v1.54", "--arch", "v3", ...(mock ? ["--features", "mock-vrf"] : [])], {
      cwd: join(ROOT, "programs", p.key),
      env: { CARGO_TARGET_DIR: DEPLOY_CACHE, SBF_OUT_PATH: join(ROOT, "target/deploy") },
    });
    if (mock) {
      // Moved, not copied: a mock .so left in target/deploy would survive the next plain anchor build
      // (see above).
      renameSync(built, soPath(p.name));
      touch(SESSION_SRC);
    } else {
      copyFileSync(built, soPath(p.name));
    }
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
      throw new Error(`${p.name}.so is ${size} bytes, a no-entrypoint (cpi) build; run without --skip-build`);
    }
  }
  const mode = sessionVrfMode();
  if (opts.mockVrf && mode !== "mock") throw new Error(`session build is "${mode}", expected the mock-vrf build`);
  if (!opts.mockVrf && mode !== "real") {
    throw new Error(`refusing: target/deploy/deadroll_session.so is a "${mode}" VRF build, not the real-oracle build`);
  }
  for (const f of ["target/render-wasm/deadroll_render.wasm", `${ASSETS}/palette.bin`]) {
    if (!existsSync(join(ROOT, f))) throw new Error(`${f} missing; run without --skip-build`);
  }
  if (bookFiles().length === 0) throw new Error(`${ASSETS}/book_<id>.bin missing; run without --skip-build`);
}

/** Refuses programs that call a syscall in `inactive` (lib/syscalls.ts). */
export function refuseGatedSyscalls(inactive: readonly GatedSyscall[] = GATED_SYSCALLS): void {
  const programs = PROGRAMS.map((p) => ({ name: p.name, so: readFileSync(soPath(p.name)) }));
  const problems = syscallProblems(programs, inactive);
  if (problems.length > 0) throw new Error(`refusing: a program calls a syscall inactive on this cluster:\n  ${problems.join("\n  ")}`);
}

/** The gated syscalls whose feature is not active on the cluster, then `refuseGatedSyscalls`. */
async function checkSyscalls(conn: Rpc): Promise<void> {
  step("syscalls");
  const inactive: GatedSyscall[] = [];
  for (const s of GATED_SYSCALLS) if (!featureActive(await fetchData(conn.rpc, s.feature))) inactive.push(s);
  refuseGatedSyscalls(inactive);
  console.log(inactive.length > 0 ? `no program calls ${inactive.map((s) => s.name).join(", ")} (inactive here)` : "every gated syscall is active here");
}

async function preflight(conn: Rpc, opts: DeployOptions, wallet: KeyPairSigner, bins: ReturnType<typeof resolveBins>) {
  step("preflight");
  const version = await conn.rpc.getVersion().send().catch(() => null);
  if (!version) throw new Error(`no RPC at ${maskUrl(opts.url)}`);
  console.log(`rpc ${maskUrl(opts.url)} (solana-core ${version["solana-core"]})`);
  const programIds: Record<string, string> = {};
  for (const p of PROGRAMS) {
    if (opts.preloaded) {
      programIds[p.key] = SDK_PROGRAMS[p.key];
      continue;
    }
    if (!existsSync(keypairPath(p.name))) throw new Error(`${keypairPath(p.name)} missing; program keypairs are never generated here`);
    const kp = await loadKeypair(keypairPath(p.name));
    // The sdk ids are the IDL addresses; an sdk test keeps them equal to Anchor.toml.
    if (SDK_PROGRAMS[p.key] !== kp.address) {
      throw new Error(`${p.name}: keypair is ${kp.address} but the sdk (Anchor.toml) says ${SDK_PROGRAMS[p.key]}`);
    }
    programIds[p.key] = kp.address;
  }
  const balance = (await conn.rpc.getBalance(wallet.address).send()).value;
  console.log(`wallet ${wallet.address}, ${sol(balance)} SOL`);
  console.log(`solana ${bins.solana}\ncargo ${bins.cargo}`);
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
  for (const seed of ["aspects", "resonances", "bestiary", "tileset", "prefabs", "traits", "monster_skills", "triples"]) {
    const [table] = await findTablePda({ seed });
    if (!(await fetchData(conn.rpc, table))) add(`registry table ${seed}`, await rentFor(conn.rpc, 16 + 4096));
  }
  const blobs: [string, "rasterizer" | "palette" | "sprite_book", number][] = [
    ["target/render-wasm/deadroll_render.wasm", "rasterizer", opts.rasterizerVersion],
    [`${ASSETS}/palette.bin`, "palette", paletteVersion()],
    ...bookFiles().map((b): [string, "sprite_book", number] => [relative(ROOT, b.file), "sprite_book", b.id]),
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
    if (opts.preloaded) throw new Error(`${p.name}: the validator does not hold ${soPath(p.name)} (--preloaded never deploys)`);
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
  const programs = PROGRAMS.map((p) => address(programIds[p.key]));
  const accounts = { authority: wallet, config: GAME_CONFIG };
  let cfg = await fetchData(conn.rpc, GAME_CONFIG);
  if (!cfg) {
    await send(conn, wallet, [getInitConfigInstruction({ ...accounts, systemProgram: SYSTEM_PROGRAM, programs, uriBase: uriBytes(opts.uriBase) })]);
    console.log(`init_config: ${GAME_CONFIG}, authority ${wallet.address}`);
    return;
  }
  // GameConfigData: read alike before patch 7 (no sprite_books) and after.
  const core = getGameConfigDataDecoder().decode(cfg, 8);
  const authority = core.authority;
  if (authority !== wallet.address) {
    throw new Error(`GameConfig authority is ${authority}, not this wallet; reset the validator or use that wallet`);
  }
  if (cfg.length < CONFIG_LEN) {
    // Patch 7 appended `sprite_books`: an older account must grow before anything can load it.
    await send(conn, wallet, [getResizeConfigInstruction({ ...accounts, systemProgram: SYSTEM_PROGRAM })]);
    console.log(`resize_config: ${cfg.length} -> ${CONFIG_LEN} bytes`);
  }
  if (core.programs.length !== programs.length || core.programs.some((a, i) => a !== programs[i])) {
    await send(conn, wallet, [getSetProgramsInstruction({ ...accounts, programs })]);
    console.log("set_programs: program list updated");
  } else {
    console.log(`exists: ${GAME_CONFIG}, programs match`);
  }
  cfg = (await fetchData(conn.rpc, GAME_CONFIG))!;
  if (opts.uriBaseExplicit && !sameBytes(Uint8Array.from(getGameConfigDataDecoder().decode(cfg, 8).uriBase), uriBytes(opts.uriBase))) {
    await send(conn, wallet, [getSetUriBaseInstruction({ ...accounts, uriBase: uriBytes(opts.uriBase) })]);
    console.log(`set_uri_base: ${opts.uriBase}`);
  }
}

function script(name: string, args: string[], opts: DeployOptions) {
  run(process.execPath, ["--experimental-strip-types", "--no-warnings", join(ROOT, "tools/scripts", name), ...args, "--url", opts.url, "--wallet", opts.wallet]);
}

function paletteVersion(): number {
  const pal = readFileSync(join(ROOT, `${ASSETS}/palette.bin`));
  return pal[1] | (pal[2] << 8);
}

/** The books `art pack` wrote (`book_<id>.bin`, at most 4), in id order: the GameConfig lookup order. */
export function bookFiles(dir = join(ROOT, ASSETS)): { file: string; id: number }[] {
  if (!existsSync(dir)) return [];
  const books = readdirSync(dir)
    .filter((n) => /^book_\d+\.bin$/.test(n))
    .map((n) => {
      const body = readFileSync(join(dir, n));
      return { file: join(dir, n), id: body[1] | (body[2] << 8) };
    })
    .sort((a, b) => a.id - b.id);
  if (books.length > 4) throw new Error(`${books.length} sprite books, GameConfig.sprite_books holds 4`);
  return books;
}

/**
 * Packs the sprite books (`art pack --first-book`) from the first id whose accounts take them
 * (`blobFits`). A book finalized with other sprites never changes, so after a sprite change the
 * books move to the next free ids; a rerun finds the same ids. `--book-id` fixes the first id.
 */
async function packSpriteBooks(conn: Rpc, opts: DeployOptions, bins: ReturnType<typeof resolveBins>): Promise<void> {
  let first = opts.bookId ?? FIRST_BOOK_ID;
  if (first < FIRST_BOOK_ID) throw new Error(`--book-id must be ${FIRST_BOOK_ID} or more: book 1 holds sprite format 1`);
  for (;;) {
    run(bins.cargo, ["run", "-q", "-p", "art", "--", "pack", "--out", ASSETS, "--first-book", String(first)]);
    let taken: number | null = null;
    for (const b of bookFiles()) {
      const onChain = await fetchData(conn.rpc, await blobPda("sprite_book", b.id));
      if (!blobFits("sprite_book", b.id, readFileSync(b.file), onChain)) {
        taken = b.id;
        break;
      }
    }
    if (taken === null) {
      console.log(`sprite books from id ${first}`);
      return;
    }
    if (opts.bookId !== undefined) throw new Error(`sprite book ${taken} already holds other sprites; pass a free --book-id or leave it out`);
    first = taken + 1;
    if (first + 4 > 0xffff) throw new Error("no free sprite book id");
  }
}

async function uploadRender(conn: Rpc, opts: DeployOptions, wallet: KeyPairSigner, programIds: Record<string, string>) {
  step("registry tables");
  script("seed-registry.ts", [], opts);
  // open_session copies the bestiary records, the spawns' monster skills, the tile costs and the
  // resonance tables (aspects, traits, resonances, triples) into the session; finalize_map reads the
  // bestiary and the monster skills (patch 6) and the tileset (patch 13).
  for (const seed of ["bestiary", "tileset", "aspects", "traits", "resonances", "triples", "monster_skills"]) {
    const [table] = await findTablePda({ seed });
    const data = await fetchData(conn.rpc, table);
    if (!data || data.subarray(16).every((b) => b === 0)) throw new Error(`registry ${seed} table ${table} is not seeded`);
  }

  step("render blobs");
  const palVersion = paletteVersion();
  // Books first: finalize_rasterizer points GameConfig at the new rasterizer right away.
  script(
    "upload-sprites.ts",
    [
      "--palette", join(ROOT, `${ASSETS}/palette.bin`), "--palette-version", String(palVersion),
      ...bookFiles().flatMap((b) => ["--book", b.file, "--book-id", String(b.id)]),
    ],
    opts,
  );
  script("upload-rasterizer.ts", ["--version", String(opts.rasterizerVersion), "--wasm", join(ROOT, "target/render-wasm/deadroll_render.wasm")], opts);

  step("GameConfig pointers");
  const rasterizer = await blobPda("rasterizer", opts.rasterizerVersion);
  const palette = await blobPda("palette", palVersion);
  const raster = parseHeader((await fetchData(conn.rpc, rasterizer))!);
  const cfg = getGameConfigDecoder().decode((await fetchData(conn.rpc, GAME_CONFIG))!);
  const accounts = { authority: wallet, config: GAME_CONFIG };
  if (cfg.core.rasterizer === rasterizer && sameBytes(Uint8Array.from(cfg.core.rasterizerSha256), raster.sha256)) {
    console.log(`rasterizer: already ${rasterizer}`);
  } else {
    await send(conn, wallet, [getSetRasterizerInstruction({ ...accounts, blob: address(rasterizer) })]);
    console.log(`set_rasterizer: ${rasterizer}`);
  }
  if (cfg.core.palette === palette) {
    console.log(`palette: already ${palette}`);
  } else {
    await send(conn, wallet, [getSetPaletteInstruction({ ...accounts, blob: address(palette) })]);
    console.log(`set_palette: ${palette}`);
  }
  const books: string[] = [];
  for (const b of bookFiles()) books.push(await blobPda("sprite_book", b.id));
  const now: string[] = cfg.spriteBooks;
  const want = [...books, ...Array(4 - books.length).fill(EMPTY_ADDRESS)];
  if (now.every((b, i) => b === want[i])) {
    console.log(`sprite books: already [${books.join(", ")}]`);
  } else {
    // The books ride as remaining accounts, in lookup order.
    const ix = getSetSpriteBooksInstruction(accounts);
    await send(conn, wallet, [{ ...ix, accounts: [...ix.accounts, ...books.map((b) => ({ address: address(b), role: AccountRole.READONLY }))] }]);
    console.log(`set_sprite_books: [${books.join(", ")}]`);
  }
  return { rasterizer, palette, paletteVersion: palVersion, books };
}

export async function mapAddresses(mapProgram: string, dm: string, index: number) {
  const programAddress = address(mapProgram);
  const [map] = await findMapPda({ dm: address(dm), index }, { programAddress });
  const [layout] = await findLayoutPda({ map }, { programAddress });
  return { map, layout };
}

async function ensureDemoMap(conn: Rpc, opts: DeployOptions, wallet: KeyPairSigner, programIds: Record<string, string>) {
  step("demo map");
  const preset = loadMapPreset();
  const { map, layout } = await mapAddresses(programIds.map, wallet.address, opts.mapIndex);
  const edit = { dm: wallet, map, layout };
  let data = await fetchData(conn.rpc, map);
  if (!data) {
    await send(conn, wallet, [
      getCreateMapInstruction({ ...edit, systemProgram: SYSTEM_PROGRAM, index: opts.mapIndex, width: preset.width, height: preset.height, tier: preset.tier, minPlayers: preset.minPlayers, chestRadius: preset.chestRadius, settleTimeoutSecs: preset.settleTimeoutSecs, biome: preset.biome }),
    ]);
    console.log(`create_map: ${map} (index ${opts.mapIndex})`);
    data = (await fetchData(conn.rpc, map))!;
  }
  const status = getMapDecoder().decode(data).core.status;
  if (status !== MapStatus.Draft) {
    const name = MapStatus[status] ?? String(status);
    if (status === 1) console.log(`${map}: already Finalized`);
    else console.log(`${map}: status ${name}; a session used it. For a fresh demo map pass --map-index ${opts.mapIndex + 1}`);
    return { map, layout, status: name, preset };
  }
  if (MAP_STRIDE !== 32) throw new Error("unexpected stride");
  await send(conn, wallet, [getWriteLayoutInstruction({ ...edit, offset: 0, cells: layoutBytes(preset) })]);
  await send(conn, wallet, [getWriteSpawnsInstruction({ ...edit, spawns: preset.spawns })]);
  await send(conn, wallet, [getWritePointsInstruction({ ...edit, points: presetPoints(preset), bossTable: preset.bossTable })]);
  // No secret cells in the demo preset; a fixed root keeps reruns identical. The builder rules
  // read the registry bestiary, monster skills (roles, pools, slots), reward tables and tileset.
  const table = async (seed: string) => (await findTablePda({ seed }))[0];
  const secretsRoot = sha256(enc.encode(`deadroll:${preset.name}:no-secrets`));
  await send(conn, wallet, [
    getFinalizeMapInstruction({ ...edit, bestiary: await table("bestiary"), monsterSkills: await table("monster_skills"), rewards: await table("rewards"), tileset: await table("tileset"), secretsRoot }),
  ]);
  console.log(`write_layout, write_spawns, write_points, finalize_map: ${map} Finalized`);
  return { map, layout, status: "Finalized", preset };
}

// ---------------------------------------------------------------------------------------------

export async function deploy(opts: DeployOptions): Promise<void> {
  const bins = resolveBins();
  const wallet = await loadKeypair(opts.wallet);
  const conn = connect(opts.url);
  const { programIds, balance } = await preflight(conn, opts, wallet, bins);

  if (!opts.skipBuild) build(bins, opts.mockVrf);
  step("sprite books");
  await packSpriteBooks(conn, opts, bins);
  checkArtifacts(opts);
  console.log(`session VRF: ${opts.mockVrf ? "mock-vrf (localnet only)" : "real MagicBlock oracle"}`);
  await checkSyscalls(conn);

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
  step("city");
  script("seed-city.ts", [], opts);

  step("summary");
  for (const p of PROGRAMS) console.log(`${p.key.padEnd(10)} ${programIds[p.key]}`);
  console.log(`GameConfig ${GAME_CONFIG} (authority ${wallet.address})`);
  console.log(`rasterizer ${render.rasterizer} (version ${opts.rasterizerVersion})`);
  console.log(`palette    ${render.palette} (version ${render.paletteVersion})`);
  bookFiles().forEach((b, i) => console.log(`book       ${render.books[i]} (id ${b.id}, GameConfig.sprite_books[${i}])`));
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
