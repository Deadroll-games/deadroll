// Character program tests (lane L5) against the shared local validator.
//
//   anchor deploy -p deadroll_character --provider.cluster localnet
//   anchor test --skip-local-validator --skip-deploy  (runs this file through the tests package)
//
// Shared-validator rules: fresh random keypairs for everything; singletons (GameConfig) are read
// if they exist, never created or assumed. What cannot run yet is skipped with the reason:
// - without GameConfig (registry, L4) no character can be created;
// - the session-only happy paths (lock, rewards, death, unlock, then freeze_tombstone) need the
//   session program (L7) to sign through CPI. Their rejections run here.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { AnchorProvider, Program, setProvider, type Idl } from "@anchor-lang/core";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { newSkill } from "./skill_ext.ts";

const MPL_CORE = new PublicKey("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");
const REGISTRY = new PublicKey("dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG");
const idl = JSON.parse(
  readFileSync(new URL("../target/idl/deadroll_character.json", import.meta.url), "utf8"),
) as Idl;

const provider = AnchorProvider.env();
setProvider(provider);
const program = new Program(idl, provider);
const connection = provider.connection;
const ID = program.programId;
const CONFIG = PublicKey.findProgramAddressSync([Buffer.from("config")], REGISTRY)[0];

// ---------- GameConfig (read only) ----------

/** `uri_base` of the registry GameConfig, or null if registry has not initialized it. */
async function readConfig(): Promise<{ uriBase: string } | null> {
  const acc = await connection.getAccountInfo(CONFIG);
  if (!acc || !acc.owner.equals(REGISTRY)) return null;
  // discriminator 8, version 1, authority 32, programs 7*32, rasterizer_version 2,
  // rasterizer 32, sha256 32, palette 32, then uri_base 64 (CONTRACTS.md, section 4).
  const at = 8 + 1 + 32 + 7 * 32 + 2 + 32 + 32 + 32;
  const raw = acc.data.subarray(at, at + 64);
  const end = raw.indexOf(0);
  return { uriBase: raw.subarray(0, end < 0 ? 64 : end).toString("ascii").replace(/\/+$/, "") };
}
const config = await readConfig();
const NEEDS_CONFIG = config ? false : "GameConfig not initialized yet (registry, L4)";
const NEEDS_SESSION = "needs the session program (L7) to sign through CPI";

// ---------- data ----------

const e = (name: string) => ({ [name]: {} });
const text = (s: string, len: number) => {
  const out = new Array(len).fill(0);
  Buffer.from(s, "ascii").forEach((b, i) => (out[i] = b));
  return out;
};

type Blocks = ReturnType<typeof blocks>;
function blocks(
  category: string,
  purpose: string,
  source: string,
  form: string,
  aspects: string[],
  opts: { effect?: string; trigger?: string; line?: boolean; aura?: number } = {},
) {
  return {
    category: e(category),
    purpose: e(purpose),
    source: e(source),
    form: e(form),
    target: e(opts.line ? "line" : "single"),
    aspects: [...aspects, "fire", "fire"].slice(0, 3).map(e),
    aspectCount: aspects.length,
    effect: e(opts.effect ?? "none"),
    trigger: e(opts.trigger ?? "none"),
    auraRadius: opts.aura ?? 0,
  };
}

// Same sets as `deadroll_skillcheck::presets`.
const WARRIOR_SKILLS: [string, Blocks][] = [
  ["Piercing Thrust", blocks("active", "damage", "combat", "strike", ["piercing"])],
  ["Shield Bash", blocks("active", "damage", "combat", "strike", ["blunt"], { effect: "stun" })],
  ["Javelin", blocks("active", "damage", "combat", "projectile", ["piercing"])],
  ["Bleeding Cleave", blocks("active", "damage", "combat", "strike", ["blood", "piercing"], { line: true })],
  ["Riposte", blocks("reactive", "damage", "combat", "strike", ["piercing"], { trigger: "hit" })],
  ["Iron Skin", blocks("passive", "buff", "combat", "strike", ["blunt"], { effect: "protection" })],
];
const MAGE_SKILLS: [string, Blocks][] = [
  ["Frost Bolt", blocks("active", "damage", "magical", "projectile", ["cold"])],
  ["Frost Burn", blocks("active", "damage", "magical", "projectile", ["cold", "fire"])],
  ["Ice Prison", blocks("active", "damage", "magical", "projectile", ["cold"], { effect: "freeze" })],
  ["Arc", blocks("active", "damage", "magical", "strike", ["lightning"], { line: true })],
  ["Cold Snap", blocks("reactive", "debuff", "magical", "strike", ["cold"], { effect: "slow", trigger: "enemyAdjacent" })],
  ["Cold Mind", blocks("passive", "buff", "magical", "projectile", ["cold"], { effect: "empowerment" })],
];
// Deep copies: tests mutate blocks, the preset tables must stay intact. The patch 5 ext
// (range, timing, HP threshold) matches the Rust presets (./skill_ext.ts).
const skills = (set: [string, Blocks][]) => set.map(([name, b]) => newSkill(name, b, text));

const layer = { spriteId: 1, materialRamp: 2, accentRamp: 3, elementRamp: 0 };

/** Demo character 1: human warrior, Piercing predisposition. */
const warrior = () => ({
  name: text("Bran", 32),
  race: e("human"),
  class: e("warrior"),
  predisposition: e("piercing"),
  stats: [10, 7, 5, 8],
  appearance: new Array(20).fill(layer),
  skills: skills(WARRIOR_SKILLS),
});
/** Demo character 2: forest elf mage, Cold predisposition. */
const mage = () => ({
  name: text("Ilyra", 32),
  race: e("forestElf"),
  class: e("mage"),
  predisposition: e("cold"),
  stats: [5, 7, 13, 5],
  appearance: new Array(20).fill({ spriteId: 0, materialRamp: 0, accentRamp: 0, elementRamp: 0 }),
  skills: skills(MAGE_SKILLS),
});

// ---------- helpers ----------

const pda = (seeds: (Buffer | Uint8Array)[]) => PublicKey.findProgramAddressSync(seeds, ID)[0];
const profileOf = (owner: PublicKey) => pda([Buffer.from("profile"), owner.toBuffer()]);
const characterOf = (owner: PublicKey, index: number) =>
  pda([Buffer.from("character"), owner.toBuffer(), Buffer.from([index])]);
const skillsOf = (c: PublicKey) => pda([Buffer.from("skills"), c.toBuffer()]);
const bagOf = (c: PublicKey) => pda([Buffer.from("bag"), c.toBuffer()]);
const extrasOf = (c: PublicKey) => pda([Buffer.from("skill_extras"), c.toBuffer()]);

async function funded(sol = 5): Promise<Keypair> {
  const kp = Keypair.generate();
  const sig = await connection.requestAirdrop(kp.publicKey, sol * LAMPORTS_PER_SOL);
  const bh = await connection.getLatestBlockhash();
  await connection.confirmTransaction({ signature: sig, ...bh }, "confirmed");
  return kp;
}

/** Awaits `p` and checks it failed with the Anchor error `code` (custom or constraint). */
async function fails(p: Promise<unknown>, code: string) {
  try {
    await p;
  } catch (err: any) {
    const got = err?.error?.errorCode?.code ?? err?.errorCode?.code;
    const logs: string[] = err?.logs ?? err?.transactionLogs ?? [];
    if (got === code || logs.some((l) => l.includes(`Error Code: ${code}.`))) return;
    assert.fail(`expected ${code}, got ${got ?? err}\n${logs.join("\n")}`);
  }
  assert.fail(`expected ${code}, transaction succeeded`);
}

const accounts = program.account as any;
const fetchCharacter = (c: PublicKey) => accounts.character.fetch(c);
const fetchSkills = (c: PublicKey) => accounts.skillSet.fetch(skillsOf(c));
const fetchBag = (c: PublicKey) => accounts.bag.fetch(bagOf(c));
const fetchProfile = (o: PublicKey) => accounts.playerProfile.fetch(profileOf(o));

const createProfile = (owner: Keypair, profile = profileOf(owner.publicKey)) =>
  program.methods
    .createProfile()
    .accountsStrict({ owner: owner.publicKey, profile, systemProgram: SystemProgram.programId })
    .signers([owner])
    .rpc();

async function newPlayer(): Promise<Keypair> {
  const kp = await funded();
  await createProfile(kp);
  return kp;
}

function createIx(
  owner: Keypair,
  index: number,
  args: any,
  opts: { asset?: Keypair | null; config?: PublicKey; mpl?: PublicKey } = {},
) {
  const character = characterOf(owner.publicKey, index);
  const asset = opts.asset === undefined ? Keypair.generate() : opts.asset;
  const call = program.methods
    .createCharacter(args)
    .accountsStrict({
      owner: owner.publicKey,
      profile: profileOf(owner.publicKey),
      character,
      skillSet: skillsOf(character),
      bag: bagOf(character),
      config: opts.config ?? CONFIG,
      asset: asset ? asset.publicKey : null,
      mplCoreProgram: asset ? (opts.mpl ?? MPL_CORE) : null,
      systemProgram: SystemProgram.programId,
      skillExtras: extrasOf(character),
    })
    .signers(asset ? [owner, asset] : [owner]);
  return { call, character, asset };
}

interface Created {
  owner: Keypair;
  character: PublicKey;
  asset: Keypair;
}

async function create(owner: Keypair, args: any): Promise<Created> {
  const index = (await fetchProfile(owner.publicKey)).characterCount;
  const { call, character, asset } = createIx(owner, index, args);
  await call.rpc();
  return { owner, character, asset: asset! };
}

const session = (character: PublicKey, caller: PublicKey) => ({
  character,
  sessionAuthority: caller,
  config: CONFIG,
});

/** mpl-core TransferV1 (discriminator 14, no compression proof); optional accounts = program id. */
async function transferAsset(asset: PublicKey, owner: Keypair, to: PublicKey) {
  const ix = new TransactionInstruction({
    programId: MPL_CORE,
    keys: [
      { pubkey: asset, isSigner: false, isWritable: true },
      { pubkey: MPL_CORE, isSigner: false, isWritable: false },
      { pubkey: owner.publicKey, isSigner: true, isWritable: true },
      { pubkey: owner.publicKey, isSigner: true, isWritable: false },
      { pubkey: to, isSigner: false, isWritable: false },
      { pubkey: MPL_CORE, isSigner: false, isWritable: false },
      { pubkey: MPL_CORE, isSigner: false, isWritable: false },
    ],
    data: Buffer.from([14, 0]),
  });
  await provider.sendAndConfirm(new Transaction().add(ix), [owner]);
}

/** Key, owner, update authority, name and uri of a Core AssetV1, read without its SDK. */
async function readAsset(asset: PublicKey) {
  const acc = await connection.getAccountInfo(asset);
  assert.ok(acc, "asset missing");
  const d = acc.data;
  const nameLen = d.readUInt32LE(66);
  const uriAt = 70 + nameLen;
  return {
    programOwner: acc.owner,
    key: d[0],
    owner: new PublicKey(d.subarray(1, 33)),
    updateAuthorityKind: d[33],
    updateAuthority: new PublicKey(d.subarray(34, 66)),
    name: d.subarray(70, uriAt).toString(),
    uri: d.subarray(uriAt + 4, uriAt + 4 + d.readUInt32LE(uriAt)).toString(),
  };
}

// ---------- tests ----------

describe("create_profile", () => {
  it("creates an empty profile", async () => {
    const p = await newPlayer();
    const prof = await fetchProfile(p.publicKey);
    assert.equal(prof.version, 1);
    assert.ok(prof.owner.equals(p.publicKey));
    assert.equal(prof.characterCount, 0);
  });

  it("rejects a second profile", async () => {
    const p = await newPlayer();
    await assert.rejects(createProfile(p));
  });

  it("rejects a profile at someone else's address", async () => {
    const p = await funded(1);
    await fails(createProfile(p, profileOf(Keypair.generate().publicKey)), "ConstraintSeeds");
  });
});

describe("create_character: argument checks (run before GameConfig is read)", () => {
  const rejects = (name: string, change: (a: any) => void, code: string) =>
    it(`rejects ${name}`, async () => {
      const p = await newPlayer();
      const args = warrior();
      change(args);
      await fails(createIx(p, 0, args).call.rpc(), code);
    });

  rejects("an empty name", (a) => (a.name = new Array(32).fill(0)), "InvalidName");
  rejects("a non-ASCII name", (a) => (a.name[0] = 0xc3), "InvalidName");
  rejects("a name with a leading space", (a) => (a.name = text(" Bran", 32)), "InvalidName");
  rejects("a predisposition of another race", (a) => (a.predisposition = e("cold")), "PredispositionNotAllowed");
  rejects("stats over the budget", (a) => (a.stats = [10, 7, 5, 9]), "InvalidStats");
  rejects("stats under the budget", (a) => (a.stats = [10, 7, 5, 7]), "InvalidStats");
  rejects("a stat over the start cap", (a) => (a.stats = [16, 5, 5, 4]), "InvalidStats");
  rejects("seven skills", (a) => a.skills.push(a.skills[0]), "TooManySkills");
  rejects("a skill of another class", (a) => (a.skills[0] = skills(MAGE_SKILLS)[0]), "InvalidSkill");
  rejects("a skill that breaks the builder", (a) => (a.skills[0].blocks.aspectCount = 0), "InvalidSkill");
  rejects("two reactive skills", (a) => (a.skills[0] = a.skills[4]), "InvalidSkill");
  rejects("an invalid skill name", (a) => (a.skills[2].name = new Array(24).fill(0)), "InvalidName");
  // Patch 5: no Heal in the builder (CHARACTER 8.1), thresholds only on the HP triggers.
  rejects("a heal", (a) => (a.skills[1].blocks.effect = e("heal")), "InvalidSkill");
  rejects("an HP threshold on a hit trigger", (a) => (a.skills[4].ext.hpBelowPct = 40), "InvalidSkill");
  rejects("an HP trigger without a threshold", (a) => (a.skills[4].blocks.trigger = e("hpBelow")), "InvalidSkill");
  rejects("an HP threshold out of range", (a) => {
    a.skills[4].blocks.trigger = e("hpBelow");
    a.skills[4].ext.hpBelowPct = 95;
  }, "InvalidSkill");
  rejects("a passive debuff without an aura", (a) => {
    a.skills[5].blocks.purpose = e("debuff");
    a.skills[5].blocks.effect = e("slow");
  }, "InvalidSkill");

  it("rejects a mage predisposition on an elf from the human list", async () => {
    const p = await newPlayer();
    const args = mage();
    args.predisposition = e("piercing");
    await fails(createIx(p, 0, args).call.rpc(), "PredispositionNotAllowed");
  });

  it("rejects a fake mpl-core program", async () => {
    const p = await newPlayer();
    await fails(createIx(p, 0, warrior(), { mpl: SystemProgram.programId }).call.rpc(), "ConstraintAddress");
  });

  it("rejects a wallet without a profile", async () => {
    const stranger = await funded();
    await fails(createIx(stranger, 0, warrior()).call.rpc(), "AccountNotInitialized");
  });

  it("rejects a config account not owned by registry", async () => {
    const p = await newPlayer();
    const fake = await funded(1); // a system-owned account
    await fails(createIx(p, 0, warrior(), { config: fake.publicKey }).call.rpc(), "InvalidConfig");
  });

  it("rejects while GameConfig does not exist", { skip: config ? "GameConfig exists" : false }, async () => {
    const p = await newPlayer();
    await fails(createIx(p, 0, warrior()).call.rpc(), "InvalidConfig");
  });
});

describe("create_character", { skip: NEEDS_CONFIG }, () => {
  it("creates the demo warrior with a Core NFT", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    const ch = await fetchCharacter(c.character);
    const core = ch.core;
    assert.equal(core.version, 3); // 2: the gear follows the core; 3: then the class trait tier
    assert.ok(core.owner.equals(p.publicKey));
    assert.equal(core.index, 0);
    assert.ok(core.nft.equals(c.asset.publicKey));
    assert.deepEqual([core.race, core.class, core.predisposition], [e("human"), e("warrior"), e("piercing")]);
    assert.deepEqual([core.level, core.xp, core.predispositionTier], [1, 0, 14]);
    assert.deepEqual(core.stats, [10, 7, 5, 8]);
    assert.equal(core.hpMax, 30 + 8 * 2); // class base + Endurance x 2 (BALANCE placeholders)
    assert.equal(core.alive, true);
    assert.equal(core.inSession, null);
    assert.equal(core.stateVersion, 0);
    assert.deepEqual(core.appearance[0], layer);
    assert.equal(Buffer.from(ch.name).toString().replace(/\0+$/, ""), "Bran");
    assert.deepEqual([ch.unspentPoints, ch.nftFrozen], [0, false]);
    // Race and class trait tiers are separate and both start at the worst tier (CHARACTER 6.3).
    assert.equal(ch.classTier, 14);
    // The class starter items come with the character, worn (docs/CHARACTER.md 9, step 5).
    assert.equal(ch.gear.itemCount, 2);
    assert.deepEqual(ch.gear.items.slice(0, 2).map((r: any) => Object.keys(r.base)[0]), ["sword", "leatherArmor"]);
    assert.ok(ch.gear.equipped[0].source.soulbound !== undefined && ch.gear.equipped[3].source.soulbound !== undefined);
    assert.equal((await fetchProfile(p.publicKey)).characterCount, 1);

    const set = await fetchSkills(c.character);
    assert.equal(set.count, 6);
    assert.ok(set.character.equals(c.character));
    assert.deepEqual(set.skills[0].sharesBp, [10_000, 0, 0]);
    assert.equal(set.skills[1].cooldown, 2); // Shield Bash: hard control
    assert.equal(set.skills[3].fusion, 1 + 3 * 6 + 5); // Blood + Piercing
    assert.equal(set.skills[3].powerBp, 6_666); // line counterweight
    assert.equal(Buffer.from(set.skills[4].name).toString().replace(/\0+$/, ""), "Riposte");
    assert.equal(set.skills[6].powerBp, 0);

    const bag = await fetchBag(c.character);
    assert.ok(bag.character.equals(c.character));
    assert.equal(bag.itemCount, 0);
    assert.deepEqual(bag.resources, new Array(8).fill(0));

    const asset = await readAsset(c.asset.publicKey);
    assert.ok(asset.programOwner.equals(MPL_CORE));
    assert.equal(asset.key, 1); // AssetV1
    assert.ok(asset.owner.equals(p.publicKey));
    assert.equal(asset.updateAuthorityKind, 1); // Address
    assert.ok(asset.updateAuthority.equals(c.character));
    assert.equal(asset.name, "Bran");
    assert.equal(asset.uri, `${config!.uriBase}/char/${c.character.toBase58()}/v0.json`);
  });

  it("creates the demo mage as the second character", async () => {
    const p = await newPlayer();
    await create(p, warrior());
    const c = await create(p, mage());
    const core = (await fetchCharacter(c.character)).core;
    assert.equal(core.index, 1);
    assert.deepEqual([core.race, core.predisposition], [e("forestElf"), e("cold")]);
    const set = await fetchSkills(c.character);
    assert.equal(set.skills[1].fusion, 1 + 1 * 6 + 0); // Cold + Fire
    assert.equal(set.skills[2].powerBp, 8_000); // effect counterweight
  });

  it("accepts fewer skills", async () => {
    const p = await newPlayer();
    const args = mage();
    args.skills = args.skills.slice(0, 2);
    const c = await create(p, args);
    assert.equal((await fetchSkills(c.character)).count, 2);
  });

  it("the NFT of a living character transfers like any Core asset", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    const to = Keypair.generate().publicKey;
    await transferAsset(c.asset.publicKey, p, to);
    assert.ok((await readAsset(c.asset.publicKey)).owner.equals(to));
  });

  it("rejects without NFT accounts", async () => {
    const p = await newPlayer();
    await fails(createIx(p, 0, warrior(), { asset: null }).call.rpc(), "NftAccountsMissing");
  });

  it("rejects a reused index", async () => {
    const p = await newPlayer();
    await create(p, warrior());
    await fails(createIx(p, 0, mage()).call.rpc(), "ConstraintSeeds");
  });
});

/** `spend_stat_points`: the owner's main wallet, all points at once (`Stat` order). */
function spend(signer: Keypair, character: PublicKey, points: number[]) {
  return program.methods
    .spendStatPoints(points)
    .accountsStrict({ owner: signer.publicKey, character })
    .signers([signer])
    .rpc();
}

// The happy path, death and the party lock need a character that gained a level: session.test.ts.
describe("spend_stat_points: rejections", { skip: NEEDS_CONFIG }, () => {
  it("rejects choosing no points", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    await fails(spend(p, c.character, [0, 0, 0, 0]), "NoPointsSpent");
  });

  it("rejects more points than unspent (a new character has none)", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    await fails(spend(p, c.character, [1, 0, 0, 0]), "NotEnoughStatPoints");
    const ch = await fetchCharacter(c.character);
    assert.deepEqual([ch.core.stats, ch.unspentPoints], [[10, 7, 5, 8], 0]);
  });

  it("rejects anyone but the owner's main wallet, a session key included", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    const stranger = await newPlayer();
    await fails(spend(stranger, c.character, [1, 0, 0, 0]), "NotOwner");
    // There is no session token path: a game key is just another signer.
    const key = await newPlayer();
    await fails(spend(key, c.character, [1, 0, 0, 0]), "NotOwner");
  });

  it("rejects an account that is not a character", async () => {
    const p = await newPlayer();
    const profile = PublicKey.findProgramAddressSync([Buffer.from("profile"), p.publicKey.toBuffer()], ID)[0];
    await fails(spend(p, profile, [1, 0, 0, 0]), "AccountDiscriminatorMismatch");
  });
});

describe("session-only instructions: untrusted callers", { skip: NEEDS_CONFIG }, () => {
  it("lock_for_session rejects a wallet signing as the session", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    const call = (caller: Keypair) =>
      program.methods
        .lockForSession(Keypair.generate().publicKey, [0, 1, 2, 3, 4, 5])
        .accountsStrict({ ...session(c.character, caller.publicKey), owner: p.publicKey, skillSet: skillsOf(c.character) })
        .signers(caller === p ? [p] : [caller, p])
        .rpc();
    await fails(call(p), "UntrustedCaller");
    await fails(call(await funded(1)), "UntrustedCaller");
  });

  it("grant_rewards rejects an untrusted caller", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    await fails(
      program.methods
        .grantRewards(100, [1, 0, 0, 0, 0, 0, 0, 0])
        .accountsStrict({ ...session(c.character, p.publicKey), bag: bagOf(c.character) })
        .signers([p])
        .rpc(),
      "UntrustedCaller",
    );
  });

  it("mark_dead rejects the player and strangers", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    for (const caller of [p, await funded(1)]) {
      await fails(
        program.methods
          .markDead()
          .accountsStrict({ ...session(c.character, caller.publicKey), skillSet: skillsOf(c.character), bag: bagOf(c.character) })
          .signers([caller])
          .rpc(),
        "UntrustedCaller",
      );
    }
    assert.equal((await fetchCharacter(c.character)).core.alive, true);
    assert.equal((await fetchSkills(c.character)).skills[0].powerBp, 10_000, "nothing burned");
  });

  it("unlock rejects an untrusted caller", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    await fails(
      program.methods.unlock().accountsStrict(session(c.character, p.publicKey)).signers([p]).rpc(),
      "UntrustedCaller",
    );
  });

  it("session-only calls reject a config not owned by registry", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    const fake = await funded(1);
    await fails(
      program.methods
        .markDead()
        .accountsStrict({ ...session(c.character, p.publicKey), skillSet: skillsOf(c.character), bag: bagOf(c.character), config: fake.publicKey })
        .signers([p])
        .rpc(),
      "InvalidConfig",
    );
  });
});

describe("freeze_tombstone", { skip: NEEDS_CONFIG }, () => {
  const freeze = (c: PublicKey, asset: PublicKey, payer: Keypair, mpl = MPL_CORE) =>
    program.methods
      .freezeTombstone()
      .accountsStrict({ character: c, asset, payer: payer.publicKey, mplCoreProgram: mpl, systemProgram: SystemProgram.programId })
      .signers([payer])
      .rpc();

  it("rejects a living character", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    await fails(freeze(c.character, c.asset.publicKey, await funded(1)), "StillAlive");
  });

  it("rejects a fake mpl-core program", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    await fails(freeze(c.character, c.asset.publicKey, p, SystemProgram.programId), "ConstraintAddress");
  });
});

describe("session flow: lock, rewards, death, unlock, freeze", { skip: NEEDS_SESSION }, () => {
  // Runs through the session program once L7 deploys it (join_session -> lock_for_session,
  // victory -> grant_rewards, HP 0 -> mark_dead, end -> unlock), then freeze_tombstone by a
  // stranger and a failing Core transfer of the frozen tombstone. Covered by L8.2 until then.
  it("locks, levels up, dies, unlocks and freezes the tombstone NFT");
});

// ---------- patch 5: SkillExtras and set_skill_description ----------

/** The raw `SkillExtras` records (zero-copy: 4 bytes and a 200-byte description each). */
async function readExtras(character: PublicKey) {
  const acc = await connection.getAccountInfo(extrasOf(character));
  assert.ok(acc, "SkillExtras missing");
  const d = acc.data;
  const at = (i: number) => 8 + 2 + 32 + i * 204;
  return {
    version: d[8],
    character: new PublicKey(d.subarray(10, 42)),
    records: Array.from({ length: 16 }, (_, i) => {
      const r = d.subarray(at(i), at(i) + 204);
      const desc = r.subarray(4);
      const end = desc.indexOf(0);
      return {
        range: r[0],
        timing: r[1],
        hpBelowPct: r[2],
        apCost: r[3],
        description: desc.subarray(0, end < 0 ? 200 : end).toString("ascii"),
      };
    }),
  };
}

function describeIx(owner: Keypair, character: PublicKey, index: number, description: number[], extras = extrasOf(character)) {
  return program.methods
    .setSkillDescription(index, description)
    .accountsStrict({ owner: owner.publicKey, character, skillSet: skillsOf(character), skillExtras: extras })
    .signers([owner]);
}

describe("SkillExtras (patch 5)", { skip: NEEDS_CONFIG }, () => {
  it("create_character stores each skill's range, timing, HP threshold and AP cost", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    const x = await readExtras(c.character);
    assert.equal(x.version, 1);
    assert.ok(x.character.equals(c.character));
    // Piercing Thrust 2 AP melee, Shield Bash 3 (Stun), Javelin 2 ranged, Bleeding Cleave 3 (line)
    // ranged, Riposte and Iron Skin 0 (not actions).
    assert.deepEqual(
      x.records.slice(0, 6).map((r) => [r.range, r.apCost]),
      [
        [0, 2],
        [0, 3],
        [1, 2],
        [1, 3],
        [0, 0],
        [0, 0],
      ],
    );
    assert.ok(x.records.every((r) => r.timing === 0 && r.hpBelowPct === 0 && r.description === ""));
  });

  it("stores the HP threshold of an HP trigger", async () => {
    const p = await newPlayer();
    const args = warrior();
    args.skills[4].blocks.trigger = e("hpBelow");
    args.skills[4].ext.hpBelowPct = 35;
    const c = await create(p, args);
    assert.equal((await readExtras(c.character)).records[4].hpBelowPct, 35);
  });
});

describe("set_skill_description", { skip: NEEDS_CONFIG }, () => {
  it("writes a description once, then refuses a second one", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    const sig = await describeIx(p, c.character, 2, text("Thrown hard and low.", 200)).rpc({ commitment: "confirmed" });
    const tx = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
    const parser = new (await import("@anchor-lang/core")).EventParser(ID, program.coder);
    const ev = [...parser.parseLogs(tx?.meta?.logMessages ?? [])].find((x) => x.name === "skillDescribed");
    assert.ok(ev, "SkillDescribed");
    assert.equal(ev.data.skillIndex, 2);
    const x = await readExtras(c.character);
    assert.equal(x.records[2].description, "Thrown hard and low.");
    // The full 200 bytes fit.
    await describeIx(p, c.character, 0, text("x".repeat(200), 200)).rpc();
    assert.equal((await readExtras(c.character)).records[0].description.length, 200);
    // Once: like the name, it never changes.
    await fails(describeIx(p, c.character, 2, text("Another text.", 200)).rpc(), "DescriptionAlreadySet");
  });

  it("rejects a stranger, a missing skill, bad text and a foreign extras account", async () => {
    const p = await newPlayer();
    const c = await create(p, warrior());
    const other = await create(await newPlayer(), warrior());
    const stranger = await funded(1);
    await fails(describeIx(stranger, c.character, 0, text("Mine now.", 200)).rpc(), "NotOwner");
    await fails(describeIx(p, c.character, 6, text("No such skill.", 200)).rpc(), "NoSuchSkill");
    await fails(describeIx(p, c.character, 0, new Array(200).fill(0)).rpc(), "InvalidDescription");
    await fails(describeIx(p, c.character, 0, text(" Leading space.", 200)).rpc(), "InvalidDescription");
    const nonAscii = text("Caf", 200);
    nonAscii[3] = 0xc3;
    await fails(describeIx(p, c.character, 0, nonAscii).rpc(), "InvalidDescription");
    await fails(describeIx(p, c.character, 0, text("Wrong account.", 200), extrasOf(other.character)).rpc(), "ConstraintSeeds");
    // Nothing was written.
    assert.ok((await readExtras(c.character)).records.every((r) => r.description === ""));
  });
});
