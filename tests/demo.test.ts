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
  TransactionInstruction,
} from "@solana/web3.js";
import { MAP_STRIDE, Tile, layoutBytes, loadMapPreset } from "../tools/scripts/lib/demo-map.ts";

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

const REGISTRY = new PublicKey("dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG");
const CHARACTER = new PublicKey("9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL");
const MAP = new PublicKey("2YeyBfQ5xkRKD2kagxRS2yZYCQdWt5qidikaKfQhLBXH");
const SESSION = new PublicKey("3gzfih2WaBgXcYZ57jWokmZM3FDE6QHX5y4R9EvKZoNJ");
const SESSION_KEYS = new PublicKey("KeyspM2ssCJbqUhQ4k7sveSiY4WjnYsrXkC8oDbwde5");
const MPL_CORE = new PublicKey("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");
// MagicBlock VRF accounts (ephemeral-vrf-sdk consts); under mock-vrf they only pass address checks.
const VRF_PROGRAM = new PublicKey("Vrf1RNUjXmQGjmQrQLvJHs9SNkvDJEsRVFPkfSQUwGz");
const ORACLE_QUEUE = new PublicKey("Cuj97ggrhhidhbu39TijNVqE74xvKJ69gDervRUXAxGh");
const SLOT_HASHES = new PublicKey("SysvarS1otHashes111111111111111111111111111");

const PRICE = new BN(LAMPORTS_PER_SOL / 10);
const [N, E, S, W] = [0, 1, 2, 3];
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
// helpers (same conventions as tests/session.test.ts)

const pda = (seeds: (Buffer | Uint8Array)[], programId: PublicKey) => PublicKey.findProgramAddressSync(seeds, programId)[0];
const sha256 = (data: Buffer | string) => createHash("sha256").update(data).digest();
const sighash = (preimage: string) => sha256(preimage).subarray(0, 8);

const config = pda([Buffer.from("config")], REGISTRY);
const bestiary = pda([Buffer.from("bestiary")], REGISTRY);
const tileset = pda([Buffer.from("tileset")], REGISTRY);
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
function seedWhere(pred: (seed: Buffer) => boolean): Buffer {
  for (;;) {
    const seed = randomBytes(32);
    if (pred(seed)) return seed;
  }
}
const d20 = (seed: Buffer) => Number(rollAt(seed, 0) % 20n) + 1;
/** Natural 20: always a hit and a crit. */
const CRIT = seedWhere((s) => d20(s) === 20);
const MISS = seedWhere((s) => d20(s) === 1);
/**
 * A crit whose first tint roll (roll 1: a natural 20 makes no crit-chance roll) is below 1%, so
 * any tint with a real chance succeeds (crates/combat hit order, steps 1-3).
 */
const RESONANT = seedWhere((s) => d20(s) === 20 && rollAt(s, 1) % 10_000n < 100n);

const signatures: { what: string; sig: string }[] = [];
/** Every VRF callback, read back from the chain right after it landed. */
const publicRolls: { sig: string; ok: boolean }[] = [];
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

function path(steps: number[]): [number[], number] {
  const padded = [...steps.slice(0, MAX_PATH)];
  while (padded.length < MAX_PATH) padded.push(0);
  return [padded, steps.length];
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
for (const [name, id] of [["registry", REGISTRY], ["character", CHARACTER], ["map", MAP], ["session", SESSION], ["mpl-core", MPL_CORE], ["session-keys", SESSION_KEYS]] as const) {
  if (!(await isProgram(id))) missing.push(`${name} program`);
}
const cfg = await connection.getAccountInfo(config);
if (!cfg?.owner.equals(REGISTRY)) missing.push("registry GameConfig");
else if (!new PublicKey(cfg.data.subarray(41 + 5 * 32, 41 + 6 * 32)).equals(SESSION)) missing.push("GameConfig trusting session");
for (const [name, table] of [["bestiary", bestiary], ["tileset", tileset]] as const) {
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
  stash: PublicKey;
  skillSet: PublicKey;
  bag: PublicKey;
}

const preset = loadMapPreset();

class Demo {
  dm!: Keypair;
  dmActor!: Keypair;
  dmToken!: PublicKey;
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
      skills: (warrior ? WARRIOR_SKILLS : MAGE_SKILLS).map(([name, b]) => ({ blocks: structuredClone(b), name: text(name, 24) })),
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
      .createMap(index, preset.width, preset.height, preset.tier)
      .accountsStrict({ ...edit, systemProgram: SystemProgram.programId })
      .signers([this.dm])
      .rpc()
      .then(track("create_map"));
    await mapProgram.methods.writeLayout(0, Buffer.from(layoutBytes(preset))).accountsStrict(edit).signers([this.dm]).rpc().then(track("write_layout"));
    await mapProgram.methods.writeSpawns(preset.entrance, preset.exit, preset.spawns).accountsStrict(edit).signers([this.dm]).rpc().then(track("write_spawns"));
    await mapProgram.methods.finalizeMap(new Array(32).fill(0)).accountsStrict(edit).signers([this.dm]).rpc().then(track("finalize_map"));
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
      stash: pda([Buffer.from("stash"), s.toBuffer(), character.toBuffer()], SESSION),
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
  move(who: number, steps: number[]) {
    const pl = this.players[who];
    const [padded, len] = path(steps);
    return session.methods
      .moveUnit(padded, len)
      .accountsStrict({
        signer: pl.actor.publicKey,
        session: this.acc.session,
        participation: pl.participation,
        monsters: this.acc.monsters,
        layout: this.layout,
        sessionToken: pl.token,
      })
      .signers([pl.actor])
      .rpc()
      .then(track(`move ${pl.kind}`));
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
        monsters: this.acc.monsters,
        sessionToken: pl.token,
        ...vrfAccounts,
        targetParticipation: null,
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
      })
      .rpc({ commitment: "confirmed" })
      .then(track(`start_turn ${pl?.kind ?? "monster"}`));
  }
  // --- DM moves through the DM's session key
  monsterMove(monster: number, steps: number[]) {
    const [padded, len] = path(steps);
    return session.methods
      .monsterMove(monster, padded, len)
      .accountsStrict({
        signer: this.dmActor.publicKey,
        session: this.acc.session,
        monsters: this.acc.monsters,
        layout: this.layout,
        sessionToken: this.dmToken,
      })
      .signers([this.dmActor])
      .rpc()
      .then(track("monster_move"));
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
        sessionToken: this.dmToken,
        ...vrfAccounts,
      })
      .signers([this.dmActor])
      .rpc()
      .then(track("monster_act"));
  }
  /** The oracle's callback for the pending action of player `who` (attacker or target). */
  consume(who: number, seed: Buffer) {
    const pl = this.players[who];
    return session.methods
      .consumeRandomness([...seed])
      .accountsStrict({
        vrfProgramIdentity: MOCK_VRF.publicKey,
        session: this.acc.session,
        monsters: this.acc.monsters,
        participation: pl.participation,
        character: pl.character,
        skillSet: pl.skillSet,
        sessionAuthority,
        config,
        characterProgram: CHARACTER,
        targetParticipation: null,
      })
      .signers([MOCK_VRF])
      .rpc({ commitment: "confirmed" })
      .then(track("consume_randomness"))
      .then(async (sig: string) => {
        // Read back at once: the shared validator keeps ~100 slots of transaction history.
        const names = (await eventsOf(sig)).map((x) => x.name);
        publicRolls.push({ sig, ok: names.includes("hitResolved") || names.includes("initiativeRolled") });
        return sig;
      });
  }
  /** One player attack: request, oracle callback, the `HitResolved` event. */
  async attack(who: number, slot: number, monster: number, seed: Buffer) {
    await this.act(who, slot, monster);
    const sig = await this.consume(who, seed);
    return { sig, hit: await eventOf(sig, "hitResolved"), events: await eventsOf(sig) };
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
   * by a living unit (traps avoided), with each step's action-point cost.
   */
  async route(from: { x: number; y: number }, goal: (x: number, y: number) => boolean, mover: number) {
    const s = await this.state();
    const taken = new Set<number>();
    for (const m of await this.monsters()) if (m.alive) taken.add(m.y * MAP_STRIDE + m.x);
    for (let i = 0; i < this.players.length; i++) {
      if (i !== mover && s.aliveMask & (1 << i)) taken.add(s.playerPos[i].y * MAP_STRIDE + s.playerPos[i].x);
    }
    const cost = (x: number, y: number) => {
      if (x < 0 || y < 0 || x >= preset.width || y >= preset.height) return 0;
      const t = preset.tiles[y * MAP_STRIDE + x];
      if (t === Tile.Wall || t === Tile.Trap || taken.has(y * MAP_STRIDE + x)) return 0;
      return t === Tile.Water ? 2 : 1;
    };
    const dirs = [
      [N, 0, -1],
      [E, 1, 0],
      [S, 0, 1],
      [W, -1, 0],
    ];
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
    if (who !== null) await this.move(who, take);
    else await this.monsterMove(monster!, take);
    return spent;
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
      demo.players.push({ kind, wallet, actor, token: PublicKey.default, character, asset, participation: PublicKey.default, stash: PublicKey.default, skillSet: PublicKey.default, bag: PublicKey.default });
    }
  });

  it("the DM builds the demo map from the preset, validates and finalizes it", async () => {
    await demo.buildMap();
    const map = await mapProgram.account.map.fetch(demo.map);
    assert.ok(map.core.status.finalized);
    assert.equal(map.core.width, preset.width);
    wolves = preset.spawns.flatMap((s, i) => (s.bestiaryId === WOLF ? [i] : []));
    boss = preset.spawns.findIndex((s) => s.bestiaryId === CHIEFTAIN);
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
      })
      .signers([demo.dm])
      .rpc({ commitment: "confirmed" })
      .then(track("open_session"));
    assert.equal((await eventOf(sig, "sessionOpened")).monsters, preset.spawns.length);
    assert.ok((await mapProgram.account.map.fetch(demo.map)).core.status.inSession);

    const escrowBefore = await connection.getBalance(demo.acc.escrow);
    const half = PRICE.divn(2);
    for (const pl of demo.players) {
      Object.assign(pl, demo.playerAccounts(pl.character));
      await session.methods
        .joinSession(half, pl.actor.publicKey)
        .accountsStrict({
          player: pl.wallet.publicKey,
          session: demo.acc.session,
          character: pl.character,
          participation: pl.participation,
          stash: pl.stash,
          escrow: demo.acc.escrow,
          sessionAuthority,
          config,
          characterProgram: CHARACTER,
          systemProgram: SystemProgram.programId,
        })
        .signers([pl.wallet])
        .rpc()
        .then(track(`join_session ${pl.kind}`));
      assert.ok((await characterProgram.account.character.fetch(pl.character)).core.inSession.equals(demo.acc.session));
      // The one wallet signature of the play session: this key may act for the player.
      pl.token = await issueToken(pl.actor, pl.wallet);
    }
    assert.equal((await connection.getBalance(demo.acc.escrow)) - escrowBefore, PRICE.toNumber());
    demo.dmToken = await issueToken(demo.dmActor, demo.dm);

    for (const pl of demo.players) {
      await session.methods
        .setReady(true)
        .accountsStrict({ player: pl.wallet.publicKey, session: demo.acc.session, participation: pl.participation })
        .signers([pl.wallet])
        .rpc()
        .then(track(`set_ready ${pl.kind}`));
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
    assert.equal(units.length, 2 + preset.spawns.length);
    const seed = randomBytes(32);
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
    let resonanceShown = false;
    let mageDied: { sig: string } | null = null;
    for (let guard = 0; guard < 300; guard++) {
      const { s, unit } = await demo.current();
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
        await demo.endTurn(WARRIOR);
      } else if (unit === MAGE) {
        const target = livingWolves[livingWolves.length - 1];
        if (!resonanceShown && target !== undefined && cheb(s.playerPos[MAGE], ms[target]) <= 6) {
          // Frost Burn (cold + fire) with the mage's cold race tint: the fire base rolls the tint.
          const { hit } = await demo.attack(MAGE, FROST_BURN, target, RESONANT);
          const r = hit.result;
          assert.ok(r.outcome.hit);
          const res = r.resonances.filter((x: any) => x !== null);
          console.log(`  mage Frost Burn: damage ${r.damage}, resonances ${JSON.stringify(res.map((x: any) => ({ id: x.id, active: x.active })))}`);
          assert.ok(r.tintRollCount >= 1 && r.tintRolls[0].success, "the tint roll succeeded");
          assert.ok(res.some((x: any) => x.active), "a resonance triggered");
          resonanceShown = true;
        }
        await demo.endTurn(MAGE);
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
            if (died) mageDied = { sig };
          }
          if ((await demo.state()).core.turnOrder[(await demo.state()).core.turnIndex] === unit) await demo.endTurn(null);
        } else {
          await demo.endTurn(null);
        }
      }
    }
    assert.ok(resonanceShown, "the mage's resonance hit happened");
    assert.ok(mageDied, "the mage died");
    const died = await eventOf(mageDied.sig, "playerDied");
    assert.equal(died.slot, MAGE);
    const tomb = await eventOf(mageDied.sig, "characterDied");
    assert.ok(tomb.character.equals(demo.players[MAGE].character));
    assert.equal(tomb.stateVersion, 1);
    const ms = await demo.monsters();
    assert.ok(wolves.every((i) => !ms[i].alive), "both wolves dead");
  });

  it("the dead character becomes a tombstone: NFT frozen", async () => {
    const mage = demo.players[MAGE];
    const c = await demo.character(MAGE);
    assert.ok(!c.core.alive);
    assert.ok(c.core.inSession.equals(demo.acc.session));
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
        const killed = events.find((x) => x.name === "monsterKilled")?.data;
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
    assert.ok(s.xpPool > 0);
  });

  it("rewards: XP and loot for the living, nothing for the tombstone", async () => {
    const before = await demo.character(WARRIOR);
    const xpPool = (await demo.state()).xpPool;
    const claim = (who: number) => {
      const pl = demo.players[who];
      return session.methods
        .claimRewards()
        .accountsStrict({
          signer: pl.actor.publicKey, // the owner's session key: still no wallet popup
          session: demo.acc.session,
          participation: pl.participation,
          stash: pl.stash,
          character: pl.character,
          bag: pl.bag,
          sessionAuthority,
          config,
          characterProgram: CHARACTER,
          sessionToken: pl.token,
        })
        .signers([pl.actor])
        .rpc({ commitment: "confirmed" })
        .then(track(`claim_rewards ${pl.kind}`));
    };
    const ev = await eventOf(await claim(WARRIOR), "rewardsClaimed");
    assert.equal(ev.xp, xpPool);
    const after = await demo.character(WARRIOR);
    assert.equal(after.core.xp, before.core.xp + xpPool);
    assert.equal(after.core.inSession, null, "unlocked");
    const dead = await eventOf(await claim(MAGE), "rewardsClaimed");
    assert.equal(dead.xp, 0);
    assert.ok(!(await demo.character(MAGE)).core.alive);
  });

  it("the DM gets the price from escrow; the map is finished", async () => {
    const before = await connection.getBalance(demo.dm.publicKey);
    const sig = await session.methods
      .claimEscrow()
      .accountsStrict({
        dm: demo.dm.publicKey,
        session: demo.acc.session,
        escrow: demo.acc.escrow,
        map: demo.map,
        sessionAuthority,
        config,
        mapProgram: MAP,
      })
      .signers([demo.dm])
      .rpc({ commitment: "confirmed" })
      .then(track("claim_escrow"));
    const ev = await eventOf(sig, "escrowClaimed");
    assert.equal(ev.amount.toString(), PRICE.toString());
    const gained = (await connection.getBalance(demo.dm.publicKey)) - before;
    assert.ok(gained > PRICE.toNumber() - 10_000, `DM got ${gained}`);
    assert.ok((await mapProgram.account.map.fetch(demo.map)).core.status.finished);
  });

  it("every roll is public: each VRF callback is a confirmed transaction with its event", async () => {
    const rolls = signatures.filter((x) => x.what === "consume_randomness");
    assert.ok(rolls.length >= 3);
    assert.equal(publicRolls.length, rolls.length);
    for (const { sig, ok } of publicRolls) assert.ok(ok, `${sig}: no InitiativeRolled / HitResolved in its logs`);
  });
});
