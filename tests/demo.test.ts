// The demo video script (CLAUDE.md section 8) end to end on the local validator, as transactions.
//
// Needs the whole stack from `tools/scripts/deploy-local.ts` (all 7 programs, session built with
// `mock-vrf`, GameConfig trusting them). Fresh keypairs for the DM and both players; the DM builds
// the map from assets/registry/maps/demo_16x16.json. Under mock-vrf the test plays the VRF oracle:
// it signs `consume_randomness` with the fixed mock identity and picks each seed.
//
// cd tests && ANCHOR_PROVIDER_URL=http://127.0.0.1:8899 ANCHOR_WALLET=~/.config/solana/id.json \
//   node --import tsx --test demo.test.ts
import { after, before, describe, it } from "node:test";
import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import anchor from "@anchor-lang/core";
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  ComputeBudgetProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import { MAP_STRIDE, cellKind, layoutBytes, loadMapPreset, presetPoints, presetSecrets, secretChunks } from "../tools/scripts/lib/demo-map.ts";
import { address as kitAddress, createKeyPairSignerFromBytes } from "@solana/kit";
import type { Commitment, Roster, CellEntry } from "@deadroll/sdk/secrets";

/** `PointKind` as the Anchor client encodes it, by discriminant. */
const POINT_KIND = [{ chest: {} }, { entrance: {} }, { exit: {} }];
/** Arena update: `Facing` as the IDL enum, by discriminant. */
const FACING_KIND = ["n", "ne", "e", "se", "s", "sw", "w", "nw"].map((k) => ({ [k]: {} }));
const modeOf = (s: any) => Object.keys(s.mode)[0];
import { newSkill, partyAccounts } from "./skill_ext.ts";
import { MPL_CORE_PROGRAM, PROGRAMS, SESSION_KEYS_PROGRAM, VRF } from "@deadroll/sdk";

const { AnchorProvider, BN, EventParser, Program } = anchor;
type BN = InstanceType<typeof BN>;

// Everything at "confirmed": with the default "processed" a fresh blockhash or account can be
// unknown to the next simulation ("Blockhash not found", AccountNotInitialized).
const env = AnchorProvider.env();
const provider = new AnchorProvider(new Connection(env.connection.rpcEndpoint, "confirmed"), env.wallet, {
  commitment: "confirmed",
  preflightCommitment: "confirmed",
});
const connection = provider.connection;

const REGISTRY = new PublicKey(PROGRAMS.registry);
const CHARACTER = new PublicKey(PROGRAMS.character);
const MAP = new PublicKey(PROGRAMS.map);
const SESSION = new PublicKey(PROGRAMS.session);
const ITEMS = new PublicKey(PROGRAMS.items);
const SESSION_KEYS = new PublicKey(SESSION_KEYS_PROGRAM);
const MPL_CORE = new PublicKey(MPL_CORE_PROGRAM);
// MagicBlock VRF accounts (ephemeral-vrf-sdk consts); under mock-vrf they only pass address checks.
const VRF_PROGRAM = new PublicKey(VRF.program);
const ORACLE_QUEUE = new PublicKey(VRF.oracleQueue);
const SLOT_HASHES = new PublicKey(VRF.slotHashes);

const PRICE = new BN(LAMPORTS_PER_SOL / 10);
const [N, E, S, W] = [0, 1, 2, 3];
// Patch 13: move_path walks the 8 neighbors; a diagonal is one step.
const [NE, SE, SW, NW] = [4, 5, 6, 7];
const MAX_PATH = 16;
const MONSTER_BASE = 4;
const [WOLF, CHIEFTAIN] = [0, 2];
/** Skill slots of the preset sets (crates/skillcheck presets). */
const PIERCING_THRUST = 0;
/** `act` slot of the weapon's basic attack (no cooldown). */
const BASE_ATTACK = 0xff;
const FROST_BOLT = 0;
const FROST_BURN = 1;
const WARRIOR = 0;
const MAGE = 1;

// ---------------------------------------------------------------------------------------------
// helpers (same conventions as programs/session/tests/session.test.ts)

const pda = (seeds: (Buffer | Uint8Array)[], programId: PublicKey) => PublicKey.findProgramAddressSync(seeds, programId)[0];
const sha256 = (data: Buffer | string) => createHash("sha256").update(data).digest();
const sighash = (preimage: string) => sha256(preimage).subarray(0, 8);

const config = pda([Buffer.from("config")], REGISTRY);
const bestiary = pda([Buffer.from("bestiary")], REGISTRY);
const tileset = pda([Buffer.from("tileset")], REGISTRY);
const aspects = pda([Buffer.from("aspects")], REGISTRY);
const traits = pda([Buffer.from("traits")], REGISTRY);
const resonances = pda([Buffer.from("resonances")], REGISTRY);
// Patch 6: the skills of the bestiary pools (finalize_map checks the choice, open_session copies it).
const monsterSkills = pda([Buffer.from("monster_skills")], REGISTRY);
// Patch 11: reward tables (the preset's chest and boss table).
const rewards = pda([Buffer.from("rewards")], REGISTRY);
const triples = pda([Buffer.from("triples")], REGISTRY);

/** Patch 6: the accounts that finalize the deaths a callback queued (`GameSession.pending_deaths`):
 * each queued slot's Character, SkillSet and Bag, writable, in slot order. */
function deathAccounts(st: any) {
  const out: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] = [];
  for (let slot = 0; slot < st.core.playerCount; slot++) {
    if (!(st.pendingDeaths & (1 << slot))) continue;
    const c: PublicKey = st.core.players[slot];
    for (const pubkey of [c, pda([Buffer.from("skills"), c.toBuffer()], CHARACTER), pda([Buffer.from("bag"), c.toBuffer()], CHARACTER)]) {
      out.push({ pubkey, isSigner: false, isWritable: true });
    }
  }
  return out;
}
/** Compute limit of an instruction that may finalize deaths (`character.mark_dead` each). */
const finalizeBudget = (deaths: unknown[]) => (deaths.length ? [ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })] : []);
const sessionAuthority = pda([Buffer.from("authority")], SESSION);
const MOCK_VRF = Keypair.fromSeed(sha256("deadroll:mock-vrf"));
const vrfAccounts = {
  programIdentity: pda([Buffer.from("identity")], SESSION),
  oracleQueue: ORACLE_QUEUE,
  vrfProgram: VRF_PROGRAM,
  slotHashes: SLOT_HASHES,
  systemProgram: SystemProgram.programId,
};

/** Roll `index` of a VRF value, exactly `deadroll_combat::VrfRoll`: sha256(seed ‖ index u32 LE). */
function rollAt(seed: Buffer, index: number): bigint {
  const i = Buffer.alloc(4);
  i.writeUInt32LE(index);
  return sha256(Buffer.concat([seed, i])).readBigUInt64LE(0);
}
/** First four bytes are `n` little endian. The same n always yields the same VRF value. */
function fixedSeed(n: number): Buffer {
  const seed = Buffer.alloc(32);
  seed.writeUInt32LE(n, 0);
  return seed;
}
/** Natural 20: always a hit and a crit. */
const CRIT = fixedSeed(9);
/** Natural 1: always a miss. */
const MISS = fixedSeed(17);
/**
 * A crit for the mage's Frost Burn (crates/combat hit order, steps 1-3; a natural 20 makes no
 * crit-chance roll): roll 1 is the Cold base's staff tint (Blunt, fails), roll 2 the Fire base's
 * race tint (Cold, a d100 under 5: Pale Flame), roll 3 the Fire base's staff tint (fails, so no
 * Blasting Flame knockback moves the wolf).
 */
const RESONANT = fixedSeed(128);

const signatures: { what: string; sig: string }[] = [];
/** Every VRF callback, read back from the chain right after it landed. */
const publicRolls: { sig: string; ok: boolean }[] = [];
/** Patch 13: the compute units of a move, for the report (read right after it, while the ledger has it). */
async function logCu(what: string, sig: string) {
  const tx = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  const vrf = (tx?.meta?.logMessages ?? []).some((l) => l.includes("invoke [2]"));
  console.log(`${what}: ${tx?.meta?.computeUnitsConsumed ?? "?"} CU${vrf ? " (with a reactive VRF request)" : ""}`);
}

function track(what: string) {
  return (sig: string) => {
    signatures.push({ what, sig });
    return sig;
  };
}

async function funded(sol = 10): Promise<Keypair> {
  const kp = Keypair.generate();
  const sig = await connection.requestAirdrop(kp.publicKey, sol * LAMPORTS_PER_SOL);
  const latest = await connection.getLatestBlockhash();
  await connection.confirmTransaction({ signature: sig, ...latest }, "confirmed");
  return kp;
}

async function isProgram(id: PublicKey): Promise<boolean> {
  return (await connection.getAccountInfo(id))?.executable ?? false;
}

const DELTA: [number, number][] = [
  [0, -1],
  [1, 0],
  [0, 1],
  [-1, 0],
  [1, -1],
  [1, 1],
  [-1, 1],
  [-1, -1],
];

/** The cells of a move_path: the direction `steps` from `start`. */
function cellsOf(start: { x: number; y: number }, steps: number[]) {
  const out: { x: number; y: number }[] = [];
  let at = start;
  for (const d of steps) {
    at = { x: at.x + DELTA[d][0], y: at.y + DELTA[d][1] };
    out.push(at);
  }
  return out;
}

const e = (name: string) => ({ [name]: {} });
const text = (s: string, len: number) => {
  const out = new Array(len).fill(0);
  Buffer.from(s, "ascii").forEach((b, i) => (out[i] = b));
  return out;
};
function blocks(category: string, purpose: string, source: string, form: string, aspects: string[], o: any = {}) {
  return {
    category: e(category),
    purpose: e(purpose),
    source: e(source),
    form: e(form),
    target: e(o.line ? "line" : "single"),
    aspects: [...aspects, "fire", "fire"].slice(0, 3).map(e),
    aspectCount: aspects.length,
    effect: e(o.effect ?? "none"),
    trigger: e(o.trigger ?? "none"),
    auraRadius: 0,
  };
}
const WARRIOR_SKILLS: [string, any][] = [
  ["Piercing Thrust", blocks("active", "damage", "combat", "strike", ["piercing"])],
  ["Shield Bash", blocks("active", "damage", "combat", "strike", ["blunt"], { effect: "stun" })],
  ["Javelin", blocks("active", "damage", "combat", "projectile", ["piercing"])],
  ["Bleeding Cleave", blocks("active", "damage", "combat", "strike", ["blood", "piercing"], { line: true })],
  ["Riposte", blocks("reactive", "damage", "combat", "strike", ["piercing"], { trigger: "hit" })],
  ["Iron Skin", blocks("passive", "buff", "combat", "strike", ["blunt"], { effect: "protection" })],
];
const MAGE_SKILLS: [string, any][] = [
  ["Frost Bolt", blocks("active", "damage", "magical", "projectile", ["cold"])],
  ["Frost Burn", blocks("active", "damage", "magical", "projectile", ["cold", "fire"])],
  ["Ice Prison", blocks("active", "damage", "magical", "projectile", ["cold"], { effect: "freeze" })],
  ["Arc", blocks("active", "damage", "magical", "strike", ["lightning"], { line: true })],
  ["Cold Snap", blocks("reactive", "debuff", "magical", "strike", ["cold"], { effect: "slow", trigger: "enemyAdjacent" })],
  ["Cold Mind", blocks("passive", "buff", "magical", "projectile", ["cold"], { effect: "empowerment" })],
];

const cheb = (a: { x: number; y: number }, b: { x: number; y: number }) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));

/**
 * `rules::roll_initiative`: unit `i` (players by slot, then living monsters by index) rolls d20
 * number `i` of the start_session VRF value; higher `d20 + initiative` first, then higher
 * initiative, then lower unit id.
 */
function initiativeOrder(seed: Buffer, units: [number, number][]): { order: number[]; d20: number[] } {
  const rolled = units.map(([unit, init], i) => ({ unit, init, d20: Number(rollAt(seed, i) % 20n) + 1 }));
  rolled.sort((a, b) => b.d20 + b.init - (a.d20 + a.init) || b.init - a.init || a.unit - b.unit);
  return { order: rolled.map((r) => r.unit), d20: rolled.map((r) => r.d20) };
}

// ---------------------------------------------------------------------------------------------
// what must be on the validator

const IDLS = ["deadroll_session", "deadroll_character", "deadroll_map"].map(
  (n) => new URL(`../target/idl/${n}.json`, import.meta.url),
);
const missing: string[] = [];
for (const idl of IDLS) if (!existsSync(idl)) missing.push(`${idl.pathname.split("/").pop()} (anchor build)`);
for (const [name, id] of [["registry", REGISTRY], ["character", CHARACTER], ["map", MAP], ["session", SESSION], ["items", ITEMS], ["mpl-core", MPL_CORE], ["session-keys", SESSION_KEYS]] as const) {
  if (!(await isProgram(id))) missing.push(`${name} program`);
}
const cfg = await connection.getAccountInfo(config);
if (!cfg?.owner.equals(REGISTRY)) missing.push("registry GameConfig");
else if (!new PublicKey(cfg.data.subarray(41 + 5 * 32, 41 + 6 * 32)).equals(SESSION)) missing.push("GameConfig trusting session");
for (const [name, table] of [
  ["bestiary", bestiary],
  ["tileset", tileset],
  ["aspects", aspects],
  ["traits", traits],
  ["resonances", resonances],
] as const) {
  if (!(await connection.getAccountInfo(table))?.owner.equals(REGISTRY)) missing.push(`registry ${name} table`);
}
const SKIP = missing.length ? `run tools/scripts/deploy-local.ts first; missing: ${missing.join(", ")}` : false;

const load = (i: number) => JSON.parse(readFileSync(IDLS[i], "utf8"));
const session: any = SKIP ? null : new Program(load(0), provider);
const characterProgram: any = SKIP ? null : new Program(load(1), provider);
const mapProgram: any = SKIP ? null : new Program(load(2), provider);
const sessionEvents = SKIP ? null : new EventParser(SESSION, session.coder);
const characterEvents = SKIP ? null : new EventParser(CHARACTER, characterProgram.coder);

async function eventsOf(sig: string): Promise<{ name: string; data: any }[]> {
  const tx = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  const logs = tx?.meta?.logMessages ?? [];
  return [...sessionEvents!.parseLogs(logs), ...characterEvents!.parseLogs(logs)];
}
async function eventOf(sig: string, name: string): Promise<any> {
  const found = (await eventsOf(sig)).find((x) => x.name.toLowerCase() === name.toLowerCase());
  assert.ok(found, `no ${name} event in ${sig}`);
  return found.data;
}

/** Session Keys `create_session_v2`: the wallet lets `signer` act for it in the session program. */
async function issueToken(signer: Keypair, authority: Keypair): Promise<PublicKey> {
  const token = pda(
    [Buffer.from("session_token_v2"), SESSION.toBuffer(), signer.publicKey.toBuffer(), authority.publicKey.toBuffer()],
    SESSION_KEYS,
  );
  const ix = new TransactionInstruction({
    programId: SESSION_KEYS,
    keys: [
      { pubkey: token, isSigner: false, isWritable: true },
      { pubkey: signer.publicKey, isSigner: true, isWritable: true },
      { pubkey: authority.publicKey, isSigner: true, isWritable: true },
      { pubkey: authority.publicKey, isSigner: true, isWritable: false },
      { pubkey: SESSION, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    // top_up: None, valid_until: None, lamports: None
    data: Buffer.concat([sighash("global:create_session_v2"), Buffer.from([0, 0, 0])]),
  });
  track("create_session_v2")(await provider.sendAndConfirm(new Transaction().add(ix), [signer, authority]));
  return token;
}

// ---------------------------------------------------------------------------------------------
// the party

type Kind = "warrior" | "mage";
interface Player {
  kind: Kind;
  wallet: Keypair;
  actor: Keypair;
  token: PublicKey;
  character: PublicKey;
  asset: PublicKey;
  participation: PublicKey;
  rewardCache: PublicKey;
  skillSet: PublicKey;
  bag: PublicKey;
}

const preset = loadMapPreset();

class Demo {
  dm!: Keypair;
  dmActor!: Keypair;
  dmToken!: PublicKey;
  /** Patch C: what the DM committed (its client opens the encrypted copy with the wallet's key). */
  commitment!: Commitment;
  rosters!: Roster[];
  cells!: CellEntry[];
  map!: PublicKey;
  layout!: PublicKey;
  acc!: { session: PublicKey; monsters: PublicKey; escrow: PublicKey };
  players: Player[] = [];

  async createCharacter(owner: Keypair, kind: Kind): Promise<{ character: PublicKey; asset: PublicKey; sig: string }> {
    const profile = pda([Buffer.from("profile"), owner.publicKey.toBuffer()], CHARACTER);
    if (!(await connection.getAccountInfo(profile))) {
      await characterProgram.methods
        .createProfile()
        .accountsStrict({ owner: owner.publicKey, profile, systemProgram: SystemProgram.programId })
        .signers([owner])
        .rpc()
        .then(track("create_profile"));
    }
    const index = (await characterProgram.account.playerProfile.fetch(profile)).characterCount;
    const character = pda([Buffer.from("character"), owner.publicKey.toBuffer(), Buffer.from([index])], CHARACTER);
    const layer = { spriteId: 1, materialRamp: 2, accentRamp: 3, elementRamp: 0 };
    const warrior = kind === "warrior";
    const args = {
      name: text(warrior ? "Bran" : "Ilyra", 32),
      race: e(warrior ? "human" : "forestElf"),
      class: e(kind),
      predisposition: e(warrior ? "piercing" : "cold"),
      stats: warrior ? [10, 7, 5, 8] : [5, 7, 13, 5],
      appearance: new Array(20).fill(layer),
      skills: (warrior ? WARRIOR_SKILLS : MAGE_SKILLS).map(([name, b]) => newSkill(name, b, text)),
    };
    const asset = Keypair.generate();
    const sig = await characterProgram.methods
      .createCharacter(args)
      .accountsStrict({
        owner: owner.publicKey,
        profile,
        character,
        skillSet: pda([Buffer.from("skills"), character.toBuffer()], CHARACTER),
        bag: pda([Buffer.from("bag"), character.toBuffer()], CHARACTER),
        config,
        asset: asset.publicKey,
        mplCoreProgram: MPL_CORE,
        systemProgram: SystemProgram.programId,
        skillExtras: pda([Buffer.from("skill_extras"), character.toBuffer()], CHARACTER),
      })
      .signers([owner, asset])
      .rpc({ commitment: "confirmed" })
      .then(track(`create_character ${kind}`));
    return { character, asset: asset.publicKey, sig };
  }

  async buildMap() {
    const index = randomBytes(4).readUInt32LE(0);
    const idx = Buffer.alloc(4);
    idx.writeUInt32LE(index);
    this.map = pda([Buffer.from("map"), this.dm.publicKey.toBuffer(), idx], MAP);
    this.layout = pda([Buffer.from("layout"), this.map.toBuffer()], MAP);
    const edit = { dm: this.dm.publicKey, map: this.map, layout: this.layout };
    await mapProgram.methods
      .createMap(index, preset.width, preset.height, preset.tier, { minPlayers: preset.minPlayers, chestRadius: preset.chestRadius, settleTimeoutSecs: preset.settleTimeoutSecs, biome: preset.biome })
      .accountsStrict({ ...edit, systemProgram: SystemProgram.programId })
      .signers([this.dm])
      .rpc()
      .then(track("create_map"));
    await mapProgram.methods.writeLayout(0, Buffer.from(layoutBytes(preset))).accountsStrict(edit).signers([this.dm]).rpc().then(track("write_layout"));
    // Patch C: only the visible monsters in the open; the rosters and the secrets under the root,
    // encrypted on chain with the DM's key.
    const { design, commitment, blob } = await presetSecrets(preset, kitAddress(this.map.toBase58()), await createKeyPairSignerFromBytes(this.dm.secretKey));
    this.commitment = commitment;
    this.rosters = design.rosters;
    this.cells = design.cells;
    await mapProgram.methods.writeSpawns(design.spawns).accountsStrict(edit).signers([this.dm]).rpc().then(track("write_spawns"));
    const points = presetPoints(preset).map((p) => ({ ...p, kind: POINT_KIND[p.kind] }));
    await mapProgram.methods.writePoints(points, preset.bossTable).accountsStrict(edit).signers([this.dm]).rpc().then(track("write_points"));
    // Arena update: every pack's monster on the map, its facing and its arena.
    for (const pk of design.packs) {
      const record = { visible: pk.visible, x: pk.x, y: pk.y, facing: FACING_KIND[pk.facing], width: pk.width, height: pk.height };
      await mapProgram.methods.writeArena(pk.pack, record, Buffer.from(pk.cells)).accountsStrict(edit).signers([this.dm]).rpc().then(track("write_arena"));
    }
    for (const c of secretChunks(blob, design)) {
      await mapProgram.methods.writeSecrets(c.offset, Buffer.from(c.bytes), c.len, c.threat, c.declared).accountsStrict(edit).signers([this.dm]).rpc().then(track("write_secrets"));
    }
    await mapProgram.methods
      .finalizeMap(Array.from(commitment.root))
      .accountsStrict({ ...edit, bestiary, monsterSkills, rewards, tileset })
      .signers([this.dm])
      .rpc()
      .then(track("finalize_map"));
    const session = pda([Buffer.from("session"), this.map.toBuffer()], SESSION);
    this.acc = {
      session,
      monsters: pda([Buffer.from("monsters"), session.toBuffer()], SESSION),
      escrow: pda([Buffer.from("escrow"), session.toBuffer()], SESSION),
    };
  }

  playerAccounts(character: PublicKey) {
    const s = this.acc.session;
    return {
      participation: pda([Buffer.from("participation"), s.toBuffer(), character.toBuffer()], SESSION),
      rewardCache: pda([Buffer.from("cache"), character.toBuffer()], SESSION),
      skillSet: pda([Buffer.from("skills"), character.toBuffer()], CHARACTER),
      bag: pda([Buffer.from("bag"), character.toBuffer()], CHARACTER),
    };
  }

  // --- reads
  state(): Promise<any> {
    return session.account.gameSession.fetch(this.acc.session);
  }
  part(who: number): Promise<any> {
    return session.account.participation.fetch(this.players[who].participation);
  }
  async monsters(): Promise<any[]> {
    const m = (await session.account.monsterState.fetch(this.acc.monsters)).core;
    return m.monsters.slice(0, m.count);
  }
  character(who: number): Promise<any> {
    return characterProgram.account.character.fetch(this.players[who].character);
  }

  // --- player moves, all through the session key (no wallet signature)
  async move(who: number, steps: number[]) {
    const pl = this.players[who];
    const from = (await this.state()).playerPos[who];
    return session.methods
      .movePath(who, cellsOf(from, steps))
      .accountsStrict({
        signer: pl.actor.publicKey,
        session: this.acc.session,
        participation: pl.participation,
        monsters: this.acc.monsters,
        layout: this.layout,
        sessionToken: pl.token,
        // Patch 6: stepping next to a monster can trigger its reactive skill (a VRF roll).
        ...vrfAccounts,
      })
      .signers([pl.actor])
      .rpc({ commitment: "confirmed" })
      .then(track(`move ${pl.kind}`))
      .then(async (sig: string) => {
        await this.revealOwed();
        return sig;
      });
  }
  /** Patch C: the DM's client reveals what the party uncovered, with the DM's session key. */
  async revealOwed() {
    for (let i = 0; i < 40; i++) {
      const s = await this.state();
      if (s.core.pendingVrf) return;
      if (s.obligationCount > 0) {
        await this.revealCell(s.obligations[0], this.players[s.obligedBy]?.participation ?? null, true);
        continue;
      }
      if (modeOf(s) === "engaged" && !(s.packsRevealed & (1 << s.arenaPack))) {
        await this.revealPack(s.arenaPack, true);
        continue;
      }
      return;
    }
    throw new Error("reveals never ended");
  }
  async revealCell(leaf: number, participation: PublicKey | null, key: boolean) {
    const r = await this.commitment.cell(leaf);
    const names = ["none", "trap", "ambush", "stash", "door"];
    const signer = key ? this.dmActor : this.dm;
    return session.methods
      .revealCell(leaf, { kind: { [names[r.secret.kind as number]]: {} }, a: r.secret.a, b: r.secret.b, c: r.secret.c } as any, Array.from(r.salt), r.proof.map((h) => Array.from(h)))
      .accountsStrict({ signer: signer.publicKey, session: this.acc.session, layout: this.layout, participation, monsters: this.acc.monsters, rewards, sessionToken: key ? this.dmToken : null })
      .signers([signer])
      .rpc({ commitment: "confirmed" })
      .then(track("reveal_cell"));
  }
  async revealPack(pack: number, key: boolean) {
    const r = await this.commitment.pack(pack);
    const signer = key ? this.dmActor : this.dm;
    return session.methods
      .revealPack(pack, r.roster as any, Array.from(r.salt), r.proof.map((h) => Array.from(h)))
      .accountsStrict({ signer: signer.publicKey, session: this.acc.session, layout: this.layout, monsters: this.acc.monsters, bestiary, monsterSkills, rewards, sessionToken: key ? this.dmToken : null })
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
      .signers([signer])
      .rpc({ commitment: "confirmed" })
      .then(track("reveal_pack"));
  }
  /** Patch C: the DM's final audit: every declared secret and pack not revealed yet (main wallet). */
  async audit() {
    const s = await this.state();
    for (const c of this.cells) if (!(s.revealed[c.leaf >> 3] & (1 << (c.leaf & 7)))) await this.revealCell(c.leaf, null, false);
    for (let n = 0; n < 8; n++) if (this.rosters[n].count && !(s.packsRevealed & (1 << n))) await this.revealPack(n, false);
  }
  act(who: number, slot: number, monster: number) {
    const pl = this.players[who];
    return session.methods
      .act(slot, monster)
      .accountsStrict({
        signer: pl.actor.publicKey,
        session: this.acc.session,
        participation: pl.participation,
        character: pl.character,
        skillSet: pl.skillSet,
        bag: pl.bag,
        monsters: this.acc.monsters,
        sessionToken: pl.token,
        ...vrfAccounts,
        targetParticipation: null,
        layout: this.layout,
      })
      .signers([pl.actor])
      .rpc()
      .then(track(`act ${pl.kind}`));
  }
  endTurn(who: number | null) {
    const pl = who === null ? null : this.players[who];
    const signer = pl ? pl.actor : this.dmActor;
    return session.methods
      .endTurn()
      .accountsStrict({
        signer: signer.publicKey,
        session: this.acc.session,
        monsters: this.acc.monsters,
        participation: pl ? pl.participation : null,
        sessionToken: pl ? pl.token : this.dmToken,
      })
      .signers([signer])
      .rpc()
      .then(track(`end_turn ${pl?.kind ?? "monster"}`));
  }
  /** `start_turn` for whoever's turn it is: permissionless, sent here by the provider wallet. */
  async begin() {
    const s = await this.state();
    const unit = s.core.turnOrder[s.core.turnIndex];
    const pl = unit < MONSTER_BASE ? this.players[unit] : null;
    // Patch 6: the deaths the last callback queued are finalized first.
    const queue = deathAccounts(s);
    return session.methods
      .startTurn()
      .accountsStrict({
        session: this.acc.session,
        monsters: this.acc.monsters,
        participation: pl ? pl.participation : null,
        character: pl ? pl.character : null,
        sessionAuthority,
        config,
        characterProgram: CHARACTER,
        // Burned if over-time damage kills the player.
        skillSet: pl ? pl.skillSet : null,
        bag: pl ? pl.bag : null,
        payer: provider.wallet.publicKey,
        ...vrfAccounts,
      })
      .remainingAccounts(queue)
      .preInstructions(finalizeBudget(queue))
      .rpc({ commitment: "confirmed" })
      .then(track(queue.length ? "start_turn finalizing deaths" : `start_turn ${pl?.kind ?? "monster"}`))
      .then(async (sig: string) => {
        await this.settle();
        return sig;
      });
  }
  /**
   * Patch 5: the oracle answers a reactive-only roll (a monster stepping next to the mage's Cold
   * Snap, over-time damage under a threshold) on the first bearer's accounts.
   */
  async settle(seed: Buffer = MISS) {
    const s = await this.state();
    // Patch 6: players down in the last callback are finalized by start_turn.
    if (!s.core.pendingVrf) return s.pendingDeaths && s.core.status.active ? this.begin() : null;
    // Patch 6: a monster's reactive-only roll rides on the player the request named.
    if (s.pendingMonsterReactive.kind !== 0) {
      return this.consume(
        this.players.findIndex((pl) => pl.participation.equals(s.pending.participation)),
        seed,
      );
    }
    if (s.pendingReactive.kind === 0) return null;
    const bearer = [0, 1, 2, 3].find((i) => s.pendingReactive.bearers & (1 << i))!;
    return this.consume(bearer, seed);
  }
  // --- DM moves through the DM's session key
  async monsterMove(monster: number, steps: number[]) {
    const m = (await this.monsters())[monster];
    return session.methods
      .movePath(MONSTER_BASE + monster, cellsOf(m, steps))
      .accountsStrict({
        signer: this.dmActor.publicKey,
        session: this.acc.session,
        participation: null,
        monsters: this.acc.monsters,
        layout: this.layout,
        sessionToken: this.dmToken,
        ...vrfAccounts,
      })
      .signers([this.dmActor])
      .rpc()
      .then(track("move_path monster"));
  }
  monsterAct(monster: number, target: number) {
    const pl = this.players[target];
    return session.methods
      .monsterAct(monster, 0, target)
      .accountsStrict({
        signer: this.dmActor.publicKey,
        session: this.acc.session,
        monsters: this.acc.monsters,
        participation: pl.participation,
        character: pl.character,
        skillSet: pl.skillSet,
        bag: pl.bag,
        sessionToken: this.dmToken,
        ...vrfAccounts,
        layout: this.layout,
      })
      .signers([this.dmActor])
      .rpc()
      .then(track("monster_act"));
  }
  /** The oracle's callback for the pending action of player `who` (attacker or target). */
  async consume(who: number, seed: Buffer) {
    const pl = this.players[who];
    const remaining = partyAccounts(await this.state(), pl.participation);
    return session.methods
      .consumeRandomness([...seed])
      .accountsStrict({
        vrfProgramIdentity: MOCK_VRF.publicKey,
        session: this.acc.session,
        monsters: this.acc.monsters,
        participation: pl.participation,
        character: pl.character,
        skillSet: pl.skillSet,
        bag: pl.bag,
        sessionAuthority,
        config,
        characterProgram: CHARACTER,
        targetParticipation: null,
        layout: this.layout,
      })
      .remainingAccounts(remaining)
      .signers([MOCK_VRF])
      .rpc({ commitment: "confirmed" })
      .then(track("consume_randomness"))
      .then(async (sig: string) => {
        // Read back at once: the shared validator keeps ~100 slots of transaction history.
        const names = (await eventsOf(sig)).map((x) => x.name);
        publicRolls.push({ sig, ok: ["hitResolved", "initiativeRolled", "reactiveTriggered"].some((n) => names.includes(n)) });
        if ((await this.state()).core.status.active) await this.revealOwed();
        return sig;
      });
  }
  /** One player attack: request, oracle callback, the `HitResolved` event. */
  async attack(who: number, slot: number, monster: number, seed: Buffer) {
    await this.act(who, slot, monster);
    const sig = await this.consume(who, seed);
    return { sig, hit: await eventOf(sig, "hitResolved"), events: await eventsOf(sig) };
  }

  // --- arena update
  /** Each living player's participation, writable, in slot order. */
  async livingParts() {
    const s = await this.state();
    return this.players.flatMap((pl, i) => (s.aliveMask & (1 << i) ? [{ pubkey: pl.participation, isSigner: false, isWritable: true }] : []));
  }
  /** `enter_arena` or `leave_arena`: permissionless, sent here by the provider wallet. */
  async enterArena() {
    await this.revealOwed();
    return session.methods
      .enterArena()
      .accountsStrict({ payer: provider.wallet.publicKey, session: this.acc.session, monsters: this.acc.monsters, layout: this.layout })
      .remainingAccounts(await this.livingParts())
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
      .rpc({ commitment: "confirmed" })
      .then(track("enter_arena"));
  }
  async leaveArena() {
    return session.methods
      .leaveArena()
      .accountsStrict({ payer: provider.wallet.publicKey, session: this.acc.session, monsters: this.acc.monsters })
      .remainingAccounts(await this.livingParts())
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 })])
      .rpc({ commitment: "confirmed" })
      .then(track("leave_arena"));
  }
  /** The first living player walks the map next to pack `pack`'s visible monster (the others end
   * their turns); the party enters its arena and its first turn starts. */
  async engage(pack: number) {
    for (let guard = 0; guard < 100; guard++) {
      const { s, unit } = await this.current();
      if (modeOf(s) === "engaged") {
        await logCu("enter_arena", await this.enterArena());
        await this.begin();
        return;
      }
      if (s.startedSeq !== s.turnSeq) {
        await this.begin();
        continue;
      }
      const walker = this.players.findIndex((_, i) => s.aliveMask & (1 << i));
      if (unit !== walker) {
        await this.endTurn(unit);
        continue;
      }
      const pk = s.packs[pack];
      const steps = await this.route(s.playerPos[unit], (x, y) => cheb({ x, y }, pk) === 1, unit, pack);
      assert.ok(steps, `a path to pack ${pack}`);
      await this.walk(unit, null, steps, await this.ap(unit, s));
      if (modeOf(await this.state()) === "map") await this.endTurn(unit);
    }
    assert.fail(`pack ${pack} never engaged`);
  }

  // --- turn helpers
  /** Initiative units of the started session: players by slot, then living monsters by index. */
  async initiativeUnits(): Promise<[number, number][]> {
    const s = await this.state();
    const ms = await this.monsters();
    const units: [number, number][] = [];
    for (let slot = 0; slot < s.core.playerCount; slot++) units.push([slot, s.playerInitiative[slot]]);
    ms.forEach((m, i) => m.alive && units.push([MONSTER_BASE + i, s.monsterInitiative[i]]));
    return units;
  }
  async current(): Promise<{ s: any; unit: number }> {
    const s = await this.state();
    return { s, unit: s.core.turnOrder[s.core.turnIndex] };
  }
  async ap(who: number, s: any): Promise<number> {
    const p = await this.part(who);
    return p.apSeq === s.turnSeq ? p.core.actionPoints : p.maxAp;
  }

  /**
   * Shortest step list from `from` to any cell where `goal` holds, over passable cells not taken
   * by a living unit or a closed chest, with each step's action-point cost.
   */
  async route(from: { x: number; y: number }, goal: (x: number, y: number) => boolean, mover: number, toward = -1) {
    const s = await this.state();
    const taken = new Set<number>();
    for (const m of await this.monsters()) if (m.alive) taken.add(m.y * MAP_STRIDE + m.x);
    for (let i = 0; i < this.players.length; i++) {
      if (i !== mover && s.aliveMask & (1 << i)) taken.add(s.playerPos[i].y * MAP_STRIDE + s.playerPos[i].x);
    }
    // Arena update: in an arena its cells; on the map the chests and the packs' monsters occupy theirs.
    const mode = modeOf(s);
    const arena = mode === "arena" || mode === "won" ? preset.packs.find((pk) => pk.pack === s.arenaPack)! : null;
    if (!arena) {
      // Patch 11: a closed chest occupies its cell.
      for (let i = 0; i < s.chestCount; i++) if (!(s.chestsOpened & (1 << i))) taken.add(s.chests[i].y * MAP_STRIDE + s.chests[i].x);
      for (let n = 0; n < 8; n++) {
        if (!s.packs[n].width || !(s.packsAlive & (1 << n))) continue;
        const { x: px, y: py } = s.packs[n];
        taken.add(py * MAP_STRIDE + px);
        // Next to another pack's monster the walk would engage it.
        if (n !== toward) for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) taken.add((py + dy) * MAP_STRIDE + px + dx);
      }
    }
    // Patch 13: the step cost of the cell's tile type, as the session copied the Tileset.
    const byte = (x: number, y: number) => (arena ? arena.cells[y * arena.width + x]! : preset.tiles[y * MAP_STRIDE + x]!);
    const tile = (x: number, y: number) => (x < 0 || y < 0 || x >= s.width || y >= s.height ? 0 : s.tileCost[cellKind(byte(x, y))]);
    const cost = (x: number, y: number) => (taken.has(y * MAP_STRIDE + x) ? 0 : tile(x, y));
    const dirs = [N, NE, E, SE, S, SW, W, NW].map((d) => [d, ...DELTA[d]]);
    // Dijkstra on a 32x32 grid: a linear scan for the minimum is plenty.
    const dist = new Map<number, number>([[from.y * MAP_STRIDE + from.x, 0]]);
    const prev = new Map<number, [number, number, number]>();
    const done = new Set<number>();
    for (;;) {
      let best = -1;
      for (const [k, d] of dist) if (!done.has(k) && (best < 0 || d < dist.get(best)!)) best = k;
      if (best < 0) return null;
      done.add(best);
      const x = best % MAP_STRIDE;
      const y = Math.floor(best / MAP_STRIDE);
      if (goal(x, y)) {
        const steps: { dir: number; cost: number }[] = [];
        for (let k = best; prev.has(k); k = prev.get(k)![0]) steps.unshift({ dir: prev.get(k)![1], cost: prev.get(k)![2] });
        return steps;
      }
      for (const [dir, dx, dy] of dirs) {
        // A diagonal never cuts a corner: both orthogonal cells must be passable tiles.
        if (dx && dy && !(tile(x + dx, y) && tile(x, y + dy))) continue;
        const c = cost(x + dx, y + dy);
        if (!c) continue;
        const k = (y + dy) * MAP_STRIDE + x + dx;
        const d = dist.get(best)! + c;
        if (!dist.has(k) || d < dist.get(k)!) {
          dist.set(k, d);
          prev.set(k, [best, dir, c]);
        }
      }
    }
  }

  /** Walks as far along `steps` as `ap` (minus `keep`) allows. Returns the action points spent. */
  async walk(who: number | null, monster: number | null, steps: { dir: number; cost: number }[], ap: number, keep = 0) {
    const take: number[] = [];
    let spent = 0;
    for (const st of steps) {
      if (spent + st.cost > ap - keep || take.length === MAX_PATH) break;
      take.push(st.dir);
      spent += st.cost;
    }
    if (take.length === 0) return 0;
    if (who !== null) {
      await logCu(`move_path player, ${take.length} cells`, await this.move(who, take));
      return spent;
    }
    // A monster's walk stops on the first cell next to a Cold Snap bearer (patch 5): the AP
    // actually spent, after the reactive roll.
    const before = (await this.monsters())[monster!].actionPoints;
    await logCu(`move_path monster, ${take.length} cells`, await this.monsterMove(monster!, take));
    const after = (await this.monsters())[monster!].actionPoints;
    await this.settle();
    return before - after;
  }
}

// ---------------------------------------------------------------------------------------------

describe("demo script end to end (CLAUDE.md section 8)", { skip: SKIP }, () => {
  const demo = new Demo();
  /** The warrior's strike this turn: Piercing Thrust off cooldown, else the weapon. */
  const warriorStrike = async () =>
    (await demo.part(WARRIOR)).cooldowns[PIERCING_THRUST] === 0 ? PIERCING_THRUST : BASE_ATTACK;
  let wolves: number[] = [];
  let boss = -1;
  /** The demo preset's wolves and boss pack numbers. */
  const [WOLF_PACK, BOSS_PACK] = [1, 3];

  before(async () => {
    [demo.dm, demo.dmActor] = await Promise.all([funded(), funded(1)]);
  });

  after(() => {
    if (signatures.length === 0) return;
    const rpc = encodeURIComponent(provider.connection.rpcEndpoint);
    console.log(`\n${signatures.length} transactions, every roll public:`);
    for (const { what, sig } of signatures) {
      console.log(`  ${what.padEnd(22)} https://explorer.solana.com/tx/${sig}?cluster=custom&customUrl=${rpc}`);
    }
  });

  it("two players create characters: human warrior (piercing) and forest elf mage (cold), each a Core NFT", async () => {
    for (const kind of ["warrior", "mage"] as Kind[]) {
      const [wallet, actor] = await Promise.all([funded(), funded(1)]);
      const { character, asset, sig } = await demo.createCharacter(wallet, kind);
      const ev = await eventOf(sig, "characterCreated");
      assert.ok(ev.character.equals(character));
      assert.ok(ev.nft?.equals(asset), "Core NFT minted");
      const c = await characterProgram.account.character.fetch(character);
      assert.ok(c.core.nft.equals(asset));
      assert.ok(c.core.alive);
      if (kind === "warrior") {
        assert.ok(c.core.race.human && c.core.class.warrior && c.core.predisposition.piercing);
      } else {
        assert.ok(c.core.race.forestElf && c.core.class.mage && c.core.predisposition.cold);
      }
      const nft = await connection.getAccountInfo(asset);
      assert.ok(nft?.owner.equals(MPL_CORE), "asset owned by Metaplex Core");
      // Starter items by class, in the character, worn: sword + leather armor, staff + robe.
      assert.equal(ev.itemCount, 2);
      const [weapon, armor] = kind === "warrior" ? ["sword", "leatherArmor"] : ["staff", "robe"];
      assert.ok(c.gear.items[0].base[weapon] !== undefined && c.gear.items[1].base[armor] !== undefined);
      assert.ok(c.gear.equipped[0].source.soulbound !== undefined && c.gear.equipped[3].source.soulbound !== undefined);
      demo.players.push({ kind, wallet, actor, token: PublicKey.default, character, asset, participation: PublicKey.default, rewardCache: PublicKey.default, skillSet: PublicKey.default, bag: PublicKey.default });
    }
  });

  it("the DM builds the demo map from the preset, validates and finalizes it", async () => {
    await demo.buildMap();
    const map = await mapProgram.account.map.fetch(demo.map);
    assert.ok(map.core.status.finalized);
    assert.equal(map.core.width, preset.width);
    // Arena update: the slots in each pack's arena (spawn order within the pack).
    const slotsOf = (pack: number) => preset.spawns.filter((x) => x.pack === pack);
    wolves = slotsOf(WOLF_PACK).flatMap((x, i) => (x.bestiaryId === WOLF ? [i] : []));
    boss = slotsOf(BOSS_PACK).findIndex((x) => x.bestiaryId === CHIEFTAIN);
    assert.equal(wolves.length, 2);
    assert.ok(boss >= 0);
  });

  it("party with a price: players pay into escrow, start a session key once, ready, start", async () => {
    const sig = await session.methods
      .openSession(PRICE, demo.dmActor.publicKey)
      .accountsStrict({
        dm: demo.dm.publicKey,
        map: demo.map,
        layout: demo.layout,
        ...demo.acc,
        sessionAuthority,
        config,
        bestiary,
        tileset,
        mapProgram: MAP,
        systemProgram: SystemProgram.programId,
        aspects,
        traits,
        resonances,
        monsterSkills,
        rewards,
        triples,
      })
      .signers([demo.dm])
      .rpc({ commitment: "confirmed" })
      .then(track("open_session"));
    // Patch C: only the monster each pack shows is public.
    assert.equal((await eventOf(sig, "sessionOpened")).monsters, preset.packs.length);
    assert.ok((await mapProgram.account.map.fetch(demo.map)).core.status.inSession);

    const escrowBefore = await connection.getBalance(demo.acc.escrow);
    const half = PRICE.divn(2);
    for (const pl of demo.players) {
      Object.assign(pl, demo.playerAccounts(pl.character));
      await session.methods
        .joinSession(half, PublicKey.default) // the session key comes with Ready
        .accountsStrict({
          player: pl.wallet.publicKey,
          session: demo.acc.session,
          character: pl.character,
          participation: pl.participation,
          rewardCache: pl.rewardCache,
          escrow: demo.acc.escrow,
          systemProgram: SystemProgram.programId,
        })
        .signers([pl.wallet])
        .rpc()
        .then(track(`join_session ${pl.kind}`));
    }
    assert.equal((await connection.getBalance(demo.acc.escrow)) - escrowBefore, PRICE.toNumber());
    demo.dmToken = await issueToken(demo.dmActor, demo.dm);

    // Ready: the 4 + 1 + 1 skills taken onto the map; the character is locked (bag and gear frozen).
    // The same main wallet transaction binds the player's session key (docs/PARTY_AND_TURNS.md 2.1).
    for (const pl of demo.players) {
      pl.token = await issueToken(pl.actor, pl.wallet);
      const bind = await session.methods
        .setSessionKey(pl.actor.publicKey)
        .accountsStrict({ signer: pl.wallet.publicKey, session: demo.acc.session, participation: pl.participation })
        .instruction();
      await session.methods
        .setReady(true, [0, 1, 2, 3, 4, 5])
        .accountsStrict({
          player: pl.wallet.publicKey,
          session: demo.acc.session,
          participation: pl.participation,
          character: pl.character,
          skillSet: pl.skillSet,
          sessionAuthority,
          config,
          characterProgram: CHARACTER,
          skillExtras: pda([Buffer.from("skill_extras"), pl.character.toBuffer()], CHARACTER),
        })
        .preInstructions([bind])
        .signers([pl.wallet])
        .rpc()
        .then(track(`set_ready ${pl.kind}`));
      assert.ok((await characterProgram.account.character.fetch(pl.character)).core.inSession.equals(demo.acc.session));
    }
    await session.methods
      .startSession()
      .accountsStrict({
        dm: demo.dm.publicKey,
        session: demo.acc.session,
        monsters: demo.acc.monsters,
        escrow: demo.acc.escrow,
        ...vrfAccounts,
      })
      .signers([demo.dm])
      .rpc()
      .then(track("start_session"));
    let s = await demo.state();
    assert.ok(s.core.status.active && s.core.pendingVrf, "waiting for the initiative roll");
    assert.equal(s.turnOrderLen, 0);

    // The oracle answers the initiative request (action 0) with player slot 0's accounts.
    const units = await demo.initiativeUnits();
    // Arena update: on the map only the players take turns.
    assert.equal(units.length, 2);
    // A fixed VRF value: every run rolls the same order.
    const seed = createHash("sha256").update("deadroll:demo:initiative").digest();
    const rolled = await demo.consume(0, seed);
    const ev = await eventOf(rolled, "initiativeRolled");
    const expected = initiativeOrder(seed, units);
    assert.equal(ev.actionCounter, 0);
    assert.deepEqual(ev.order.slice(0, ev.len), expected.order);
    assert.deepEqual(ev.d20.slice(0, ev.len), expected.d20);
    s = await demo.state();
    assert.ok(!s.core.pendingVrf);
    assert.deepEqual([...s.core.turnOrder.slice(0, s.turnOrderLen)], expected.order);
    console.log(`  initiative: ${expected.order.map((u) => (u < MONSTER_BASE ? demo.players[u].kind : `monster ${u - MONSTER_BASE}`)).join(", ")}`);
  });

  it("combat with wolves: moves, attacks, VRF rolls, hit breakdown, a resonance; the mage dies", async () => {
    // Arena update: the warrior walks next to the wolves on the map; the party enters their arena.
    await demo.engage(WOLF_PACK);
    assert.equal(modeOf(await demo.state()), "arena");
    // A turn ends unless the action just won the arena.
    const pass = async (who: number | null) => {
      if (modeOf(await demo.state()) === "arena") await demo.endTurn(who);
    };
    let resonanceShown = false;
    let mageDied: { sig: string; fin: string | null } | null = null;
    for (let guard = 0; guard < 300; guard++) {
      const { s, unit } = await demo.current();
      // Arena update: the last wolf's death wins the arena.
      if (modeOf(s) === "won") {
        console.log(`  wolves' arena won, the mage ${(s.aliveMask & (1 << MAGE)) !== 0 ? "alive" : "dead"}, deaths pending ${s.pendingDeaths}`);
        if (s.pendingDeaths) await demo.begin();
        break;
      }
      if (s.core.status.active && s.startedSeq !== s.turnSeq) {
        await demo.begin(); // every turn starts first (states, cooldowns)
        continue;
      }
      const ms = await demo.monsters();
      const livingWolves = wolves.filter((i) => ms[i].alive);
      const mageAlive = (s.aliveMask & (1 << MAGE)) !== 0;
      if (!mageAlive && livingWolves.length === 0) break;
      assert.ok(s.core.status.active, "party still active");

      if (unit === WARRIOR) {
        // The warrior fights the first wolf; the second one is left to the mage until it falls.
        const targets = mageAlive ? livingWolves.slice(0, 1).filter((i) => i === wolves[0]) : livingWolves;
        let ap = await demo.ap(WARRIOR, s);
        const target = targets[0];
        if (target !== undefined) {
          const pos = s.playerPos[WARRIOR];
          if (cheb(pos, ms[target]) > 1) {
            const steps = await demo.route(pos, (x, y) => cheb({ x, y }, ms[target]) === 1, WARRIOR);
            if (steps) ap -= await demo.walk(WARRIOR, null, steps, ap, 2);
          }
          const now = (await demo.state()).playerPos[WARRIOR];
          // One attack per turn: Piercing Thrust, or the weapon while it is on cooldown.
          if (ap >= 2 && cheb(now, ms[target]) === 1 && (await demo.monsters())[target].alive) {
            const hpBefore = (await demo.monsters())[target].hp;
            const { hit, events } = await demo.attack(WARRIOR, await warriorStrike(), target, CRIT);
            // The app's shape: session, attacker, target, seed, actionCounter, result (HitResult).
            assert.ok(hit.session.equals(demo.acc.session));
            assert.deepEqual([hit.attacker, hit.target], [WARRIOR, MONSTER_BASE + target]);
            assert.ok(hit.actionCounter >= 1, "action 0 is the initiative roll");
            assert.deepEqual(Buffer.from(hit.seed), CRIT);
            const r = hit.result;
            assert.ok(r.outcome.hit && r.d20 === 20 && r.crit, "natural 20: hit and crit");
            assert.ok(r.damage > 0 && r.packetCount >= 1 && r.rollsUsed >= 2, "the dice cascade and packets");
            const after = (await demo.monsters())[target];
            assert.equal(after.hp, Math.max(0, hpBefore - r.damage));
            const killed = events.find((x) => x.name === "monsterKilled")?.data;
            assert.equal(!!killed, !after.alive);
            if (killed) assert.ok(killed.monster === target && !killed.boss);
          }
        }
        await pass(WARRIOR);
      } else if (unit === MAGE) {
        const target = livingWolves[livingWolves.length - 1];
        if (!resonanceShown && target !== undefined && cheb(s.playerPos[MAGE], ms[target]) <= 6) {
          // Frost Burn (cold + fire) with the mage's cold race tint: the fire base rolls it (Pale Flame).
          const { hit } = await demo.attack(MAGE, FROST_BURN, target, RESONANT);
          const r = hit.result;
          assert.ok(r.outcome.hit);
          const res = r.resonances.filter((x: any) => x !== null);
          console.log(`  mage Frost Burn: damage ${r.damage}, resonances ${JSON.stringify(res.map((x: any) => ({ id: x.id, active: x.active })))}`);
          const fireCold = r.tintRolls.slice(0, r.tintRollCount).find((t: any) => t.base.fire && t.tint.cold);
          assert.ok(fireCold?.success, "the Fire base's Cold tint roll succeeded");
          assert.ok(res.some((x: any) => x.active), "a resonance triggered");
          resonanceShown = true;
        }
        await pass(MAGE);
      } else {
        const mi = unit - MONSTER_BASE;
        const m = ms[mi];
        if (wolves.includes(mi) && m.alive && mageAlive) {
          // The DM sends the wolves at the mage.
          const target = s.playerPos[MAGE];
          let ap = m.actionPoints;
          if (cheb(m, target) > 1) {
            const steps = await demo.route(m, (x, y) => cheb({ x, y }, target) === 1, -1);
            if (steps) ap -= await demo.walk(null, mi, steps, ap, 2);
          }
          const near = cheb((await demo.monsters())[mi], target) === 1;
          if (near && ap >= 2) {
            await demo.monsterAct(mi, MAGE);
            const sig = await demo.consume(MAGE, CRIT);
            const hit = await eventOf(sig, "hitResolved");
            assert.equal(hit.attacker, unit);
            const died = (await eventsOf(sig)).find((x) => x.name === "playerDied");
            // Patch 6: down at once; the next start_turn makes the tombstone.
            if (died) mageDied = { sig, fin: await demo.settle() };
          }
          if ((await demo.state()).core.turnOrder[(await demo.state()).core.turnIndex] === unit) await pass(null);
        } else {
          await pass(null);
        }
      }
    }
    assert.ok(resonanceShown, "the mage's resonance hit happened");
    assert.ok(mageDied, "the mage died");
    const died = await eventOf(mageDied.sig, "playerDied");
    assert.equal(died.slot, MAGE);
    assert.ok(!(await eventsOf(mageDied.sig)).some((x) => x.name === "characterDied"), "no mark_dead in the callback");
    const tomb = await eventOf(mageDied.fin!, "characterDied");
    assert.ok(tomb.character.equals(demo.players[MAGE].character));
    assert.equal(tomb.stateVersion, 1);
    const ms = await demo.monsters();
    assert.ok(wolves.every((i) => !ms[i].alive), "both wolves dead");
  });

  it("the dead character becomes a tombstone: skills, bag and starter items burn, NFT frozen", async () => {
    const mage = demo.players[MAGE];
    const c = await demo.character(MAGE);
    assert.ok(!c.core.alive);
    assert.ok(c.core.inSession.equals(demo.acc.session));
    assert.equal(c.gear.itemCount, 0, "soulbound items vanish with the character");
    const set = await characterProgram.account.skillSet.fetch(mage.skillSet);
    assert.ok(set.skills.slice(0, set.count).every((k: any) => k.powerBp === 0 && k.name[0] !== 0), "skills burn, names stay");
    const sig = await characterProgram.methods
      .freezeTombstone()
      .accountsStrict({
        character: mage.character,
        asset: mage.asset,
        payer: provider.wallet.publicKey,
        mplCoreProgram: MPL_CORE,
        systemProgram: SystemProgram.programId,
      })
      .rpc({ commitment: "confirmed" })
      .then(track("freeze_tombstone"));
    const ev = await eventOf(sig, "tombstoneFrozen");
    assert.ok(ev.asset.equals(mage.asset));
    assert.ok((await demo.character(MAGE)).nftFrozen);
  });

  it("the warrior kills the goblin chieftain: victory", async () => {
    // Arena update: the won wolves' arena pays its experience and brings the party back; the
    // warrior walks to the boss pack.
    assert.equal(modeOf(await demo.state()), "won");
    const xpBefore = (await demo.part(WARRIOR)).cacheXp;
    await logCu("leave_arena", await demo.leaveArena());
    assert.equal(modeOf(await demo.state()), "map");
    assert.ok((await demo.part(WARRIOR)).cacheXp > xpBefore, "the wolves' experience");
    await demo.engage(BOSS_PACK);
    let victory: string | null = null;
    for (let guard = 0; guard < 400 && !victory; guard++) {
      const { s, unit } = await demo.current();
      assert.ok(s.core.status.active, "party still active");
      if (s.startedSeq !== s.turnSeq) {
        await demo.begin();
        continue;
      }
      if (unit !== WARRIOR) {
        await demo.endTurn(null); // the DM passes every monster turn
        continue;
      }
      const chief = (await demo.monsters())[boss];
      let ap = await demo.ap(WARRIOR, s);
      if (cheb(s.playerPos[WARRIOR], chief) > 1) {
        const steps = await demo.route(s.playerPos[WARRIOR], (x, y) => cheb({ x, y }, chief) === 1, WARRIOR);
        assert.ok(steps, "a path to the chieftain");
        ap -= await demo.walk(WARRIOR, null, steps, ap, 2);
      }
      // One attack per turn: Piercing Thrust, or the weapon while it is on cooldown.
      if (ap >= 2 && cheb((await demo.state()).playerPos[WARRIOR], chief) === 1) {
        const { sig, hit, events } = await demo.attack(WARRIOR, await warriorStrike(), boss, CRIT);
        assert.ok(hit.result.outcome.hit);
        // A resonance may take a helper next to it too: the boss's own kill.
        const killed = events.find((x) => x.name === "monsterKilled" && x.data.monster === boss)?.data;
        if (killed) {
          assert.ok(killed.boss);
          const ended = events.find((x) => x.name === "sessionEnded")!.data;
          assert.ok(ended.status.victory);
          victory = sig;
        }
      }
      if (!victory) await demo.endTurn(WARRIOR);
    }
    assert.ok(victory, "boss killed");
    const s = await demo.state();
    assert.ok(s.core.status.victory);
    // Patch 11: the kills filled the survivor's cache during the party.
    assert.ok((await demo.part(WARRIOR)).cacheXp > 0);
  });

  it("the DM settles the party: XP for the living, nothing for the tombstone, the price from escrow", async () => {
    const before = await demo.character(WARRIOR);
    const cacheXp = (await demo.part(WARRIOR)).cacheXp;
    const dmBefore = await connection.getBalance(demo.dm.publicKey);
    const players = demo.players.flatMap((pl) =>
      [pl.participation, pl.rewardCache, pl.character].map((pubkey) => ({ pubkey, isSigner: false, isWritable: true })),
    );
    // Patch C: the escrow waits for the final audit (docs/PARTY_AND_TURNS.md 3.2).
    await demo.audit();
    const sig = await session.methods
      .settleParty()
      .accountsStrict({
        dm: demo.dm.publicKey, // the DM's main wallet: the session key cannot settle
        session: demo.acc.session,
        escrow: demo.acc.escrow,
        monsters: demo.acc.monsters,
        map: demo.map,
        sessionAuthority,
        config,
        mapProgram: MAP,
        characterProgram: CHARACTER,
      })
      .remainingAccounts([...deathAccounts(await demo.state()), ...players])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
      .signers([demo.dm])
      .rpc({ commitment: "confirmed" })
      .then(track("settle_party"));
    const settled = await eventsOf(sig);
    const byName = (name: string) => settled.filter((x) => x.name === name).map((x) => x.data);
    const [warrior, mage] = byName("cacheSettled");
    assert.deepEqual([warrior.granted, warrior.xp], [true, cacheXp]);
    assert.deepEqual([mage.granted, mage.xp], [false, 0]);
    const after = await demo.character(WARRIOR);
    assert.equal(after.core.xp, before.core.xp + cacheXp);
    assert.equal(after.core.inSession, null, "unlocked");
    assert.ok(!(await demo.character(MAGE)).core.alive);
    const [escrow] = byName("escrowClaimed");
    assert.equal(escrow.amount.toString(), PRICE.toString());
    const gained = (await connection.getBalance(demo.dm.publicKey)) - dmBefore;
    assert.ok(gained > PRICE.toNumber() - 20_000, `DM got ${gained}`);
    assert.ok((await mapProgram.account.map.fetch(demo.map)).core.status.finished);
  });

  it("every roll is public: each VRF callback is a confirmed transaction with its event", async () => {
    const rolls = signatures.filter((x) => x.what === "consume_randomness");
    assert.ok(rolls.length >= 3);
    assert.equal(publicRolls.length, rolls.length);
    for (const { sig, ok } of publicRolls) assert.ok(ok, `${sig}: no InitiativeRolled / HitResolved / ReactiveTriggered in its logs`);
  });
});
