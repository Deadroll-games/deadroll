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
  lamports,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
} from "@solana/kit";
import { PROGRAMS as SDK_PROGRAMS, errorCode } from "@deadroll/sdk";
import {
  findConfigPda,
  findTablePda,
  getGameConfigDataDecoder,
  getGameConfigDecoder,
  getInitConfigInstruction,
  getInitTableInstruction,
  getResizeConfigInstruction,
  getSetPaletteInstruction,
  getSetProgramsInstruction,
  getSetRasterizerFromRenderInstruction,
  getSetRasterizerInstruction,
  getSetSpriteBooksInstruction,
  getSetUriBaseInstruction,
  getWriteTableInstruction,
  type TableKind,
} from "@deadroll/sdk/registry";
import { getBlobHeaderDecoder } from "@deadroll/sdk/render";

// Program ids in `ProgramIdx` order (Anchor.toml, agents/CONTRACTS.md 1).
const PROGRAMS: string[] = Object.values(SDK_PROGRAMS);
const RENDER_PROGRAM = SDK_PROGRAMS.render;

const TABLE = { Aspects: 0, Resonances: 1, Bestiary: 2, Tileset: 3, Prefabs: 4, Traits: 5, MonsterSkills: 6 };
const TABLE_SEED = ["aspects", "resonances", "bestiary", "tileset", "prefabs", "traits", "monster_skills"];
/** Patch 6: monster skill records (deadroll_skillcheck::monster). */
const MONSTER_SKILL_SIZE = 32;
const MONSTER_SKILLS_MAX = 128;
const MONSTER_SIZE = 48;
const BESTIARY_MAX = 32;
/** Body starts after the discriminator and the 8-byte table header. */
const TABLE_BODY = 16;
/** Patch 7: `GameConfigData` (420) then the registry field `sprite_books: [Pubkey; 4]`. */
const CONFIG_LEN = 8 + 420 + 4 * 32;
const EMPTY = "11111111111111111111111111111111";

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
  const code = errorCode("registry", name);
  assert.ok(code !== null, `no error ${name} in the IDL`);
  return code;
}

const u16 = (v: number) => [v & 0xff, (v >> 8) & 0xff];
/** The edit instructions with their args, as the tests send them. */
type Edit = { name: "set_programs"; programs: string[] } | { name: "set_uri_base"; uri: string };
const uriBytes = (s: string) => {
  const out = new Uint8Array(64);
  out.set(new TextEncoder().encode(s));
  return out;
};
const config = (d: Uint8Array) => getGameConfigDecoder().decode(d);
const blobHeader = (d: Uint8Array) => getBlobHeaderDecoder().decode(d, 8);

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

const configAddress = async () => (await findConfigPda())[0];
const tableAddress = async (kind: number) => (await findTablePda({ seed: TABLE_SEED[kind]! }))[0];

const initConfigIx = async (authority: Signer, programs: string[], uri: string) =>
  getInitConfigInstruction({ authority, config: await configAddress(), programs: programs.map((p) => address(p)), uriBase: uriBytes(uri) });

const editIx = async (authority: Signer, edit: Edit) => {
  const accounts = { authority, config: await configAddress() };
  return edit.name === "set_programs"
    ? getSetProgramsInstruction({ ...accounts, programs: edit.programs.map((p) => address(p)) })
    : getSetUriBaseInstruction({ ...accounts, uriBase: uriBytes(edit.uri) });
};

const resizeIx = async (authority: Signer) => getResizeConfigInstruction({ authority, config: await configAddress() });

/** set_sprite_books: the books are remaining accounts after the EditConfig pair. */
const booksIx = async (authority: Signer, books: string[]) => {
  const ix = getSetSpriteBooksInstruction({ authority, config: await configAddress() });
  return { ...ix, accounts: [...ix.accounts, ...books.map((b) => ({ address: address(b), role: AccountRole.READONLY }))] };
};

/** set_rasterizer_from_render with any signer in the render authority slot. */
const fromRenderIx = async (signer: Signer, blob: string) =>
  getSetRasterizerFromRenderInstruction({ renderAuthority: signer, config: await configAddress(), blob: address(blob) });

/** Finalized render blobs of a kind on this validator (1 rasterizer, 2 palette, 3 sprite book). */
async function finalizedBlobs(kind: number): Promise<{ pubkey: string; data: Uint8Array }[]> {
  const accounts = await rpc.getProgramAccounts(RENDER_PROGRAM, { encoding: "base64", commitment: "confirmed" }).send();
  return accounts
    .map((a: any) => ({ pubkey: a.pubkey as string, data: Uint8Array.from(getBase64Encoder().encode(a.account.data[0])) }))
    .filter((a: { data: Uint8Array }) => blobHeader(a.data).kind === kind && blobHeader(a.data).finalized === 1);
}

const pullIx = async (authority: Signer, name: "set_rasterizer" | "set_palette", blob: string) => {
  const accounts = { authority, config: await configAddress(), blob: address(blob) };
  return name === "set_rasterizer" ? getSetRasterizerInstruction(accounts) : getSetPaletteInstruction(accounts);
};

const initTableIx = async (authority: Signer, kind: number) =>
  getInitTableInstruction({ authority, config: await configAddress(), table: await tableAddress(kind), kind: kind as TableKind });

const writeTableIx = async (authority: Signer, kind: number, offset: number, bytes: Uint8Array | number[]) =>
  getWriteTableInstruction({
    authority,
    config: await configAddress(),
    table: await tableAddress(kind),
    kind: kind as TableKind,
    offset,
    bytes: Uint8Array.from(bytes),
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
      owned = getGameConfigDataDecoder().decode(cfg, 8).authority === authority.address;
    }
  });

  describe("init_config", () => {
    it("stores the authority and all 7 program ids", async () => {
      const cfg = await data(await configAddress());
      assert.equal(cfg[8], 1, "version first after the discriminator");
      const stored = config(cfg).core.programs;
      for (let i = 0; i < PROGRAMS.length; i++) assert.equal(stored[i], PROGRAMS[i], `programs[${i}]`);
      assert.equal(cfg.length, CONFIG_LEN, "discriminator + GameConfigData::SIZE + sprite_books");
    });

    it("rejects a second init", async () => {
      await assert.rejects(send(authority, [await initConfigIx(authority, PROGRAMS, "")]));
    });
  });

  describe("set_programs / set_uri_base", () => {
    it("rejects a non-authority", async () => {
      await expectErr(
        send(stranger, [await editIx(stranger, { name: "set_programs", programs: PROGRAMS })]),
        err("Unauthorized"),
      );
      await expectErr(
        send(stranger, [await editIx(stranger, { name: "set_uri_base", uri: "https://evil.invalid/" })]),
        err("Unauthorized"),
      );
    });

    it("rejects an empty program id and a foreign registry slot", async (t) => {
      if (!owned) return t.skip(ownerSkip);
      const empty = [...PROGRAMS];
      empty[6] = "11111111111111111111111111111111";
      await expectErr(send(authority, [await editIx(authority, { name: "set_programs", programs: empty })]), err("EmptyProgramId"));
      const foreign = [...PROGRAMS];
      foreign[0] = PROGRAMS[1];
      await expectErr(
        send(authority, [await editIx(authority, { name: "set_programs", programs: foreign })]),
        err("WrongRegistryId"),
      );
    });

    it("authority updates the program list and the uri base", async (t) => {
      if (!owned) return t.skip(ownerSkip);
      await send(authority, [await editIx(authority, { name: "set_programs", programs: PROGRAMS })]);
      await send(authority, [await editIx(authority, { name: "set_uri_base", uri: "https://example.invalid/nft/" })]);
      const cfg = await data(await configAddress());
      assert.equal(new TextDecoder().decode(Uint8Array.from(config(cfg).core.uriBase).slice(0, 28)), "https://example.invalid/nft/");
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
      assert.equal(config(cfg).core.programs[1], RENDER_PROGRAM);
      const exists = await maybeData(RENDER_PROGRAM);
      if (!exists) return t.skip("render program is not deployed on this validator");
      const accounts = await rpc.getProgramAccounts(RENDER_PROGRAM, { encoding: "base64", commitment: "confirmed" }).send();
      const finalized = accounts.find((a: any) => {
        const d = Uint8Array.from(getBase64Encoder().encode(a.account.data[0]));
        return blobHeader(d).kind === 1 && blobHeader(d).finalized === 1; // kind rasterizer, finalized
      });
      if (!finalized) return t.skip("no finalized RasterizerCode on this validator (upload one first)");
      const blob = Uint8Array.from(getBase64Encoder().encode(finalized.account.data[0]));
      const original: string = config(cfg).core.rasterizer;
      await send(authority, [await pullIx(authority, "set_rasterizer", finalized.pubkey)]);
      const after = config(await data(await configAddress())).core;
      assert.equal(after.rasterizerVersion, blobHeader(blob).id);
      assert.equal(after.rasterizer, finalized.pubkey);
      assert.deepEqual(Array.from(after.rasterizerSha256), Array.from(blobHeader(blob).sha256));
      // Put the deployed rasterizer back (the website reads this pointer).
      if (original !== EMPTY) await send(authority, [await pullIx(authority, "set_rasterizer", original)]);
    });
  });

  describe("patch 7: resize_config, set_sprite_books, set_rasterizer_from_render", () => {
    it("resize_config is refused once GameConfig has its current size", async () => {
      assert.equal((await data(await configAddress())).length, CONFIG_LEN);
      await expectErr(send(authority, [await resizeIx(authority)]), err("ConfigAlreadyResized"));
      await expectErr(send(stranger, [await resizeIx(stranger)]), err("ConfigAlreadyResized"));
    });

    it("set_rasterizer_from_render accepts only the render authority PDA", async () => {
      const config = await configAddress();
      await expectErr(send(stranger, [await fromRenderIx(stranger, config)]), err("UntrustedCaller"));
      // Not even the game authority: only render's finalize_rasterizer (CPI) may call it.
      await expectErr(send(authority, [await fromRenderIx(authority, config)]), err("UntrustedCaller"));
    });

    it("set_sprite_books rejects a non-authority, no books, duplicates and wrong accounts", async (t) => {
      const config = await configAddress();
      await expectErr(send(stranger, [await booksIx(stranger, [config])]), err("Unauthorized"));
      if (!owned) return t.skip(ownerSkip);
      await expectErr(send(authority, [await booksIx(authority, [])]), err("SpriteBookCount"));
      const five = await Promise.all([0, 1, 2, 3, 4].map(async () => (await generateKeyPairSigner()).address));
      await expectErr(send(authority, [await booksIx(authority, five)]), err("SpriteBookCount"));
      await expectErr(send(authority, [await booksIx(authority, [config])]), err("WrongBlobOwner"));
      if (!(await maybeData(RENDER_PROGRAM))) return t.skip("render program is not deployed on this validator");
      const [palette] = await finalizedBlobs(2);
      if (palette) await expectErr(send(authority, [await booksIx(authority, [palette.pubkey])]), err("WrongBlobKind"));
      const [book] = await finalizedBlobs(3);
      if (!book) return t.skip("no finalized SpriteBook on this validator (upload one first)");
      await expectErr(send(authority, [await booksIx(authority, [book.pubkey, book.pubkey])]), err("DuplicateSpriteBook"));
    });

    it("set_sprite_books stores the books in order and clears the other slots", async (t) => {
      if (!owned) return t.skip(ownerSkip);
      if (!(await maybeData(RENDER_PROGRAM))) return t.skip("render program is not deployed on this validator");
      const books = await finalizedBlobs(3);
      if (books.length < 2) return t.skip("needs two finalized SpriteBooks on this validator");
      const cfg = await data(await configAddress());
      const slots = (d: Uint8Array): string[] => config(d).spriteBooks;
      const original = slots(cfg).filter((b) => b !== EMPTY);
      const pick = [books[1]!.pubkey, books[0]!.pubkey];
      await send(authority, [await booksIx(authority, pick)]);
      assert.deepEqual(slots(await data(await configAddress())), [...pick, EMPTY, EMPTY]);
      // Put the deployed list back.
      if (original.length > 0) await send(authority, [await booksIx(authority, original)]);
      assert.deepEqual(slots(await data(await configAddress())).filter((b) => b !== EMPTY), original);
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
      // Patch 6: a skill pool longer than a spawn's 16-bit mask.
      await expectErr(
        send(authority, [await writeTableIx(authority, TABLE.Bestiary, slot * MONSTER_SIZE, monster({ 40: 17 }))]),
        err("InvalidRecord"),
      );
    });

    it("patch 6: monster skills pass the monster builder rules (Heal only as an action or a reaction)", async (t) => {
      if (!owned) return t.skip(ownerSkip);
      const shared = JSON.parse(readFileSync(new URL("../tools/scripts/lib/__fixtures__/registry-tables.json", import.meta.url), "utf8"));
      const record = (i: number) => Uint8Array.from(Buffer.from(shared.monster_skills.slice(i * 64, i * 64 + 64), "hex"));
      // The last slot is past the shipped skills; clear it afterwards.
      const at = (MONSTER_SKILLS_MAX - 1) * MONSTER_SKILL_SIZE;
      const lunge = record(0);
      await send(authority, [await writeTableIx(authority, TABLE.MonsterSkills, at, lunge)]);
      const d = await data(await tableAddress(TABLE.MonsterSkills));
      assert.deepEqual(Array.from(d.slice(TABLE_BODY + at, TABLE_BODY + at + MONSTER_SKILL_SIZE)), Array.from(lunge));
      await send(authority, [await writeTableIx(authority, TABLE.MonsterSkills, at, new Uint8Array(MONSTER_SKILL_SIZE))]);
      // Mending (index 12) as a passive skill: a heal is never an aura or a passive.
      const passiveHeal = record(12);
      passiveHeal[1] = 2;
      await expectErr(send(authority, [await writeTableIx(authority, TABLE.MonsterSkills, at, passiveHeal)]), err("InvalidRecord"));
      // An unknown flag bit, a reserved byte.
      const flag = record(0);
      flag[16] = 2;
      await expectErr(send(authority, [await writeTableIx(authority, TABLE.MonsterSkills, at, flag)]), err("InvalidRecord"));
      const reserved = record(0);
      reserved[31] = 1;
      await expectErr(send(authority, [await writeTableIx(authority, TABLE.MonsterSkills, at, reserved)]), err("InvalidRecord"));
    });
  });
});
