// Devnet smoke test for items and the real MagicBlock VRF: a warrior created together with its
// starter kit (items init_loadout), unequip and equip, a tiny party, start_session, then turns until
// the warrior attacks a wolf; prints the oracle's InitiativeRolled and HitResolved transactions.
// Needs tools/scripts/deploy-devnet.ts. The session must be the real-VRF build: nothing here plays the oracle.
//
// cd tests && ANCHOR_PROVIDER_URL=<devnet rpc> ANCHOR_WALLET=<keypair.json> node --import tsx smoke-devnet.ts
// (DEADROLL_WS_URL overrides the websocket endpoint derived from the RPC.)
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import anchor from "@anchor-lang/core";
import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { randomBytes } from "node:crypto";
import { MAP_STRIDE, Tile, layoutBytes, loadMapPreset } from "../tools/scripts/lib/demo-map.ts";

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
const ITEMS = new PublicKey("48FtbrDegNrAUHFza9HdJzZAvUo3uUNqs1kNnZxFsqJS");
const MPL_CORE = new PublicKey("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");
const VRF_PROGRAM = new PublicKey("Vrf1RNUjXmQGjmQrQLvJHs9SNkvDJEsRVFPkfSQUwGz");
const ORACLE_QUEUE = new PublicKey("Cuj97ggrhhidhbu39TijNVqE74xvKJ69gDervRUXAxGh");
const SLOT_HASHES = new PublicKey("SysvarS1otHashes111111111111111111111111111");

const PRICE = new BN(10_000_000);
const [N, E, S, W] = [0, 1, 2, 3];
const MAX_PATH = 16;
const MONSTER_BASE = 4;
const WOLF = 0;
const PIERCING_THRUST = 0;
const BASE_ATTACK = 0xff;
/** `EMPTY_ITEM`, `ItemBase` and `EquipSlot` discriminants (crates/types). */
const EMPTY_ITEM = 0xff;
const [SWORD, LEATHER] = [0, 3];
const [MAIN_HAND, BODY] = [0, 3];
const worn = (on: Record<number, number>) => Array.from({ length: 11 }, (_, i) => on[i] ?? EMPTY_ITEM);

const pda = (seeds: (Buffer | Uint8Array)[], programId: PublicKey) => PublicKey.findProgramAddressSync(seeds, programId)[0];
const e = (name: string) => ({ [name]: {} });
const text = (s: string, len: number) => {
  const out = new Array(len).fill(0);
  Buffer.from(s, "ascii").forEach((b, i) => (out[i] = b));
  return out;
};
const explorer = (sig: string) => `https://explorer.solana.com/tx/${sig}?cluster=devnet`;
const cheb = (a: { x: number; y: number }, b: { x: number; y: number }) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const load = (n: string) => JSON.parse(readFileSync(new URL(`../target/idl/${n}.json`, import.meta.url), "utf8"));
const session: any = new Program(load("deadroll_session"), provider);
const characterProgram: any = new Program(load("deadroll_character"), provider);
const mapProgram: any = new Program(load("deadroll_map"), provider);
const itemsProgram: any = new Program(load("deadroll_items"), provider);
const sessionEvents = new EventParser(SESSION, session.coder);
const itemsEvents = new EventParser(ITEMS, itemsProgram.coder);

const config = pda([Buffer.from("config")], REGISTRY);
const sessionAuthority = pda([Buffer.from("authority")], SESSION);
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
async function logsOf(sig: string): Promise<string[]> {
  const tx = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  return tx?.meta?.logMessages ?? [];
}
/** The newest session event `name` (lowercase match) among the session's recent transactions. */
async function findSessionEvent(sessionPda: PublicKey, name: string, accept: (data: any) => boolean = () => true) {
  for (const { signature } of await connection.getSignaturesForAddress(sessionPda, { limit: 20 }, "confirmed")) {
    const ev = [...sessionEvents.parseLogs(await logsOf(signature))].find((x) => x.name.toLowerCase() === name && accept(x.data));
    if (ev) return { signature, data: ev.data };
  }
  return null;
}
async function waitVrf(sessionPda: PublicKey, what: string) {
  const deadline = Date.now() + 120_000;
  while ((await session.account.gameSession.fetch(sessionPda)).core.pendingVrf) {
    if (Date.now() > deadline) throw new Error(`no VRF callback for ${what} within 120 s: the oracle did not answer`);
    await sleep(2000);
  }
}

const dm = Keypair.generate();
const player = Keypair.generate();
const dmActor = Keypair.generate();
const playerActor = Keypair.generate();
await fund(dm.publicKey, 0.1);
await fund(player.publicKey, 0.15);

// character + starter kit in one transaction
const profile = pda([Buffer.from("profile"), player.publicKey.toBuffer()], CHARACTER);
await characterProgram.methods.createProfile().accountsStrict({ owner: player.publicKey, profile, systemProgram: SystemProgram.programId }).signers([player]).rpc();
const character = pda([Buffer.from("character"), player.publicKey.toBuffer(), Buffer.from([0])], CHARACTER);
const loadout = pda([Buffer.from("loadout"), character.toBuffer()], ITEMS);
const layer = { spriteId: 1, materialRamp: 2, accentRamp: 3, elementRamp: 0 };
const asset = Keypair.generate();
const initLoadout = await itemsProgram.methods
  .initLoadout()
  .accountsStrict({ owner: player.publicKey, character, loadout, systemProgram: SystemProgram.programId })
  .instruction();
const createSig = await characterProgram.methods
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
  .postInstructions([initLoadout])
  .signers([player, asset])
  .rpc({ commitment: "confirmed" });
console.log("character + Core NFT + loadout:", character.toBase58(), asset.publicKey.toBase58());
console.log("create_character + init_loadout:", explorer(createSig));
const created = [...itemsEvents.parseLogs(await logsOf(createSig))].find((x) => x.name === "loadoutCreated");
assert.ok(created, "LoadoutCreated in the creation transaction");
assert.equal(created.data.itemCount, 2);
const kit = async () => (await itemsProgram.account.loadout.fetch(loadout)).core;
let l = await kit();
assert.ok(l.items[0].base.sword !== undefined && l.items[1].base.leatherArmor !== undefined, "warrior kit: sword + leather armor");
assert.ok(l.equipped[MAIN_HAND].source.soulbound !== undefined && l.equipped[BODY].source.soulbound !== undefined, "kit worn");
console.log("starter kit: sword (main hand), leather armor (body)");

// unequip and equip back, signed by the owner
const item = (signer: Keypair) => ({ signer: signer.publicKey, character, loadout, sessionToken: null });
const offSig = await itemsProgram.methods.unequip(e("body")).accountsStrict(item(player)).signers([player]).rpc({ commitment: "confirmed" });
l = await kit();
assert.ok(l.equipped[BODY].source.empty !== undefined, "body empty after unequip");
const onSig = await itemsProgram.methods.equip(1, e("body")).accountsStrict(item(player)).signers([player]).rpc({ commitment: "confirmed" });
l = await kit();
assert.ok(l.equipped[BODY].source.soulbound !== undefined && l.equipped[BODY].index === 1, "leather armor worn again");
console.log("unequip body:", explorer(offSig));
console.log("equip body:  ", explorer(onSig));

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
const skillSet = pda([Buffer.from("skills"), character.toBuffer()], CHARACTER);
await session.methods
  .joinSession(PRICE, playerActor.publicKey)
  .accountsStrict({
    player: player.publicKey, session: sessionPda, character, participation,
    stash: pda([Buffer.from("stash"), sessionPda.toBuffer(), character.toBuffer()], SESSION),
    escrow, sessionAuthority, config, characterProgram: CHARACTER, systemProgram: SystemProgram.programId, loadout,
  })
  .signers([player])
  .rpc();
const part = () => session.account.participation.fetch(participation);
assert.deepEqual((await part()).equipment, worn({ [MAIN_HAND]: SWORD, [BODY]: LEATHER }), "join_session snapshot of the worn items");
console.log("join_session: equipment snapshot sword + leather armor");
await session.methods.setReady(true).accountsStrict({ player: player.publicKey, session: sessionPda, participation }).signers([player]).rpc();

const startSig = await session.methods
  .startSession()
  .accountsStrict({ dm: dm.publicKey, session: sessionPda, monsters, escrow, ...vrfAccounts })
  .signers([dm])
  .rpc({ commitment: "confirmed" });
console.log("start_session:", explorer(startSig));

// the real oracle answers the initiative request
await waitVrf(sessionPda, "initiative");
const init = await findSessionEvent(sessionPda, "initiativerolled");
assert.ok(init, "pendingVrf cleared but no InitiativeRolled event found");
console.log("InitiativeRolled:", explorer(init.signature));
console.log("order", JSON.stringify(init.data.order), "d20", JSON.stringify(init.data.d20));

// turns until the warrior (slot 0) attacks a wolf
const state = () => session.account.gameSession.fetch(sessionPda);
const monstersNow = async (): Promise<any[]> => {
  const m = (await session.account.monsterState.fetch(monsters)).core;
  return m.monsters.slice(0, m.count);
};
const wolves = preset.spawns.flatMap((s, i) => (s.bestiaryId === WOLF ? [i] : []));

/** Shortest steps from `from` to a cell where `goal` holds, over free passable cells (no traps). */
function route(from: { x: number; y: number }, goal: (x: number, y: number) => boolean, ms: any[]) {
  const taken = new Set<number>();
  for (const m of ms) if (m.alive) taken.add(m.y * MAP_STRIDE + m.x);
  const cost = (x: number, y: number) => {
    if (x < 0 || y < 0 || x >= preset.width || y >= preset.height) return 0;
    const t = preset.tiles[y * MAP_STRIDE + x];
    if (t === Tile.Wall || t === Tile.Trap || taken.has(y * MAP_STRIDE + x)) return 0;
    return t === Tile.Water ? 2 : 1;
  };
  const dirs = [[N, 0, -1], [E, 1, 0], [S, 0, 1], [W, -1, 0]];
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

const endTurn = (signer: Keypair, p: PublicKey | null) =>
  session.methods.endTurn().accountsStrict({ signer: signer.publicKey, session: sessionPda, monsters, participation: p, sessionToken: null }).signers([signer]).rpc();

for (let guard = 0; guard < 80; guard++) {
  const s = await state();
  assert.ok(s.core.status.active, "party still active");
  const unit = s.core.turnOrder[s.core.turnIndex];
  if (s.startedSeq !== s.turnSeq) {
    const pl = unit < MONSTER_BASE;
    await session.methods
      .startTurn()
      .accountsStrict({
        session: sessionPda, monsters, participation: pl ? participation : null, character: pl ? character : null,
        sessionAuthority, config, characterProgram: CHARACTER,
      })
      .rpc({ commitment: "confirmed" });
    continue;
  }
  if (unit >= MONSTER_BASE) {
    // The DM lets every monster pass: the smoke only needs the warrior's attack.
    await endTurn(dm, null);
    continue;
  }
  const ms = await monstersNow();
  const target = wolves.filter((i) => ms[i].alive).sort((a, b) => cheb(s.playerPos[0], ms[a]) - cheb(s.playerPos[0], ms[b]))[0];
  assert.ok(target !== undefined, "a living wolf");
  const p = await part();
  let ap = p.apSeq === s.turnSeq ? p.core.actionPoints : p.maxAp;
  if (cheb(s.playerPos[0], ms[target]) > 1) {
    const steps = route(s.playerPos[0], (x, y) => cheb({ x, y }, ms[target]) === 1, ms);
    const take: number[] = [];
    let spent = 0;
    for (const st of steps ?? []) {
      if (spent + st.cost > ap - 2 || take.length === MAX_PATH) break;
      take.push(st.dir);
      spent += st.cost;
    }
    if (take.length) {
      const padded = [...take];
      while (padded.length < MAX_PATH) padded.push(0);
      await session.methods
        .moveUnit(padded, take.length)
        .accountsStrict({ signer: player.publicKey, session: sessionPda, participation, monsters, layout, sessionToken: null })
        .signers([player])
        .rpc({ commitment: "confirmed" });
      ap -= spent;
    }
  }
  const now = (await state()).playerPos[0];
  if (ap >= 2 && cheb(now, ms[target]) === 1) {
    const slot = (await part()).cooldowns[PIERCING_THRUST] === 0 ? PIERCING_THRUST : BASE_ATTACK;
    const actSig = await session.methods
      .act(slot, target)
      .accountsStrict({
        signer: player.publicKey, session: sessionPda, participation, character, skillSet, monsters,
        sessionToken: null, ...vrfAccounts, targetParticipation: null,
      })
      .signers([player])
      .rpc({ commitment: "confirmed" });
    console.log(`act (${slot === BASE_ATTACK ? "weapon" : "Piercing Thrust"} on wolf ${target}):`, explorer(actSig));
    await waitVrf(sessionPda, "the attack");
    const hit = await findSessionEvent(sessionPda, "hitresolved", (d) => d.attacker === 0);
    assert.ok(hit, "pendingVrf cleared but no HitResolved event found");
    const r = hit.data.result;
    console.log("HitResolved:", explorer(hit.signature));
    console.log(`d20 ${r.d20}, ${Object.keys(r.outcome)[0]}, crit ${r.crit}, damage ${r.damage}, rolls ${r.rollsUsed}`);
    console.log("items", JSON.stringify(hit.data.items));
    process.exit(0);
  }
  await endTurn(player, participation);
}
throw new Error("the warrior never reached a wolf within 80 turns");
