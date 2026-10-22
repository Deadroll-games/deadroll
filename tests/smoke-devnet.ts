// Devnet smoke test for the character gear and the real MagicBlock VRF: a warrior created with its
// starter kit (create_character), unequip and equip, a party of two on a room whose boss pack stands
// on the entrance's diagonal (patch 6), set_ready with the 4 + 1 + 1 skills, start_session, then
// turns until the warrior's Bleeding Cleave hits the line chieftain, goblin warrior, shaman: the
// heaviest callback path (three hits, the monsters' auras, Battle Cry when the dice cross 50%).
// Then the chieftain throws daggers (a monster skill) at the second hero until it is down, and
// start_turn finalizes the queued death (patch 6).
// Prints the oracle's callback transactions and what each used of the compute budget the oracle
// gave it.
// Needs tools/scripts/deploy-devnet.ts. The session must be the real-VRF build: nothing here plays the oracle.
//
// cd tests && ANCHOR_PROVIDER_URL=<devnet rpc> ANCHOR_WALLET=<keypair.json> node --import tsx smoke-devnet.ts
// (DEADROLL_WS_URL overrides the websocket endpoint derived from the RPC.)
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import anchor from "@anchor-lang/core";
import { ComputeBudgetProgram, Connection, Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { randomBytes } from "node:crypto";
import { MAP_STRIDE, Tile } from "../tools/scripts/lib/demo-map.ts";
import { newSkill } from "./skill_ext.ts";
import { MPL_CORE_PROGRAM, PROGRAMS, VRF } from "@deadroll/sdk";

const { AnchorProvider, BN, EventParser, Program } = anchor;

const env = AnchorProvider.env();
const provider = new AnchorProvider(new Connection(env.connection.rpcEndpoint, { commitment: "confirmed", wsEndpoint: process.env.DEADROLL_WS_URL }), env.wallet, {
  commitment: "confirmed",
  preflightCommitment: "confirmed",
});
const connection = provider.connection;

const REGISTRY = new PublicKey(PROGRAMS.registry);
const CHARACTER = new PublicKey(PROGRAMS.character);
const MAP = new PublicKey(PROGRAMS.map);
const SESSION = new PublicKey(PROGRAMS.session);
const MPL_CORE = new PublicKey(MPL_CORE_PROGRAM);
const VRF_PROGRAM = new PublicKey(VRF.program);
const ORACLE_QUEUE = new PublicKey(VRF.oracleQueue);
const SLOT_HASHES = new PublicKey(VRF.slotHashes);

const PRICE = new BN(10_000_000);
const MONSTER_BASE = 4;
const COMPUTE_BUDGET = new PublicKey("ComputeBudget111111111111111111111111111111");
/** Compute limit of a `start_turn` that may finalize queued deaths (`character.mark_dead` each). */
const FINALIZE_CU = 600_000;
const BLEEDING_CLEAVE = 3;
const ENTRANCE = { x: 1, y: 1 };
const EXIT = { x: 14, y: 14 };
const [WOLF, GOBLIN, CHIEFTAIN, GOBLIN_WARRIOR, SHAMAN] = [0, 1, 2, 3, 4];
/** programs/session/tests/session.test.ts CAMP: monster indices wolves 0, 1; goblins 2, 3; chieftain 4; warrior 5; shaman 6. */
const CAMP = [
  { id: WOLF, x: 10, y: 3, pack: 1, skills: 0b0111 },
  { id: WOLF, x: 11, y: 3, pack: 1, skills: 0b1110 },
  { id: GOBLIN, x: 8, y: 8, pack: 2, skills: 0b11 },
  { id: GOBLIN, x: 9, y: 8, pack: 2, skills: 0b11 },
  { id: CHIEFTAIN, x: 2, y: 2, pack: 3, skills: 0b11111 },
  { id: GOBLIN_WARRIOR, x: 3, y: 3, pack: 3, skills: 0b11 },
  { id: SHAMAN, x: 4, y: 4, pack: 3, skills: 0b111 },
];
const CHIEFTAIN_INDEX = 4;
/** The chieftain's Dagger Throw: pool bit 2, the third active slot (`deadroll_rules::monster::select_skills`). */
const DAGGER_THROW_SLOT = 2;
/** Bleeding Cleave tries for the line with Battle Cry before the smoke goes on with the line alone. */
const CLEAVE_TRIES = 3;
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
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const load = (n: string) => JSON.parse(readFileSync(new URL(`../target/idl/${n}.json`, import.meta.url), "utf8"));
const session: any = new Program(load("deadroll_session"), provider);
const characterProgram: any = new Program(load("deadroll_character"), provider);
const mapProgram: any = new Program(load("deadroll_map"), provider);
const sessionEvents = new EventParser(SESSION, session.coder);
const characterEvents = new EventParser(CHARACTER, characterProgram.coder);

const config = pda([Buffer.from("config")], REGISTRY);
const bestiary = pda([Buffer.from("bestiary")], REGISTRY);
const monsterSkills = pda([Buffer.from("monster_skills")], REGISTRY);
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
const dmActor = Keypair.generate();
await fund(dm.publicKey, 0.12);

/** A warrior with its starter kit in one transaction (create_character gives it, worn). */
async function newHero(name: string) {
  const wallet = Keypair.generate();
  await fund(wallet.publicKey, 0.15);
  const profile = pda([Buffer.from("profile"), wallet.publicKey.toBuffer()], CHARACTER);
  await characterProgram.methods.createProfile().accountsStrict({ owner: wallet.publicKey, profile, systemProgram: SystemProgram.programId }).signers([wallet]).rpc();
  const character = pda([Buffer.from("character"), wallet.publicKey.toBuffer(), Buffer.from([0])], CHARACTER);
  const skillSet = pda([Buffer.from("skills"), character.toBuffer()], CHARACTER);
  const bag = pda([Buffer.from("bag"), character.toBuffer()], CHARACTER);
  const skillExtras = pda([Buffer.from("skill_extras"), character.toBuffer()], CHARACTER);
  const layer = { spriteId: 1, materialRamp: 2, accentRamp: 3, elementRamp: 0 };
  const asset = Keypair.generate();
  const createSig = await characterProgram.methods
    .createCharacter({
      name: text(name, 32), race: e("human"), class: e("warrior"), predisposition: e("piercing"),
      stats: [10, 7, 5, 8], appearance: new Array(20).fill(layer),
      skills: WARRIOR_SKILLS.map(([n, b]) => newSkill(n, b, text)),
    })
    .accountsStrict({
      owner: wallet.publicKey, profile, character, skillSet, bag,
      config, asset: asset.publicKey, mplCoreProgram: MPL_CORE, systemProgram: SystemProgram.programId, skillExtras,
    })
    .signers([wallet, asset])
    .rpc({ commitment: "confirmed" });
  return { wallet, actor: Keypair.generate(), character, skillSet, bag, skillExtras, asset, createSig, participation: PublicKey.default };
}
const hero = await newHero("Smoke");
const bait = await newHero("Bait");
const { wallet: player, character, skillSet, bag, skillExtras, asset, createSig } = hero;
console.log("character + Core NFT + starter kit:", character.toBase58(), asset.publicKey.toBase58());
console.log("create_character:", explorer(createSig));
const created = [...characterEvents.parseLogs(await logsOf(createSig))].find((x) => x.name === "characterCreated");
assert.ok(created, "CharacterCreated in the creation transaction");
assert.equal(created.data.itemCount, 2);
const kit = async () => (await characterProgram.account.character.fetch(character)).gear;
let l = await kit();
assert.ok(l.items[0].base.sword !== undefined && l.items[1].base.leatherArmor !== undefined, "warrior kit: sword + leather armor");
assert.ok(l.equipped[MAIN_HAND].source.soulbound !== undefined && l.equipped[BODY].source.soulbound !== undefined, "kit worn");
console.log("starter kit: sword (main hand), leather armor (body)");

// unequip and equip back, signed by the owner
const item = (signer: Keypair) => ({ signer: signer.publicKey, character, sessionToken: null });
const offSig = await characterProgram.methods.unequip(e("body")).accountsStrict(item(player)).signers([player]).rpc({ commitment: "confirmed" });
l = await kit();
assert.ok(l.equipped[BODY].source.empty !== undefined, "body empty after unequip");
const onSig = await characterProgram.methods.equip(1, e("body")).accountsStrict(item(player)).signers([player]).rpc({ commitment: "confirmed" });
l = await kit();
assert.ok(l.equipped[BODY].source.soulbound !== undefined && l.equipped[BODY].index === 1, "leather armor worn again");
console.log("unequip body:", explorer(offSig));
console.log("equip body:  ", explorer(onSig));

// map: a 16x16 room, the boss pack on the entrance's diagonal (the line through three goblins)
const index = randomBytes(4).readUInt32LE(0);
const idx = Buffer.alloc(4);
idx.writeUInt32LE(index);
const map = pda([Buffer.from("map"), dm.publicKey.toBuffer(), idx], MAP);
const layout = pda([Buffer.from("layout"), map.toBuffer()], MAP);
const edit = { dm: dm.publicKey, map, layout };
const tiles = new Array(15 * MAP_STRIDE + 16).fill(Tile.Wall);
for (let y = 1; y < 15; y++) for (let x = 1; x < 15; x++) tiles[y * MAP_STRIDE + x] = Tile.Floor;
tiles[ENTRANCE.y * MAP_STRIDE + ENTRANCE.x] = Tile.Entrance;
tiles[EXIT.y * MAP_STRIDE + EXIT.x] = Tile.Exit;
const spawns = CAMP.map((s) => ({ bestiaryId: s.id, x: s.x, y: s.y, level: 1, skills: s.skills, flags: 0, aspects: 0, pack: s.pack }));
await mapProgram.methods.createMap(index, 16, 16, 1).accountsStrict({ ...edit, systemProgram: SystemProgram.programId }).signers([dm]).rpc();
await mapProgram.methods.writeLayout(0, Buffer.from(tiles)).accountsStrict(edit).signers([dm]).rpc();
await mapProgram.methods.writeSpawns(ENTRANCE, EXIT, spawns).accountsStrict(edit).signers([dm]).rpc();
await mapProgram.methods.finalizeMap(new Array(32).fill(0)).accountsStrict({ ...edit, bestiary, monsterSkills }).signers([dm]).rpc();

// session
const sessionPda = pda([Buffer.from("session"), map.toBuffer()], SESSION);
const monsters = pda([Buffer.from("monsters"), sessionPda.toBuffer()], SESSION);
const escrow = pda([Buffer.from("escrow"), sessionPda.toBuffer()], SESSION);
await session.methods
  .openSession(PRICE, dmActor.publicKey)
  .accountsStrict({
    dm: dm.publicKey, map, layout, session: sessionPda, monsters, escrow, sessionAuthority, config,
    bestiary, tileset: pda([Buffer.from("tileset")], REGISTRY),
    mapProgram: MAP, systemProgram: SystemProgram.programId,
    aspects: pda([Buffer.from("aspects")], REGISTRY), traits: pda([Buffer.from("traits")], REGISTRY),
    resonances: pda([Buffer.from("resonances")], REGISTRY), monsterSkills,
  })
  .signers([dm])
  .rpc();
// Slot 0 the warrior who cleaves, slot 1 the hero the chieftain puts down.
const heroes = [hero, bait];
for (const h of heroes) {
  h.participation = pda([Buffer.from("participation"), sessionPda.toBuffer(), h.character.toBuffer()], SESSION);
  await session.methods
    .joinSession(PRICE, h.actor.publicKey)
    .accountsStrict({
      player: h.wallet.publicKey, session: sessionPda, character: h.character, participation: h.participation,
      stash: pda([Buffer.from("stash"), sessionPda.toBuffer(), h.character.toBuffer()], SESSION),
      escrow, systemProgram: SystemProgram.programId,
    })
    .signers([h.wallet])
    .rpc();
  // Ready: the 4 + 1 + 1 skills taken onto the map; the character is locked and its gear copied.
  await session.methods
    .setReady(true, [0, 1, 2, 3, 4, 5])
    .accountsStrict({
      player: h.wallet.publicKey, session: sessionPda, participation: h.participation, character: h.character,
      skillSet: h.skillSet, sessionAuthority, config, characterProgram: CHARACTER, skillExtras: h.skillExtras,
    })
    .signers([h.wallet])
    .rpc({ commitment: "confirmed" });
  const ready = await session.account.participation.fetch(h.participation);
  assert.deepEqual(ready.equipment, worn({ [MAIN_HAND]: SWORD, [BODY]: LEATHER }), "set_ready snapshot of the worn items");
}
const participation = hero.participation;
const part = () => session.account.participation.fetch(participation);
console.log("set_ready x2: equipment snapshot sword + leather armor, skills 4 + 1 + 1");

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

const state = () => session.account.gameSession.fetch(sessionPda);
/** Patch 6: `start_turn` finalizes the deaths the last callback queued; their accounts follow. */
const deathAccounts = (s: any) => {
  const out: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] = [];
  for (let slot = 0; slot < s.core.playerCount; slot++) {
    if (!(s.pendingDeaths & (1 << slot))) continue;
    const c = s.core.players[slot] as PublicKey;
    for (const pubkey of [c, pda([Buffer.from("skills"), c.toBuffer()], CHARACTER), pda([Buffer.from("bag"), c.toBuffer()], CHARACTER)]) {
      out.push({ pubkey, isSigner: false, isWritable: true });
    }
  }
  return out;
};
const endTurn = (signer: Keypair, p: PublicKey | null) =>
  session.methods.endTurn().accountsStrict({ signer: signer.publicKey, session: sessionPda, monsters, participation: p, sessionToken: null }).signers([signer]).rpc();

/** What the oracle's callback transaction used and the budget it ran under. */
async function callbackBudget(sig: string, what: string) {
  const tx = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  assert.ok(tx?.meta, "the callback transaction is readable");
  console.log(`callback tx (${what}):`, explorer(sig));
  console.log("computeUnitsConsumed (whole tx):", tx.meta.computeUnitsConsumed);
  const own = `Program ${SESSION.toBase58()} consumed `;
  for (const line of tx.meta.logMessages ?? []) if (line.startsWith(own)) console.log("session program:", line.slice(`Program ${SESSION.toBase58()} `.length));
  // ComputeBudget instructions the oracle put in front of the callback (none: the 200k default per instruction).
  const msg = tx.transaction.message;
  const keys = msg.staticAccountKeys;
  let budgets = 0;
  for (const ix of msg.compiledInstructions) {
    if (!keys[ix.programIdIndex]?.equals(COMPUTE_BUDGET)) continue;
    budgets++;
    const data = Buffer.from(ix.data);
    if (data[0] === 2) console.log("oracle SetComputeUnitLimit:", data.readUInt32LE(1));
    else if (data[0] === 3) console.log("oracle SetComputeUnitPrice (micro-lamports):", data.readBigUInt64LE(1).toString());
    else console.log("oracle ComputeBudget instruction", data[0]);
  }
  if (!budgets) console.log("oracle sent no ComputeBudget instruction");
}

await callbackBudget(init.signature, "initiative");

const monsterState = () => session.account.monsterState.fetch(monsters);
const named = (events: any[], name: string) => events.filter((x) => x.name.toLowerCase() === name);
const BAIT_SLOT = 1;
let cleaves = 0;
let cried = false;
let daggerShown = false;
let actedSeq = -1;

// The warrior (slot 0, on the entrance) cleaves the line chieftain, goblin warrior, shaman, until
// Battle Cry fires or CLEAVE_TRIES; then the chieftain throws daggers at the bait (slot 1) until
// it is down, and start_turn finalizes the death. The DM lets every other monster pass.
for (let guard = 0; guard < 400; guard++) {
  const s = await state();
  assert.ok(s.core.status.active, "party still active");
  const unit = s.core.turnOrder[s.core.turnIndex];
  if (s.startedSeq !== s.turnSeq || s.pendingDeaths !== 0) {
    const queued = s.pendingDeaths as number;
    const h = unit < MONSTER_BASE ? heroes[unit] : null;
    const sig = await session.methods
      .startTurn()
      .accountsStrict({
        session: sessionPda, monsters, participation: h?.participation ?? null, character: h?.character ?? null,
        sessionAuthority, config, characterProgram: CHARACTER, skillSet: h?.skillSet ?? null, bag: h?.bag ?? null,
        payer: provider.wallet.publicKey, ...vrfAccounts,
      })
      .remainingAccounts(deathAccounts(s))
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: FINALIZE_CU })])
      .rpc({ commitment: "confirmed" });
    if (queued) {
      const tx = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
      const fin = named([...sessionEvents.parseLogs(tx?.meta?.logMessages ?? [])], "deathfinalized");
      assert.deepEqual(fin.map((d) => d.data.slot), [BAIT_SLOT], "start_turn finalized the queued death");
      assert.equal((await state()).pendingDeaths, 0, "death queue empty");
      assert.equal((await characterProgram.account.character.fetch(bait.character)).core.alive, false, "tombstone through character.mark_dead");
      console.log(`start_turn finalized the death of slot ${BAIT_SLOT} (${tx?.meta?.computeUnitsConsumed} CU):`, explorer(sig));
      process.exit(0);
    }
    if ((await state()).core.pendingVrf) await waitVrf(sessionPda, "a reactive roll");
    continue;
  }
  const acted = actedSeq === s.turnSeq;
  if (unit >= MONSTER_BASE) {
    const mi = unit - MONSTER_BASE;
    const baitUp = (s.aliveMask & (1 << BAIT_SLOT)) !== 0;
    if (acted || cleaves === 0 || mi !== CHIEFTAIN_INDEX || !baitUp) {
      await endTurn(dm, null);
      continue;
    }
    const cd = (await monsterState()).skillCooldowns[mi];
    const action = cd[DAGGER_THROW_SLOT] === 0 ? 1 + DAGGER_THROW_SLOT : 0;
    const actSig = await session.methods
      .monsterAct(mi, action, BAIT_SLOT)
      .accountsStrict({
        signer: dm.publicKey, session: sessionPda, monsters, participation: bait.participation, character: bait.character,
        skillSet: bait.skillSet, bag: bait.bag, sessionToken: null, ...vrfAccounts, layout,
      })
      .signers([dm])
      .rpc({ commitment: "confirmed" });
    actedSeq = s.turnSeq;
    const what = action ? "Dagger Throw" : "natural attack";
    console.log(`monster_act (chieftain, ${what} on slot ${BAIT_SLOT}):`, explorer(actSig));
    await waitVrf(sessionPda, `the chieftain's ${what}`);
    const hit = await findSessionEvent(sessionPda, "hitresolved", (d) => d.attacker === MONSTER_BASE + mi);
    assert.ok(hit, "pendingVrf cleared but no HitResolved event found");
    const events = [...sessionEvents.parseLogs(await logsOf(hit.signature))];
    const r = named(events, "hitresolved")[0].data.result;
    const died = named(events, "playerdied").map((d) => d.data.slot);
    console.log(`  d20 ${r.d20}, ${Object.keys(r.outcome)[0]}, damage ${r.damage}${died.length ? `, down: slot ${died.join(", ")}` : ""}`);
    if (action && !daggerShown) {
      daggerShown = true;
      await callbackBudget(hit.signature, "monster skill: Dagger Throw");
    }
    if (died.length) {
      assert.deepEqual(died, [BAIT_SLOT]);
      assert.equal(named(events, "characterdied").length, 0, "no mark_dead in the callback");
      const after = await state();
      assert.ok(after.pendingDeaths & (1 << BAIT_SLOT), "the death is queued");
      if (!daggerShown) await callbackBudget(hit.signature, "the killing hit");
      console.log(`slot ${BAIT_SLOT} down; the death waits for start_turn`);
    }
    continue;
  }
  const h = heroes[unit];
  if (unit !== 0 || acted || (cleaves > 0 && (cried || cleaves >= CLEAVE_TRIES))) {
    await endTurn(h.wallet, h.participation);
    continue;
  }
  const p = await part();
  const ap = p.apSeq === s.turnSeq ? p.core.actionPoints : p.maxAp;
  const m = await monsterState();
  const lineUp = [CHIEFTAIN_INDEX, CHIEFTAIN_INDEX + 1, CHIEFTAIN_INDEX + 2].every((i) => m.core.monsters[i].alive);
  if (p.cooldowns[BLEEDING_CLEAVE] !== 0 || ap < p.mapSkills[BLEEDING_CLEAVE].apCost || !lineUp) {
    if (!lineUp && cleaves > 0) cleaves = CLEAVE_TRIES;
    await endTurn(player, participation);
    continue;
  }
  const actSig = await session.methods
    .act(BLEEDING_CLEAVE, CHIEFTAIN_INDEX)
    .accountsStrict({
      signer: player.publicKey, session: sessionPda, participation, character, skillSet, bag, monsters,
      sessionToken: null, ...vrfAccounts, targetParticipation: null, layout,
    })
    .signers([player])
    .rpc({ commitment: "confirmed" });
  actedSeq = s.turnSeq;
  cleaves++;
  console.log(`act ${cleaves} (Bleeding Cleave through chieftain, goblin warrior, shaman):`, explorer(actSig));
  await waitVrf(sessionPda, "the cleave");
  const hit = await findSessionEvent(sessionPda, "hitresolved", (d) => d.attacker === 0);
  assert.ok(hit, "pendingVrf cleared but no HitResolved event found");
  const events = [...sessionEvents.parseLogs(await logsOf(hit.signature))];
  const hits = named(events, "hitresolved");
  console.log(`hits: ${hits.length} (targets ${hits.map((x) => x.data.target).join(", ")})`);
  for (const x of hits) console.log(`  d20 ${x.data.result.d20}, ${Object.keys(x.data.result.outcome)[0]}, damage ${x.data.result.damage}`);
  cried = named(events, "monsterreactivetriggered").length > 0;
  console.log(cried ? "Battle Cry fired (the heaviest path)" : "Battle Cry did not fire on these dice (the line alone)");
  await callbackBudget(hit.signature, `cleave ${cleaves}`);
}
throw new Error("the bait was never finalized within 400 steps");
