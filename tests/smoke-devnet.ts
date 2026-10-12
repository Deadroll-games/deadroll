// Devnet smoke test for the real MagicBlock VRF: one character, a tiny party, start_session, then wait
// for the oracle callback and print the InitiativeRolled transaction. Needs tools/scripts/deploy-devnet.ts.
// The session must be the real-VRF build: nothing here plays the oracle.
//
// cd tests && ANCHOR_PROVIDER_URL=<devnet rpc> ANCHOR_WALLET=~/.config/solana/id.json \
//   DEADROLL_WS_URL=wss://api.devnet.solana.com node --import tsx smoke-devnet.ts
import { readFileSync } from "node:fs";
import anchor from "@anchor-lang/core";
import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { createHash, randomBytes } from "node:crypto";
import { layoutBytes, loadMapPreset } from "../tools/scripts/lib/demo-map.ts";

const { AnchorProvider, BN, EventParser, Program } = anchor;

const env = AnchorProvider.env();
const provider = new AnchorProvider(new Connection(env.connection.rpcEndpoint, { commitment: "confirmed", wsEndpoint: process.env.DEADROLL_WS_URL }), env.wallet, {
  commitment: "confirmed",
  preflightCommitment: "confirmed",
});
const connection = provider.connection;

const REGISTRY = new PublicKey("dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG");
const CHARACTER = new PublicKey("9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL");
const MAP = new PublicKey("2YeyBfQ5xkRKD2kagxRS2yZYCQdWt5qidikaKfQhLBXH");
const SESSION = new PublicKey("3gzfih2WaBgXcYZ57jWokmZM3FDE6QHX5y4R9EvKZoNJ");
const MPL_CORE = new PublicKey("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");
const VRF_PROGRAM = new PublicKey("Vrf1RNUjXmQGjmQrQLvJHs9SNkvDJEsRVFPkfSQUwGz");
const ORACLE_QUEUE = new PublicKey("Cuj97ggrhhidhbu39TijNVqE74xvKJ69gDervRUXAxGh");
const SLOT_HASHES = new PublicKey("SysvarS1otHashes111111111111111111111111111");

const PRICE = new BN(10_000_000);
const pda = (seeds: (Buffer | Uint8Array)[], programId: PublicKey) => PublicKey.findProgramAddressSync(seeds, programId)[0];
const e = (name: string) => ({ [name]: {} });
const text = (s: string, len: number) => {
  const out = new Array(len).fill(0);
  Buffer.from(s, "ascii").forEach((b, i) => (out[i] = b));
  return out;
};
const explorer = (sig: string) => `https://explorer.solana.com/tx/${sig}?cluster=devnet`;

const load = (n: string) => JSON.parse(readFileSync(new URL(`../target/idl/${n}.json`, import.meta.url), "utf8"));
const session: any = new Program(load("deadroll_session"), provider);
const characterProgram: any = new Program(load("deadroll_character"), provider);
const mapProgram: any = new Program(load("deadroll_map"), provider);
const sessionEvents = new EventParser(SESSION, session.coder);

const config = pda([Buffer.from("config")], REGISTRY);
const vrfAccounts = {
  programIdentity: pda([Buffer.from("identity")], SESSION),
  oracleQueue: ORACLE_QUEUE,
  vrfProgram: VRF_PROGRAM,
  slotHashes: SLOT_HASHES,
  systemProgram: SystemProgram.programId,
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
async function fund(to: PublicKey, sol: number) {
  const ix = SystemProgram.transfer({ fromPubkey: provider.wallet.publicKey, toPubkey: to, lamports: Math.round(sol * LAMPORTS_PER_SOL) });
  await provider.sendAndConfirm(new Transaction().add(ix));
}

const dm = Keypair.generate();
const player = Keypair.generate();
const dmActor = Keypair.generate();
const playerActor = Keypair.generate();
await fund(dm.publicKey, 0.1);
await fund(player.publicKey, 0.15);

// character
const profile = pda([Buffer.from("profile"), player.publicKey.toBuffer()], CHARACTER);
await characterProgram.methods.createProfile().accountsStrict({ owner: player.publicKey, profile, systemProgram: SystemProgram.programId }).signers([player]).rpc();
const character = pda([Buffer.from("character"), player.publicKey.toBuffer(), Buffer.from([0])], CHARACTER);
const layer = { spriteId: 1, materialRamp: 2, accentRamp: 3, elementRamp: 0 };
const asset = Keypair.generate();
await characterProgram.methods
  .createCharacter({
    name: text("Smoke", 32), race: e("human"), class: e("warrior"), predisposition: e("piercing"),
    stats: [10, 7, 5, 8], appearance: new Array(20).fill(layer),
    skills: WARRIOR_SKILLS.map(([name, b]) => ({ blocks: structuredClone(b), name: text(name, 24) })),
  })
  .accountsStrict({
    owner: player.publicKey, profile, character,
    skillSet: pda([Buffer.from("skills"), character.toBuffer()], CHARACTER),
    bag: pda([Buffer.from("bag"), character.toBuffer()], CHARACTER),
    config, asset: asset.publicKey, mplCoreProgram: MPL_CORE, systemProgram: SystemProgram.programId,
  })
  .signers([player, asset])
  .rpc();
console.log("character + Core NFT:", character.toBase58(), asset.publicKey.toBase58());

// map
const preset = loadMapPreset();
const index = randomBytes(4).readUInt32LE(0);
const idx = Buffer.alloc(4);
idx.writeUInt32LE(index);
const map = pda([Buffer.from("map"), dm.publicKey.toBuffer(), idx], MAP);
const layout = pda([Buffer.from("layout"), map.toBuffer()], MAP);
const edit = { dm: dm.publicKey, map, layout };
await mapProgram.methods.createMap(index, preset.width, preset.height, preset.tier).accountsStrict({ ...edit, systemProgram: SystemProgram.programId }).signers([dm]).rpc();
await mapProgram.methods.writeLayout(0, Buffer.from(layoutBytes(preset))).accountsStrict(edit).signers([dm]).rpc();
await mapProgram.methods.writeSpawns(preset.entrance, preset.exit, preset.spawns).accountsStrict(edit).signers([dm]).rpc();
await mapProgram.methods.finalizeMap(new Array(32).fill(0)).accountsStrict(edit).signers([dm]).rpc();

// session
const sessionPda = pda([Buffer.from("session"), map.toBuffer()], SESSION);
const monsters = pda([Buffer.from("monsters"), sessionPda.toBuffer()], SESSION);
const escrow = pda([Buffer.from("escrow"), sessionPda.toBuffer()], SESSION);
const sessionAuthority = pda([Buffer.from("authority")], SESSION);
await session.methods
  .openSession(PRICE, dmActor.publicKey)
  .accountsStrict({
    dm: dm.publicKey, map, layout, session: sessionPda, monsters, escrow, sessionAuthority, config,
    bestiary: pda([Buffer.from("bestiary")], REGISTRY), tileset: pda([Buffer.from("tileset")], REGISTRY),
    mapProgram: MAP, systemProgram: SystemProgram.programId,
  })
  .signers([dm])
  .rpc();
const participation = pda([Buffer.from("participation"), sessionPda.toBuffer(), character.toBuffer()], SESSION);
await session.methods
  .joinSession(PRICE, playerActor.publicKey)
  .accountsStrict({
    player: player.publicKey, session: sessionPda, character, participation,
    stash: pda([Buffer.from("stash"), sessionPda.toBuffer(), character.toBuffer()], SESSION),
    escrow, sessionAuthority, config, characterProgram: CHARACTER, systemProgram: SystemProgram.programId,
  })
  .signers([player])
  .rpc();
await session.methods.setReady(true).accountsStrict({ player: player.publicKey, session: sessionPda, participation }).signers([player]).rpc();

const startSig = await session.methods
  .startSession()
  .accountsStrict({ dm: dm.publicKey, session: sessionPda, monsters, escrow, ...vrfAccounts })
  .signers([dm])
  .rpc({ commitment: "confirmed" });
console.log("start_session:", explorer(startSig));

// the real oracle answers
const deadline = Date.now() + 120_000;
for (;;) {
  const state = await session.account.gameSession.fetch(sessionPda);
  if (!state.core.pendingVrf) break;
  if (Date.now() > deadline) throw new Error("no VRF callback within 120 s: the oracle did not answer");
  await new Promise((r) => setTimeout(r, 2000));
}
for (const { signature } of await connection.getSignaturesForAddress(sessionPda, { limit: 20 }, "confirmed")) {
  const tx = await connection.getTransaction(signature, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  const ev = sessionEvents.parseLogs(tx?.meta?.logMessages ?? []).find((x) => x.name.toLowerCase() === "initiativerolled");
  if (ev) {
    console.log("InitiativeRolled:", explorer(signature));
    console.log("order", JSON.stringify(ev.data.order), "d20", JSON.stringify(ev.data.d20));
    process.exit(0);
  }
}
throw new Error("pendingVrf cleared but no InitiativeRolled event found");
