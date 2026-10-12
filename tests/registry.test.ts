// Registry program tests on the shared local validator (agents/TASKS.md, decision blocks).
// `anchor build --tools-version v1.54 -p deadroll_registry`, `anchor deploy -p deadroll_registry
// --provider.cluster localnet`, then:
//   cd tests && ANCHOR_PROVIDER_URL=http://127.0.0.1:8899 ANCHOR_WALLET=<keypair json> \
//     node --import tsx --test --test-concurrency=1 registry.test.ts
// `GameConfig` and the tables are singletons other lanes read: the tests use the wallet as the
// authority, read what already exists, and skip authority-only cases if someone else owns it.
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { before, describe, it } from "node:test";
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
  getBase64Encoder,
  getProgramDerivedAddress,
  lamports,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
} from "@solana/kit";

const idl = JSON.parse(readFileSync(new URL("../target/idl/deadroll_registry.json", import.meta.url), "utf8"));
const REGISTRY = address(idl.address);
const SYSTEM_PROGRAM = address("11111111111111111111111111111111");

// Program ids in `ProgramIdx` order (Anchor.toml, agents/CONTRACTS.md 1).
const PROGRAMS = [
  "dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG", // Registry
  "DgQCsXAe7Q7h8zdtToiTvk8XqNjjbXuD6xith2BmEiRB", // Render
  "9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL", // Character
  "48FtbrDegNrAUHFza9HdJzZAvUo3uUNqs1kNnZxFsqJS", // Items
  "2YeyBfQ5xkRKD2kagxRS2yZYCQdWt5qidikaKfQhLBXH", // Map
  "3gzfih2WaBgXcYZ57jWokmZM3FDE6QHX5y4R9EvKZoNJ", // Session
  "2Cr5vB5awqDVx6XRcxtFFYQEG6Zy3aSAcKcqzh1xbVjS", // City
];
const RENDER_PROGRAM = address(PROGRAMS[1]);

const TABLE = { Aspects: 0, Resonances: 1, Bestiary: 2, Tileset: 3, Prefabs: 4 };
const TABLE_SEED = ["aspects", "resonances", "bestiary", "tileset", "prefabs"];
const MONSTER_SIZE = 48;
const BESTIARY_MAX = 32;
/** Body starts after the discriminator and the 8-byte table header. */
const TABLE_BODY = 16;
/** `GameConfigData` offsets after the 8-byte discriminator. */
const CFG = { authority: 9, programs: 41, rasterizerVersion: 265, rasterizer: 267, sha: 299, palette: 331, uri: 363 };

const rpcUrl = new URL(process.env.ANCHOR_PROVIDER_URL ?? "http://127.0.0.1:8899");
const wsUrl = new URL(rpcUrl);
wsUrl.protocol = "ws:";
wsUrl.port = String(Number(rpcUrl.port) + 1);
const rpc = createSolanaRpc(rpcUrl.toString());
const rpcSubscriptions = createSolanaRpcSubscriptions(wsUrl.toString());
const airdrop = airdropFactory({ rpc, rpcSubscriptions });
const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
const enc = getAddressEncoder();

// ---------- encoding ----------

function err(name: string): number {
  const e = idl.errors.find((x: { name: string }) => x.name === name);
  assert.ok(e, `no error ${name} in the IDL`);
  return e.code;
}

function disc(ix: string): Uint8Array {
  const i = idl.instructions.find((x: { name: string }) => x.name === ix);
  assert.ok(i, `no instruction ${ix} in the IDL`);
  return Uint8Array.from(i.discriminator);
}

function concat(...parts: (Uint8Array | number[])[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

const u16 = (v: number) => [v & 0xff, (v >> 8) & 0xff];
const u32 = (v: number) => [v & 0xff, (v >> 8) & 0xff, (v >> 16) & 0xff, (v >>> 24) & 0xff];
const vec = (bytes: Uint8Array | number[]) => concat(u32(bytes.length), bytes);
const programsBytes = (list: string[]) => concat(...list.map((p) => enc.encode(address(p))));
const uriBytes = (s: string) => {
  const out = new Uint8Array(64);
  out.set(new TextEncoder().encode(s));
  return out;
};

/** A valid bestiary record (a wolf-like fast monster). */
function monster(overrides: Record<number, number> = {}): Uint8Array {
  const r = new Uint8Array(MONSTER_SIZE);
  r[0] = 1; // present
  r[1] = 1; // role Fast
  r[3] = 1; // size
  r[4] = 1; // min tier
  r[5] = 5; // speed
  r[9] = 0xff; // no predisposition
  r.set(u16(3), 30);
  r.set(u16(6), 32);
  for (const [i, v] of Object.entries(overrides)) r[Number(i)] = v;
  return r;
}

// ---------- addresses and instructions ----------

type Signer = Awaited<ReturnType<typeof generateKeyPairSigner>>;

async function pda(seeds: string[], program = REGISTRY) {
  const bytes = seeds.map((s) => new TextEncoder().encode(s));
  const [addr] = await getProgramDerivedAddress({ programAddress: program, seeds: bytes });
  return addr;
}

const configAddress = () => pda(["config"]);
const tableAddress = (kind: number) => pda([TABLE_SEED[kind]]);

const signerMeta = (s: Signer, writable = false) => ({
  address: s.address,
  role: writable ? AccountRole.WRITABLE_SIGNER : AccountRole.READONLY_SIGNER,
  signer: s,
});
const meta = (a: string, writable = false) => ({
  address: address(a),
  role: writable ? AccountRole.WRITABLE : AccountRole.READONLY,
});

const initConfigIx = async (authority: Signer, programs: string[], uri: string) => ({
  programAddress: REGISTRY,
  accounts: [signerMeta(authority, true), meta(await configAddress(), true), meta(SYSTEM_PROGRAM)],
  data: concat(disc("init_config"), programsBytes(programs), uriBytes(uri)),
});

const editIx = async (authority: Signer, name: string, args: Uint8Array) => ({
  programAddress: REGISTRY,
  accounts: [signerMeta(authority), meta(await configAddress(), true)],
  data: concat(disc(name), args),
});

const pullIx = async (authority: Signer, name: string, blob: string) => ({
  programAddress: REGISTRY,
  accounts: [signerMeta(authority), meta(await configAddress(), true), meta(blob)],
  data: disc(name),
});

const initTableIx = async (authority: Signer, kind: number) => ({
  programAddress: REGISTRY,
  accounts: [
    signerMeta(authority, true),
    meta(await configAddress()),
    meta(await tableAddress(kind), true),
    meta(SYSTEM_PROGRAM),
  ],
  data: concat(disc("init_table"), [kind]),
});

const writeTableIx = async (authority: Signer, kind: number, offset: number, bytes: Uint8Array | number[]) => ({
  programAddress: REGISTRY,
  accounts: [signerMeta(authority), meta(await configAddress()), meta(await tableAddress(kind), true)],
  data: concat(disc("write_table"), [kind], u16(offset), vec(bytes)),
});

// ---------- transactions ----------

async function funded(): Promise<Signer> {
  const s = await generateKeyPairSigner();
  await airdrop({ recipientAddress: s.address, lamports: lamports(2_000_000_000n), commitment: "confirmed" });
  return s;
}

async function send(payer: Signer, ixs: any[]) {
  const { value: blockhash } = await rpc.getLatestBlockhash({ commitment: "confirmed" }).send();
  const msg = pipe(
    createTransactionMessage({ version: 0 }),
    (m) => setTransactionMessageFeePayerSigner(payer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(blockhash, m),
    (m) => appendTransactionMessageInstructions(ixs, m),
  );
  const tx = await signTransactionMessageWithSigners(msg);
  try {
    await sendAndConfirm(tx as any, { commitment: "confirmed" });
  } catch (e: any) {
    const logs: string[] = e?.context?.logs ?? [];
    throw new Error(`${e.message}\n${logs.join("\n")}`, { cause: e });
  }
}

/** The custom program error code of a failed send: from the error chain, else from the logs. */
function customCode(e: any): number | undefined {
  for (let x = e; x; x = x.cause) {
    if (typeof x.context?.code === "number" && "index" in (x.context ?? {})) return x.context.code;
  }
  for (let x = e; x; x = x.cause) {
    for (const line of x.context?.logs ?? []) {
      const m = /custom program error: 0x([0-9a-f]+)/i.exec(line);
      if (m) return parseInt(m[1], 16);
    }
  }
  return undefined;
}

async function expectErr(p: Promise<unknown>, code: number) {
  try {
    await p;
  } catch (e) {
    assert.equal(customCode(e), code, `expected custom error ${code}, got: ${e}`);
    return;
  }
  assert.fail(`expected custom error ${code}, transaction succeeded`);
}

// ---------- accounts ----------

async function maybeData(addr: string): Promise<Uint8Array | null> {
  const { value } = await rpc.getAccountInfo(address(addr), { encoding: "base64", commitment: "confirmed" }).send();
  return value ? Uint8Array.from(getBase64Encoder().encode(value.data[0])) : null;
}

async function data(addr: string): Promise<Uint8Array> {
  const d = await maybeData(addr);
  assert.ok(d, `account ${addr} missing`);
  return d;
}

/** Minimal base58 encoder (accounts are read as raw bytes). */
function bs58(bytes: Uint8Array): string {
  const alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  let n = 0n;
  for (const b of bytes) n = (n << 8n) | BigInt(b);
  let out = "";
  while (n > 0n) {
    out = alphabet[Number(n % 58n)] + out;
    n /= 58n;
  }
  for (const b of bytes) {
    if (b !== 0) break;
    out = "1" + out;
  }
  return out;
}

// ---------- tests ----------

describe("registry", () => {
  let authority: Signer;
  let stranger: Signer;
  /** The wallet owns the GameConfig (created here or already). */
  let owned = false;
  const ownerSkip = "GameConfig belongs to another authority on this validator";

  before(async () => {
    const path = process.env.ANCHOR_WALLET;
    assert.ok(path, "set ANCHOR_WALLET to a funded keypair json");
    authority = await createKeyPairSignerFromBytes(Uint8Array.from(JSON.parse(readFileSync(path, "utf8"))));
    stranger = await funded();
    const cfg = await maybeData(await configAddress());
    if (!cfg) {
      await send(authority, [await initConfigIx(authority, PROGRAMS, "https://example.invalid/nft/")]);
      owned = true;
    } else {
      owned = bs58(cfg.slice(CFG.authority, CFG.authority + 32)) === authority.address;
    }
  });

  describe("init_config", () => {
    it("stores the authority and all 7 program ids", async () => {
      const cfg = await data(await configAddress());
      assert.equal(cfg[8], 1, "version first after the discriminator");
      for (let i = 0; i < PROGRAMS.length; i++) {
        const at = CFG.programs + i * 32;
        assert.equal(bs58(cfg.slice(at, at + 32)), PROGRAMS[i], `programs[${i}]`);
      }
      assert.equal(cfg.length, 8 + 420, "discriminator + GameConfigData::SIZE");
    });

    it("rejects a second init", async () => {
      await assert.rejects(send(authority, [await initConfigIx(authority, PROGRAMS, "")]));
    });
  });

  describe("set_programs / set_uri_base", () => {
    it("rejects a non-authority", async () => {
      await expectErr(
        send(stranger, [await editIx(stranger, "set_programs", programsBytes(PROGRAMS))]),
        err("Unauthorized"),
      );
      await expectErr(
        send(stranger, [await editIx(stranger, "set_uri_base", uriBytes("https://evil.invalid/"))]),
        err("Unauthorized"),
      );
    });

    it("rejects an empty program id and a foreign registry slot", async (t) => {
      if (!owned) return t.skip(ownerSkip);
      const empty = [...PROGRAMS];
      empty[6] = "11111111111111111111111111111111";
      await expectErr(send(authority, [await editIx(authority, "set_programs", programsBytes(empty))]), err("EmptyProgramId"));
      const foreign = [...PROGRAMS];
      foreign[0] = PROGRAMS[1];
      await expectErr(
        send(authority, [await editIx(authority, "set_programs", programsBytes(foreign))]),
        err("WrongRegistryId"),
      );
    });

    it("authority updates the program list and the uri base", async (t) => {
      if (!owned) return t.skip(ownerSkip);
      await send(authority, [await editIx(authority, "set_programs", programsBytes(PROGRAMS))]);
      await send(authority, [await editIx(authority, "set_uri_base", uriBytes("https://example.invalid/nft/"))]);
      const cfg = await data(await configAddress());
      assert.equal(new TextDecoder().decode(cfg.slice(CFG.uri, CFG.uri + 28)), "https://example.invalid/nft/");
    });
  });

  describe("set_rasterizer / set_palette (registry pulls)", () => {
    it("rejects a non-authority", async () => {
      const config = await configAddress();
      await expectErr(send(stranger, [await pullIx(stranger, "set_rasterizer", config)]), err("Unauthorized"));
      await expectErr(send(stranger, [await pullIx(stranger, "set_palette", config)]), err("Unauthorized"));
    });

    it("rejects an account the render program does not own", async (t) => {
      if (!owned) return t.skip(ownerSkip);
      const config = await configAddress();
      await expectErr(send(authority, [await pullIx(authority, "set_rasterizer", config)]), err("WrongBlobOwner"));
      await expectErr(send(authority, [await pullIx(authority, "set_palette", config)]), err("WrongBlobOwner"));
    });

    it("copies version, address and sha256 of a finalized RasterizerCode", async (t) => {
      if (!owned) return t.skip(ownerSkip);
      const cfg = await data(await configAddress());
      assert.equal(bs58(cfg.slice(CFG.programs + 32, CFG.programs + 64)), RENDER_PROGRAM);
      const exists = await maybeData(RENDER_PROGRAM);
      if (!exists) return t.skip("render program is not deployed on this validator");
      const accounts = await rpc.getProgramAccounts(RENDER_PROGRAM, { encoding: "base64", commitment: "confirmed" }).send();
      const finalized = accounts.find((a: any) => {
        const d = Uint8Array.from(getBase64Encoder().encode(a.account.data[0]));
        return d[9] === 1 && d[10] === 1; // kind rasterizer, finalized
      });
      if (!finalized) return t.skip("no finalized RasterizerCode on this validator (upload one first)");
      const blob = Uint8Array.from(getBase64Encoder().encode(finalized.account.data[0]));
      await send(authority, [await pullIx(authority, "set_rasterizer", finalized.pubkey)]);
      const after = await data(await configAddress());
      assert.equal(after[CFG.rasterizerVersion] | (after[CFG.rasterizerVersion + 1] << 8), blob[16] | (blob[17] << 8));
      assert.equal(bs58(after.slice(CFG.rasterizer, CFG.rasterizer + 32)), finalized.pubkey);
      assert.deepEqual(Array.from(after.slice(CFG.sha, CFG.sha + 32)), Array.from(blob.slice(84, 116)));
    });
  });

  describe("tables", () => {
    it("init_table creates every table; a second init is rejected", async (t) => {
      if (!owned) return t.skip(ownerSkip);
      for (const kind of Object.values(TABLE)) {
        const addr = await tableAddress(kind);
        if (!(await maybeData(addr))) await send(authority, [await initTableIx(authority, kind)]);
        const d = await data(addr);
        assert.equal(d[8], 1, "version");
        assert.equal(d[9], kind, "kind");
      }
      await assert.rejects(send(authority, [await initTableIx(authority, TABLE.Tileset)]));
    });

    it("init_table and write_table reject a non-authority", async () => {
      // The tables exist by now, and Anchor runs `init` before the authority constraint, so
      // the exact code here is the system program's "already in use"; any rejection will do.
      await assert.rejects(send(stranger, [await initTableIx(stranger, TABLE.Prefabs)]));
      await expectErr(
        send(stranger, [await writeTableIx(stranger, TABLE.Bestiary, 0, monster())]),
        err("Unauthorized"),
      );
    });

    it("write_table stores a valid record at its index and can clear it", async (t) => {
      if (!owned) return t.skip(ownerSkip);
      // The last slot is unused by the P0 roster (ids 0..4); clear it afterwards.
      const slot = BESTIARY_MAX - 1;
      const at = slot * MONSTER_SIZE;
      await send(authority, [await writeTableIx(authority, TABLE.Bestiary, at, monster())]);
      const d = await data(await tableAddress(TABLE.Bestiary));
      assert.deepEqual(Array.from(d.slice(TABLE_BODY + at, TABLE_BODY + at + MONSTER_SIZE)), Array.from(monster()));
      await send(authority, [await writeTableIx(authority, TABLE.Bestiary, at, new Uint8Array(MONSTER_SIZE))]);
      const cleared = await data(await tableAddress(TABLE.Bestiary));
      assert.ok(cleared.slice(TABLE_BODY + at, TABLE_BODY + at + MONSTER_SIZE).every((b) => b === 0));
    });

    it("write_table rejects out-of-range, misaligned and invalid writes", async (t) => {
      if (!owned) return t.skip(ownerSkip);
      const slot = BESTIARY_MAX - 1;
      await expectErr(
        send(authority, [await writeTableIx(authority, TABLE.Bestiary, BESTIARY_MAX * MONSTER_SIZE, monster())]),
        err("TableRangeOutOfBounds"),
      );
      await expectErr(send(authority, [await writeTableIx(authority, TABLE.Bestiary, 0, [])]), err("TableRangeOutOfBounds"));
      await expectErr(
        send(authority, [await writeTableIx(authority, TABLE.Bestiary, 1, monster())]),
        err("TableMisaligned"),
      );
      await expectErr(
        send(authority, [await writeTableIx(authority, TABLE.Bestiary, 0, monster().slice(0, 47))]),
        err("TableMisaligned"),
      );
      await expectErr(
        send(authority, [await writeTableIx(authority, TABLE.Bestiary, slot * MONSTER_SIZE, monster({ 5: 0 }))]),
        err("InvalidRecord"),
      );
      // Aspect record 1 written at index 0.
      await expectErr(
        send(authority, [await writeTableIx(authority, TABLE.Aspects, 0, [1, 0, 0, 0, 0, 0, 0, 0])]),
        err("InvalidRecord"),
      );
    });
  });
});
