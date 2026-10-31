// @vitest-environment node
// The demo script through the app's own builders (game.ts) against the shared local validator,
// the same flow as tests/demo.test.ts, with the turn rules (start_turn, one attack per turn), and
// the support skills of PR #25 (buffs on allies, debuffs on monsters).
// Skipped unless DEADROLL_LOCALNET=1 (needs deploy-local).
//   DEADROLL_LOCALNET=1 pnpm --filter @deadroll/app exec vitest run src/chain/localnet.e2e.test.ts
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { generateKeyPairSigner, getAddressEncoder, lamports, signBytes, type Address, type KeyPairSigner } from "@solana/kit";
import { isBoss, mapKeys, type MessageSigner } from "./secrets";
import demoMap from "../../../assets/registry/maps/demo_16x16.json";
import { chestsOf } from "./loot";
import { PROGRAMS } from "../config";
import { client } from "../wallet/client";
import { createSessionInstruction } from "../session-key/magicblock";
import { FEES, validUntilFromNow } from "../session-key/sessionKey";
import { decodeEvents, type ChainEvent } from "./events";
import {
  BASE_ATTACK,
  DEFAULT_KEY,
  MONSTER_BASE,
  actIx,
  buildMapIxs,
  chainTiles,
  mapStatus,
  mapTiles,
  registryBestiary,
  registryMonsterSkills,
  registryRewards,
  registryTileset,
  closeCacheIx,
  computeLimitIx,
  settlePartyIx,
  deathAccountsOf,
  withDeaths,
  consumeIx,
  characterSteps,
  skillExtrasOf,
  SKILL_PRESETS,
  currentUnit,
  endTurnIx,
  freezeTombstoneIx,
  joinSessionIx,
  monsterActIx,
  movePathIx,
  openSessionIx,
  setSessionKeyIx,
  partyAccountsOf,
  pickSeed,
  playerAp,
  readParty,
  sessionAccounts,
  getAccount,
  setReadyIx,
  optional,
  startSessionIx,
  startTurnIx,
  status,
  type Actor,
  type Party,
  arenaMode,
  enterArenaIx,
  leaveArenaIx,
  visiblePacks,
  openDmSecrets,
  revealCellIx,
  revealPackIx,
} from "./game";
import { MAP_PRESETS, presetToLayout, type MapPresetData } from "./mock";
import { equipIx, equipmentOf, inventory, unequipIx } from "./items";
import { skillSetPda } from "./pda";
import { affordable, cheb, route } from "./route";
import { batchCost, errorName, sendBatch, sendTx } from "./send";
import { MapcheckWasm } from "./mapcheckWasm";
import { toHitResolved } from "./hit";
import { Category, Effect, defaultMapSelection, type NamedSkill } from "./skillcheck";
import { activeMods, toEffectResolved } from "./support";
import { auditLeft, revealStepFor, turnStarted } from "./turns";
import { Applied, Aspect, Class, HitOutcome, MAP_MAX, ModKind, Race, Tile, cellByte } from "./types";

const ON = process.env.DEADROLL_LOCALNET === "1";
const WARRIOR = 0;
const MAGE = 1;
const PRICE = 100_000_000n;

async function funded(sol = 10): Promise<KeyPairSigner> {
  const kp = await generateKeyPairSigner();
  const sig = await client.rpc.requestAirdrop(kp.address, lamports(BigInt(sol) * 1_000_000_000n), { commitment: "confirmed" }).send();
  for (let i = 0; i < 60; i++) {
    const { value } = await client.rpc.getSignatureStatuses([sig]).send();
    if (value[0]?.confirmationStatus === "confirmed" || value[0]?.confirmationStatus === "finalized") break;
    await new Promise((r) => setTimeout(r, 300));
  }
  return kp;
}

async function gearOf(character: Address) {
  return (await getAccount("Character", character))!.gear;
}

/** A full map selection of the character's skills: the first of each kind, in SkillSet order. */
async function mapSelection(character: Address): Promise<number[]> {
  const set = (await getAccount("SkillSet", await skillSetPda(character)))!;
  return defaultMapSelection(set.skills.slice(0, set.count).map((k) => k.blocks.category as Category));
}

/** Patch C: a key pair as the wallet's message signer (the DM's unlock of the map secrets). */
const messageSigner = (k: KeyPairSigner): MessageSigner => ({ address: k.address, signMessage: async (m) => new Uint8Array(await signBytes(k.keyPair.privateKey, m)) });
/** Patch C: `buildMapIxs`'s keys of a DM key pair. */
const keysOf = (dm: KeyPairSigner) => (map: Address) => mapKeys(messageSigner(dm), new Uint8Array(getAddressEncoder().encode(map)));

async function send(payer: KeyPairSigner, ixs: Parameters<typeof sendTx>[1]): Promise<ChainEvent[]> {
  const { logs } = await sendTx(payer, ixs);
  return decodeEvents(logs);
}

type Spec = { cls: Class; race: Race; pred: Aspect; stats: number[]; name: string; skills?: NamedSkill[] };
type Player = { wallet: KeyPairSigner; key: KeyPairSigner; character: Address; asset: Address; actor: Actor };

/** Steps 1-3 of the demo: characters, the map from the preset, the party started and initiative rolled. */
async function openParty(specs: Spec[]) {
  {
    const preset = demoMap as MapPresetData & { tier: number };
    const [dm, dmKey] = await Promise.all([funded(), funded(1)]);
    const players: Player[] = [];

    // 1. characters (Core NFT)
    for (const { cls, race, pred, stats, name, skills } of specs) {
      const [wallet, key] = await Promise.all([funded(), funded(1)]);
      // One user action, one signature: profile, character and descriptions in one batch, at the
      // price the site shows before signing.
      const c = await characterSteps(wallet, { name, race, class: cls, predisposition: pred, stats: [...stats], skills });
      const price = await batchCost(wallet, c.steps);
      const before = (await client.rpc.getBalance(wallet.address, { commitment: "confirmed" }).send()).value;
      const sentSteps = await sendBatch(wallet, c.steps);
      const after = (await client.rpc.getBalance(wallet.address, { commitment: "confirmed" }).send()).value;
      expect(before - after).toBe(price);
      const evs = sentSteps.flatMap((x) => decodeEvents(x.logs));
      const created = evs.find((e) => e.name === "CharacterCreated")!.data;
      const named = skills ?? SKILL_PRESETS[cls];
      expect((await skillExtrasOf(c.character, named.length))!.map((x) => x.description)).toEqual(named.map((k) => k.description));
      expect(optional(created.nft)).toBe(c.asset.address);
      // The class starter items come with the character, worn; the owner takes one off and back on.
      expect(created.itemCount).toBe(2);
      expect(inventory((await gearOf(c.character))).items.every((i) => i.worn !== null)).toBe(true);
      const off = await send(wallet, [await unequipIx(wallet, c.character, "body", null)]);
      expect(off.find((e) => e.name === "ItemUnequipped")).toBeTruthy();
      await send(wallet, [await equipIx(wallet, c.character, 1, "body", null)]);
      players.push({ wallet, key, character: c.character, asset: c.asset.address, actor: { signer: key, token: null } });
    }

    // 2. map from the preset
    const layout = presetToLayout(preset);
    const built = await buildMapIxs(dm, { width: preset.width, height: preset.height, tier: preset.tier, ...layout }, keysOf(dm), isBoss);
    // Five transactions, one signature, sent in order (MapBuilder's publish).
    await sendBatch(dm, built.ixs.map((ix) => [ix]));
    const acc = await sessionAccounts(built.map);

    // 3. party: open with price, pay half each, session keys, ready, start
    // One main wallet transaction each, as the site sends them: the key's MagicBlock token and
    // prepayment with open_session (DM) or, for a player, at Ready (docs/PARTY_AND_TURNS.md 2.1).
    const keyIx = (authority: typeof dm, key: typeof dmKey) =>
      createSessionInstruction({ authority, key, targetProgram: PROGRAMS.session, validUntil: BigInt(validUntilFromNow()), prepayLamports: FEES.prepay });
    const dmTok = await keyIx(dm, dmKey);
    await send(dm, [dmTok.instruction, await openSessionIx(dm, built.map, PRICE, dmKey.address)]);
    const dmActor: Actor = { signer: dmKey, token: dmTok.token };
    // Joining pays the share of the price only.
    for (const pl of players) await send(pl.wallet, [await joinSessionIx(pl.wallet, acc.session, pl.character, PRICE / 2n, DEFAULT_KEY)]);
    // Ready: the key's token and prepayment, its binding, and the skills onto the map (the first of
    // each kind); the character is locked.
    for (const pl of players) {
      const t = await keyIx(pl.wallet, pl.key);
      await send(pl.wallet, [
        t.instruction,
        await setSessionKeyIx(pl.wallet, acc.session, pl.character, pl.key.address),
        await setReadyIx(pl.wallet, acc.session, pl.character, true, await mapSelection(pl.character)),
      ]);
      pl.actor.token = t.token;
    }
    await send(dm, [await startSessionIx(dm, acc.session)]);

    const party = (await readParty(acc.session))!;
    expect(status(party.session)).toBe("active");
    // set_ready copied what each character wears.
    for (const pl of party.players) expect(Array.from(pl.part.equipment)).toEqual(equipmentOf(await gearOf(pl.character)));
    expect(party.session.core.pendingVrf).toBe(true);
    // The localnet oracle: the other living players ride along (patch 5: reactive skills, lines,
    // a buff on an ally). A death in the callback only queues (patch 6).
    const oracle = async (seed: Uint8Array): Promise<ChainEvent[]> => {
      const p = (await readParty(acc.session, party.tiles))!;
      const pendingPart = p.session.pending.participation;
      const pl = p.players.find((x) => x.participation === pendingPart)!;
      return send(dmKey, [await consumeIx(acc.session, pendingPart, pl.character, seed, p.session.layout, partyAccountsOf(p.session, pendingPart))]);
    };
    /** Patch 6: anyone sends start_turn to finalize the deaths a callback queued (with the
     * current player's accounts when the turn has not started). */
    const finalize = async (payer: KeyPairSigner): Promise<ChainEvent[]> => {
      const p = (await readParty(acc.session, party.tiles))!;
      const unit = currentUnit(p.session)!;
      const pl = unit < MONSTER_BASE ? p.players.find((x) => x.slot === unit)! : null;
      const deaths = await deathAccountsOf(p.session);
      return send(payer, withDeaths(deaths, await startTurnIx(acc.session, pl ? { participation: pl.participation, character: pl.character } : null, payer, deaths)));
    };
    const init = await oracle(await pickSeed("random"));
    expect(init.some((e) => e.name === "InitiativeRolled")).toBe(true);
    // Patch C: the DM's client opens the map secrets with the DM's wallet and reveals by itself,
    // with the DM's session key, what the party uncovers (the site's PartyChain loop).
    const secrets = await openDmSecrets(party, messageSigner(dm));
    const reveal = async (p: Party): Promise<boolean> => {
      const step = revealStepFor(p, p.session.core.dm);
      if (!step) return false;
      if (step.kind === "cell") {
        const by = p.players.find((x) => x.slot === step.by)?.participation ?? null;
        await send(dmKey, [await revealCellIx(dmActor, acc.session, p.session, step.leaf, await secrets.commitment.cell(step.leaf), by)]);
      } else {
        await send(dmKey, await revealPackIx(dmActor, acc.session, p.session, step.pack, await secrets.commitment.pack(step.pack)));
      }
      return true;
    };
    /** Patch C: the final audit, with the DM's session key. */
    const audit = async () => {
      const p = (await readParty(acc.session))!;
      const left = auditLeft(p.session, secrets.leaves, secrets.packs);
      for (const leaf of left.cells) await send(dmKey, [await revealCellIx(dmActor, acc.session, p.session, leaf, await secrets.commitment.cell(leaf), null)]);
      for (const pack of left.packs) await send(dmKey, await revealPackIx(dmActor, acc.session, p.session, pack, await secrets.commitment.pack(pack)));
    };
    /** Patch C: every reveal the DM owes now (what the site's loop sends between actions). */
    const revealAll = async () => {
      while (await reveal((await readParty(acc.session))!));
    };
    return { preset, dm, dmKey, dmActor, players, built, acc, oracle, finalize, party, reveal, revealAll, audit };
  }
}

/** `toward`: on the map, the pack the walk goes to; the cells next to every other pack's monster
 * are kept out (stepping there engages it). */
const taken = (p: Party, moverUnit: number, toward = -1) => {
  const t = new Set<number>();
  p.monsters.forEach((m, i) => m.alive && MONSTER_BASE + i !== moverUnit && t.add(m.y * MAP_MAX + m.x));
  p.players.forEach((pl) => p.session.aliveMask & (1 << pl.slot) && pl.slot !== moverUnit && t.add(pl.part.core.y * MAP_MAX + pl.part.core.x));
  // Patch 11: a closed chest occupies its cell; arena update: on the map only, with the packs.
  if (arenaMode(p.session) === "map") {
    for (const c of chestsOf(p.session)) if (!c.opened) t.add(c.y * MAP_MAX + c.x);
    for (const v of visiblePacks(p)) {
      t.add(v.y * MAP_MAX + v.x);
      if (v.pack !== toward) for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) t.add((v.y + dy) * MAP_MAX + v.x + dx);
    }
  }
  return t;
};
const tileCost = (p: Party) => Array.from(p.session.tileCost);

/** Arena update: the first living player walks the map next to pack `pack`'s monster (the others
 * pass), `enter_arena` follows; returns the party in the arena. */
async function enterPack(opened: Awaited<ReturnType<typeof openParty>>, pack: number): Promise<Party> {
  const { acc, players, dmKey, oracle, finalize, reveal, revealAll } = opened;
  for (let guard = 0; guard < 100; guard++) {
    const party = (await readParty(acc.session))!;
    const s = party.session;
    if (s.core.pendingVrf) {
      await oracle(await pickSeed("miss"));
      continue;
    }
    // Patch C: the DM's reveals come first (the next turn waits for them).
    if (await reveal(party)) continue;
    if (s.pendingDeaths) {
      await finalize(dmKey);
      continue;
    }
    const mode = arenaMode(s);
    if (mode === "arena") return party;
    if (mode === "engaged") {
      await send(dmKey, await enterArenaIx(acc.session, s, dmKey));
      continue;
    }
    const unit = currentUnit(s)!;
    const pl = players[unit]!;
    const me = party.players[unit]!;
    if (!turnStarted(s)) {
      await send(pl.key, [await startTurnIx(acc.session, { participation: me.participation, character: me.character }, pl.key)]);
      continue;
    }
    const walker = party.players.find((x) => s.aliveMask & (1 << x.slot))!.slot;
    if (unit === walker) {
      const target = s.packs[pack]!;
      const from = { x: me.part.core.x, y: me.part.core.y };
      const steps = route({ from, goal: (x, y) => cheb({ x, y }, target) === 1, tiles: party.ground!, width: s.width, height: s.height, tileCost: tileCost(party), taken: taken(party, unit, pack) });
      const take = steps ? affordable(steps, playerAp(me.part, s), 0) : [];
      if (take.length) await send(pl.key, [await movePathIx(pl.actor, acc.session, s.layout, { kind: "player", slot: unit, character: pl.character }, take)]);
              await revealAll();
    }
    if (arenaMode((await readParty(acc.session))!.session) === "map") await send(pl.key, [await endTurnIx(pl.actor, acc.session, pl.character)]);
  }
  throw new Error(`pack ${pack} never engaged`);
}
/**
 * Patch 5: a monster's walk stops on the first cell next to a Cold Snap bearer and asks for a
 * reactive roll; the oracle answers it. Returns the AP the monster has left.
 */
const afterMonsterMove = async (session: Address, tiles: Uint8Array, mi: number, oracle: (seed: Uint8Array) => Promise<ChainEvent[]>) => {
  const p = (await readParty(session, tiles))!;
  if (p.session.core.pendingVrf) await oracle(await pickSeed("miss"));
  return (await readParty(session, tiles))!.monsters[mi]!.actionPoints;
};

describe.skipIf(!ON)("localnet: the demo through the app builders", () => {
  it("plays create -> map -> party -> fight -> death -> victory -> claims", { timeout: 15 * 60_000 }, async () => {
    const { preset, dm, dmKey, dmActor, players, acc, oracle, finalize, party: first, reveal, revealAll, audit } = await openParty([
      { cls: Class.Warrior, race: Race.Human, pred: Aspect.Piercing, stats: [10, 7, 5, 8], name: "Bran" },
      { cls: Class.Mage, race: Race.ForestElf, pred: Aspect.Cold, stats: [5, 7, 13, 5], name: "Ilyra" },
    ]);
    let party = first;

    // 4-7. combat: wolves go for the mage, the mage casts Frost Burn (resonant seed), the warrior hunts.
    // Arena update: on the map the warrior walks next to the wolves' monster; the party fights them
    // in their arena, comes back, and walks to the boss pack. Arena slots: the pack's spawns in order.
    const tiles = party.tiles!;
    const [WOLF_PACK, BOSS_PACK] = [1, 3];
    const slots = (pack: number) => preset.spawns.filter((x) => x.pack === pack);
    const wolves = slots(WOLF_PACK).flatMap((x, i) => (x.bestiary_id === 0 ? [i] : []));
    const boss = slots(BOSS_PACK).findIndex((x) => x.bestiary_id === 2);
    let goal = WOLF_PACK;
    const arenas: string[] = [];
    let resonance = false;
    let mageDied = false;
    let victory = false;
    let hits = 0;
    let ticks = 0;
    let checkedNotStarted = false;
    let checkedActed = false;
    for (let guard = 0; guard < 1000 && !victory; guard++) {
      party = (await readParty(acc.session, tiles))!;
      const s = party.session;
      if (status(s) !== "active") break;
      // Patch 5: a reactive-only roll (a wolf stepping next to the mage's Cold Snap, over-time
      // damage under a threshold) waits for the oracle first.
      if (s.core.pendingVrf) {
        await oracle(await pickSeed("miss"));
        continue;
      }
      // Patch C: the DM's reveals come first (the next turn waits for them).
      if (await reveal(party)) continue;
      if (s.pendingDeaths) {
        await finalize(dmKey);
        continue;
      }
      // Arena update: the steps between the map and an arena go out by themselves (any key).
      const mode = arenaMode(s);
      if (mode === "engaged") {
        const evs = await send(dmKey, await enterArenaIx(acc.session, s, dmKey));
        arenas.push(`enter ${evs.find((e) => e.name === "ArenaEntered")!.data.pack}`);
        continue;
      }
      if (mode === "won") {
        const evs = await send(dmKey, await leaveArenaIx(acc.session, s, dmKey));
        arenas.push(`leave ${evs.find((e) => e.name === "ArenaLeft")!.data.pack}`);
        goal = BOSS_PACK;
        continue;
      }
      const unit = currentUnit(s)!;
      // Every turn begins with start_turn, sent alone by whoever plays the unit (the app's rule).
      if (!turnStarted(s)) {
        const pl = unit < MONSTER_BASE ? party.players[unit]! : null;
        const payer = pl ? players[unit]!.key : dmKey;
        if (!checkedNotStarted && pl) {
          await expect(send(payer, [await endTurnIx(players[unit]!.actor, acc.session, pl.character)])).rejects.toThrow(/TurnNotStarted/);
          checkedNotStarted = true;
        }
        const evs = await send(payer, [await startTurnIx(acc.session, pl ? { participation: pl.participation, character: pl.character } : null, payer)]);
        const tick = evs.find((e) => e.name === "TurnTicked")!.data;
        expect([tick.unit, tick.turnSeq]).toEqual([unit, s.turnSeq]);
        ticks++;
        // A skip or a DoT death hands the turn on: the next unit needs its own start.
        continue;
      }
      const mageAlive = (s.aliveMask & (1 << MAGE)) !== 0;
      const inWolves = mode === "arena" && s.arenaPack === WOLF_PACK;
      const livingWolves = inWolves ? wolves.filter((i) => party.monsters[i]!.alive) : [];
      if (mode === "map") {
        // The first living player walks to the goal pack; the others pass.
        const pl = players[unit]!;
        const walker = [WARRIOR, MAGE].find((x) => s.aliveMask & (1 << x));
        const target = s.packs[goal]!;
        if (unit === walker) {
          const me = party.players[unit]!;
          const from = { x: me.part.core.x, y: me.part.core.y };
          const steps = route({ from, goal: (x, y) => cheb({ x, y }, target) === 1, tiles: party.ground!, width: s.width, height: s.height, tileCost: tileCost(party), taken: taken(party, unit, goal) });
          const take = steps ? affordable(steps, playerAp(me.part, s), 0) : [];
          if (take.length) await send(pl.key, [await movePathIx(pl.actor, acc.session, s.layout, { kind: "player", slot: unit, character: pl.character }, take)]);
              await revealAll();
        }
        if (arenaMode((await readParty(acc.session, tiles))!.session) === "map") await send(pl.key, [await endTurnIx(pl.actor, acc.session, pl.character)]);
        continue;
      }
      if (unit < MONSTER_BASE) {
        const pl = players[unit]!;
        const me = party.players[unit]!;
        let ap = playerAp(me.part, s);
        const pos = { x: me.part.core.x, y: me.part.core.y };
        if (unit === MAGE && !resonance && livingWolves.length && cheb(pos, party.monsters[livingWolves[0]!]!) <= 6) {
          const used = (await send(pl.key, [await actIx(pl.actor, acc.session, pl.character, 1, livingWolves[0]!, null, s.layout)])).find((e) => e.name === "SkillUsed")!.data;
          expect(used.skillSlot).toBe(1);
          const after = (await readParty(acc.session, tiles))!;
          expect(Array.from(used.cooldowns as Uint8Array)).toEqual(Array.from(after.players[MAGE]!.part.cooldowns));
          expect(after.session.actedSeq).toBe(after.session.turnSeq);
          const evs = await oracle(await pickSeed("resonant"));
          const hit = toHitResolved(evs.find((e) => e.name === "HitResolved")!.data);
          hits++;
          resonance = hit.result.resonances.some((r) => r?.active);
          expect(resonance).toBe(true);
        } else if (unit === WARRIOR && (!mageAlive || livingWolves.length === 0 || !inWolves)) {
          const targets = inWolves ? livingWolves : [boss];
          const target = targets[0]!;
          const m = party.monsters[target]!;
          if (cheb(pos, m) > 1) {
            const steps = route({ from: pos, goal: (x, y) => cheb({ x, y }, m) === 1, tiles: party.ground!, width: s.width, height: s.height, tileCost: tileCost(party), taken: taken(party, unit) });
            const take = steps ? affordable(steps, ap, 2) : [];
            if (take.length) {
              await send(pl.key, [await movePathIx(pl.actor, acc.session, s.layout, { kind: "player", slot: unit, character: pl.character }, take)]);
              await revealAll();
              ap -= take.reduce((n, st) => n + st.cost, 0);
            }
          }
          party = (await readParty(acc.session, tiles))!;
          const now = party.players[unit]!.part.core;
          // One attack per turn: Piercing Thrust off cooldown, else the weapon (no cooldown).
          if (ap >= 2 && cheb(now, party.monsters[target]!) === 1 && party.monsters[target]!.alive) {
            const skill = party.players[unit]!.part.cooldowns[0] === 0 ? 0 : BASE_ATTACK;
            await send(pl.key, [await actIx(pl.actor, acc.session, pl.character, skill, target, null, s.layout)]);
            const evs = await oracle(await pickSeed("crit"));
            hits++;
            if (evs.some((e) => e.name === "SessionEnded")) victory = true;
            if (!victory && !checkedActed) {
              await expect(send(pl.key, [await actIx(pl.actor, acc.session, pl.character, BASE_ATTACK, target, null, s.layout)])).rejects.toThrow(/AlreadyActed/);
              checkedActed = true;
            }
          }
        }
        if (!victory && arenaMode((await readParty(acc.session, tiles))!.session) === "arena") await send(pl.key, [await endTurnIx(pl.actor, acc.session, pl.character)]);
      } else {
        const mi = unit - MONSTER_BASE;
        const m = party.monsters[mi]!;
        const mage = party.players[MAGE]!;
        if (inWolves && wolves.includes(mi) && m.alive && mageAlive) {
          const target = { x: mage.part.core.x, y: mage.part.core.y };
          let ap = m.actionPoints;
          if (cheb(m, target) > 1) {
            const steps = route({ from: m, goal: (x, y) => cheb({ x, y }, target) === 1, tiles: party.ground!, width: s.width, height: s.height, tileCost: tileCost(party), taken: taken(party, unit) });
            const take = steps ? affordable(steps, ap, 2) : [];
            if (take.length) {
              await send(dmKey, [await movePathIx(dmActor, acc.session, s.layout, { kind: "monster", index: mi }, take)]);
              ap = await afterMonsterMove(acc.session, tiles, mi, oracle);
            }
          }
          party = (await readParty(acc.session, tiles))!;
          if (ap >= 2 && cheb(party.monsters[mi]!, target) === 1) {
            await send(dmKey, [await monsterActIx(dmActor, acc.session, mi, 0, MAGE, mage.character, s.layout)]);
            const evs = await oracle(await pickSeed("crit"));
            hits++;
            if (evs.some((e) => e.name === "PlayerDied")) {
              mageDied = true;
              // Patch 6: down at once, the tombstone comes with the next start_turn; nothing
              // else goes before it.
              expect(evs.some((e) => e.name === "CharacterDied")).toBe(false);
              await expect(send(dmKey, [await endTurnIx(dmActor, acc.session, null)])).rejects.toThrow(/DeathsPending/);
              const fin = await finalize(dmKey);
              expect(fin.some((e) => e.name === "DeathFinalized") && fin.some((e) => e.name === "CharacterDied")).toBe(true);
              // The skills burn (names stay as a memorial), the soulbound items vanish.
              const burned = (await getAccount("SkillSet", await skillSetPda(mage.character)))!;
              expect(burned.skills.slice(0, burned.count).every((k) => k.powerBp === 0 && k.name[0] !== 0)).toBe(true);
              expect((await gearOf(mage.character)).itemCount).toBe(0);
              await send(dmKey, [freezeTombstoneIx(mage.character, players[MAGE]!.asset, dmKey)]);
            }
          }
        }
        party = (await readParty(acc.session, tiles))!;
        if (currentUnit(party.session) === unit && arenaMode(party.session) === "arena") await send(dmKey, [await endTurnIx(dmActor, acc.session, null)]);
      }
    }
    expect(hits).toBeGreaterThan(2);
    expect(ticks).toBeGreaterThan(hits);
    expect([checkedNotStarted, checkedActed]).toEqual([true, true]);
    expect(resonance).toBe(true);
    expect(mageDied).toBe(true);
    expect(victory).toBe(true);
    expect(arenas).toEqual([`enter ${WOLF_PACK}`, `leave ${WOLF_PACK}`, `enter ${BOSS_PACK}`]);

    // 8. the settlement (patch 11): the DM's main wallet hands out every cache and takes the
    // escrow; the DM's session key and a player cannot.
    const [owner0, owner1] = [players[0]!, players[1]!];
    party = (await readParty(acc.session))!;
    const settle = async (signer: typeof dm) => [computeLimitIx(1_000_000), await settlePartyIx(signer, acc.session, party!.session)];
    await expect(send(dmKey, await settle(dmKey))).rejects.toThrow(/DmMainWalletOnly/);
    await expect(send(owner1.wallet, await settle(owner1.wallet))).rejects.toThrow(/NotDm/);
    // Patch C: the escrow waits for the final audit.
    await expect(send(dm, await settle(dm))).rejects.toThrow(/AuditIncomplete/);
    await audit();
    party = (await readParty(acc.session))!;
    const evs = await send(dm, await settle(dm));
    expect(BigInt(evs.find((e) => e.name === "EscrowClaimed")!.data.amount)).toBe(PRICE);
    expect(evs.filter((e) => e.name === "CacheSettled").length).toBe(2);
    // "Done" with an empty bag: the cache closes (what was not taken burns).
    const empty = { kind: 0, id: 0, qty: 0 };
    const layout = { resources: Array.from({ length: 40 }, () => empty), consumables: Array.from({ length: 5 }, () => empty) };
    await send(owner0.wallet, [await closeCacheIx(owner0.wallet, owner0.character, acc.session, layout)]);
    party = (await readParty(acc.session))!;
    expect(status(party.session)).toBe("victory");
  });

  it("support skills: Quicken on self, Shelter on an ally, Sap on a wolf", { timeout: 15 * 60_000 }, async () => {
    const supportSkills = SKILL_PRESETS[Class.Support];
    const [QUICKEN, SAP, SHELTER] = ["Quicken", "Sap", "Shelter"].map((n) => supportSkills.findIndex((k) => k.name === n)) as [number, number, number];
    const opened = await openParty([
      { cls: Class.Warrior, race: Race.Human, pred: Aspect.Piercing, stats: [10, 7, 5, 8], name: "Bran" },
      { cls: Class.Support, race: Race.Human, pred: Aspect.Blood, stats: [5, 7, 13, 5], name: "Mira", skills: supportSkills },
    ]);
    const { dmKey, dmActor, players, acc, oracle, finalize, reveal } = opened;
    // Arena update: the wolves are fought in their arena.
    const first = await enterPack(opened, 1);
    const SUPPORT = 1;
    const tiles = first.tiles!;
    const wolves = first.monsters.flatMap((m, i) => (m.bestiaryId === 0 ? [i] : []));
    const mira = players[SUPPORT]!;
    const bran = first.players[WARRIOR]!;
    const effectOf = (evs: ChainEvent[]) => toEffectResolved(evs.find((e) => e.name === "EffectResolved")!.data);
    const act = async (skill: number, target: number, ally: Address | null) => send(mira.key, [await actIx(mira.actor, acc.session, mira.character, skill, target, ally, first.session.layout)]);

    let phase: "quicken" | "shelter" | "sap" | "done" = "quicken";
    let hasted = 0;
    let checkedHaste = false;
    for (let guard = 0; guard < 400 && phase !== "done"; guard++) {
      const party = (await readParty(acc.session, tiles))!;
      const s = party.session;
      expect(status(s)).toBe("active");
      if (s.core.pendingVrf) {
        await oracle(await pickSeed("miss"));
        continue;
      }
      // Patch C: the DM's reveals come first (the next turn waits for them).
      if (await reveal(party)) continue;
      if (s.pendingDeaths) {
        await finalize(dmKey);
        continue;
      }
      const unit = currentUnit(s)!;
      if (!turnStarted(s)) {
        const pl = unit < MONSTER_BASE ? party.players[unit]! : null;
        const starter = pl ? players[unit]!.key : dmKey;
        const evs = await send(starter, [await startTurnIx(acc.session, pl ? { participation: pl.participation, character: pl.character } : null, starter)]);
        // The first own turn after Quicken: ModsTicked follows TurnTicked with the hasted AP.
        if (unit === SUPPORT && hasted && !checkedHaste) {
          const names = evs.map((e) => e.name);
          expect(names.indexOf("ModsTicked")).toBeGreaterThan(names.indexOf("TurnTicked"));
          const ticked = evs.find((e) => e.name === "ModsTicked")!.data;
          expect([ticked.unit, ticked.actionPoints, ticked.expired]).toEqual([SUPPORT, hasted, 0]);
          checkedHaste = true;
        }
        continue;
      }
      if (unit === SUPPORT) {
        const me = party.players[SUPPORT]!;
        if (phase === "quicken") {
          // Ally rules: a slot that never joined, a buff on another ally without its account, an
          // account on a self-cast or on a debuff.
          await expect(act(QUICKEN, 3, null)).rejects.toThrow(/InvalidAllyTarget/);
          await expect(act(QUICKEN, WARRIOR, null)).rejects.toThrow(/AllyAccountMissing/);
          await expect(act(QUICKEN, SUPPORT, me.participation)).rejects.toThrow(/UnexpectedAllyAccount/);
          await expect(act(SAP, wolves[0]!, bran.participation)).rejects.toThrow(/UnexpectedAllyAccount/);
          await act(QUICKEN, SUPPORT, null);
          expect((await readParty(acc.session, tiles))!.session.pendingTarget).toBe(DEFAULT_KEY);
          const fx = effectOf(await oracle(await pickSeed("random")));
          expect([fx.caster, fx.target, fx.effect, fx.rider, fx.applied, fx.roll.d20]).toEqual([SUPPORT, SUPPORT, Effect.Haste, false, Applied.Added, 0]);
          // Slots hold one mod per kind in the first free slot: find it by kind.
          const haste = activeMods((await readParty(acc.session, tiles))!.players[SUPPORT]!.part.mods).find((m) => m.kind === ModKind.Haste)!;
          // Haste lasts 2 own turns, a self-cast adds Endurance 5 / self_buff_end_div 5 = 1 (patch 2),
          // stored as duration + 1 (mod_turns).
          expect([haste.turns, haste.powerBp, haste.source]).toEqual([4, fx.roll.amount, SUPPORT]);
          hasted = Math.min(255, Math.max(1, Math.floor((me.part.maxAp * (10_000 + fx.roll.amount)) / 10_000)));
          expect(hasted).toBeGreaterThan(me.part.maxAp);
          phase = "shelter";
        } else if (phase === "shelter") {
          // Both still stand on the entrance: Shelter (a strike) reaches the warrior there.
          await act(SHELTER, WARRIOR, bran.participation);
          // Patch 5: the ally comes with the party accounts, not `pending_target`.
          expect((await readParty(acc.session, tiles))!.session.pendingTarget).toBe(DEFAULT_KEY);
          const fx = effectOf(await oracle(await pickSeed("random")));
          expect([fx.caster, fx.target, fx.effect, fx.applied, fx.roll.outcome]).toEqual([SUPPORT, WARRIOR, Effect.Protection, Applied.Added, HitOutcome.Hit]);
          const after = (await readParty(acc.session, tiles))!;
          const shelter = activeMods(after.players[WARRIOR]!.part.mods).find((m) => m.kind === ModKind.Protection)!;
          expect([shelter.powerBp, shelter.source]).toEqual([fx.roll.amount, SUPPORT]);
          phase = "sap";
        } else {
          const pos = { x: me.part.core.x, y: me.part.core.y };
          const wolf = wolves.find((i) => party.monsters[i]!.alive && cheb(pos, party.monsters[i]!) >= 1 && cheb(pos, party.monsters[i]!) <= 6);
          if (wolf !== undefined) {
            await act(SAP, wolf, null);
            // A natural 20 always lands (and crits).
            const fx = effectOf(await oracle(await pickSeed("crit")));
            expect([fx.target, fx.effect, fx.roll.d20, fx.roll.crit, fx.applied]).toEqual([MONSTER_BASE + wolf, Effect.Weakening, 20, true, Applied.Added]);
            const weak = activeMods((await readParty(acc.session, tiles))!.monsterMods[wolf]).find((m) => m.kind === ModKind.Weakening)!;
            expect([weak.turns, weak.powerBp]).toEqual([3, fx.roll.amount]);
            phase = "done";
          }
        }
        await send(mira.key, [await endTurnIx(mira.actor, acc.session, mira.character)]);
      } else if (unit < MONSTER_BASE) {
        await send(players[unit]!.key, [await endTurnIx(players[unit]!.actor, acc.session, party.players[unit]!.character)]);
      } else {
        // Wolves walk up to the warrior on the entrance and bite (a target for Sap).
        const mi = unit - MONSTER_BASE;
        const m = party.monsters[mi]!;
        const target = { x: bran.part.core.x, y: bran.part.core.y };
        if (wolves.includes(mi) && m.alive && s.aliveMask & (1 << WARRIOR)) {
          let ap = m.actionPoints;
          if (cheb(m, target) > 1) {
            const steps = route({ from: m, goal: (x, y) => cheb({ x, y }, target) === 1, tiles: party.ground!, width: s.width, height: s.height, tileCost: tileCost(party), taken: taken(party, unit) });
            const take = steps ? affordable(steps, ap, 2) : [];
            if (take.length) {
              await send(dmKey, [await movePathIx(dmActor, acc.session, s.layout, { kind: "monster", index: mi }, take)]);
              ap = await afterMonsterMove(acc.session, tiles, mi, oracle);
            }
          }
          const now = (await readParty(acc.session, tiles))!;
          if (ap >= 2 && cheb(now.monsters[mi]!, target) === 1) {
            await send(dmKey, [await monsterActIx(dmActor, acc.session, mi, 0, WARRIOR, bran.character, s.layout)]);
            await oracle(await pickSeed("random"));
          }
        }
        if (currentUnit((await readParty(acc.session, tiles))!.session) === unit) await send(dmKey, [await endTurnIx(dmActor, acc.session, null)]);
      }
    }
    expect(phase).toBe("done");
    expect(checkedHaste).toBe(true);
  });

  it("Expose on a wolf: the warrior's Fire packet takes the Vulnerability, HP drops by the breakdown", { timeout: 15 * 60_000 }, async () => {
    const supportSkills = SKILL_PRESETS[Class.Support];
    const EXPOSE = supportSkills.findIndex((k) => k.name === "Expose");
    expect(EXPOSE).toBeGreaterThanOrEqual(0);
    // A Fire predisposition (forest elf) gives the warrior's basic hit a Fire packet next to the Piercing sword.
    const opened = await openParty([
      { cls: Class.Warrior, race: Race.ForestElf, pred: Aspect.Fire, stats: [10, 7, 5, 8], name: "Bran" },
      { cls: Class.Support, race: Race.Human, pred: Aspect.Blood, stats: [5, 7, 13, 5], name: "Mira", skills: supportSkills },
    ]);
    const { dmKey, dmActor, players, acc, oracle, finalize, reveal } = opened;
    // Arena update: the wolves are fought in their arena.
    const first = await enterPack(opened, 1);
    const SUPPORT = 1;
    const tiles = first.tiles!;
    const wolves = first.monsters.flatMap((m, i) => (m.bestiaryId === 0 ? [i] : []));
    const bran = players[WARRIOR]!;
    const mira = players[SUPPORT]!;
    const branAt = (p: Party) => ({ x: p.players[WARRIOR]!.part.core.x, y: p.players[WARRIOR]!.part.core.y });
    const vulnOf = (p: Party, wolf: number) => activeMods(p.monsterMods[wolf]).find((m) => m.kind === ModKind.Vulnerability);

    let phase: "expose" | "strike" | "done" = "expose";
    let exposed = -1;
    for (let guard = 0; guard < 400 && phase !== "done"; guard++) {
      const party = (await readParty(acc.session, tiles))!;
      const s = party.session;
      expect(status(s)).toBe("active");
      if (s.core.pendingVrf) {
        await oracle(await pickSeed("miss"));
        continue;
      }
      // Patch C: the DM's reveals come first (the next turn waits for them).
      if (await reveal(party)) continue;
      if (s.pendingDeaths) {
        await finalize(dmKey);
        continue;
      }
      const unit = currentUnit(s)!;
      if (!turnStarted(s)) {
        const pl = unit < MONSTER_BASE ? party.players[unit]! : null;
        const starter = pl ? players[unit]!.key : dmKey;
        await send(starter, [await startTurnIx(acc.session, pl ? { participation: pl.participation, character: pl.character } : null, starter)]);
        continue;
      }
      const target = branAt(party);
      if (unit === SUPPORT) {
        const me = party.players[SUPPORT]!.part.core;
        // A wolf already next to the warrior, in Mira's reach.
        const wolf = wolves.find((i) => party.monsters[i]!.alive && cheb(party.monsters[i]!, target) === 1 && cheb(me, party.monsters[i]!) >= 1 && cheb(me, party.monsters[i]!) <= 6);
        if (phase === "expose" && wolf !== undefined) {
          await send(mira.key, [await actIx(mira.actor, acc.session, mira.character, EXPOSE, wolf, null, s.layout)]);
          const fx = toEffectResolved((await oracle(await pickSeed("crit"))).find((e) => e.name === "EffectResolved")!.data);
          expect([fx.target, fx.effect, fx.roll.d20, fx.applied]).toEqual([MONSTER_BASE + wolf, Effect.Vulnerability, 20, Applied.Added]);
          const v = vulnOf((await readParty(acc.session, tiles))!, wolf)!;
          expect([v.aspect, v.powerBp]).toEqual([Aspect.Fire, fx.roll.amount]);
          exposed = wolf;
          phase = "strike";
        }
        await send(mira.key, [await endTurnIx(mira.actor, acc.session, mira.character)]);
      } else if (unit === WARRIOR) {
        const v = exposed >= 0 ? vulnOf(party, exposed) : undefined;
        const wolf = exposed >= 0 ? party.monsters[exposed]! : null;
        if (phase === "strike" && !v) phase = "expose"; // wore off before the warrior's turn
        if (phase === "strike" && v && wolf?.alive && cheb(wolf, target) === 1) {
          const hpBefore = wolf.hp;
          await send(bran.key, [await actIx(bran.actor, acc.session, bran.character, BASE_ATTACK, exposed, null, s.layout)]);
          // A plain roll (no crit), so the hit rarely kills and the HP drop is the whole damage.
          const hit = toHitResolved((await oracle(await pickSeed("random"))).find((e) => e.name === "HitResolved")!.data).result;
          const hpAfter = (await readParty(acc.session, tiles))!.monsters[exposed]!.hp;
          if (hit.outcome !== HitOutcome.Hit) {
            // A miss: nothing changes, try again on the next warrior turn.
            expect(hpAfter).toBe(hpBefore);
            await send(bran.key, [await endTurnIx(bran.actor, acc.session, party.players[WARRIOR]!.character)]);
            continue;
          }
          const fire = hit.packets.find((p) => p.aspect === Aspect.Fire)!;
          const piercing = hit.packets.find((p) => p.aspect === Aspect.Piercing)!;
          // Vulnerability is to its aspect only: the Fire packet carries it, the Piercing one does not.
          expect(fire.vulnBp).toBe(v.powerBp);
          expect(piercing.vulnBp).toBe(0);
          // Fire is not physical and the wolf resists only Cold: after defense = rolled x (1 + vuln).
          expect([fire.armor, fire.resistBp]).toEqual([0, 0]);
          const fireAfter = (BigInt(fire.rolled) * BigInt(10_000 + v.powerBp)) / 10_000n;
          expect(BigInt(fire.afterDefense)).toBe(fireAfter);
          const total = hit.packets.reduce((n, p) => n + BigInt(p.afterDefense), 0n);
          expect(hit.damage).toBe(Math.max(1, Number(total / 10_000n)));
          // The real HP drop is the breakdown's damage.
          expect(hpBefore - hpAfter).toBe(Math.min(hit.damage, hpBefore));
          const withoutVuln = Number((total - BigInt(fire.afterDefense) + BigInt(fire.rolled)) / 10_000n);
          console.log(
            `[vuln check] Vulnerability Fire +${v.powerBp / 100}%: Fire rolled ${Number(fire.rolled) / 10_000} -> ${Number(fire.afterDefense) / 10_000}, ` +
              `Piercing ${Number(piercing.afterDefense) / 10_000} (vuln ${piercing.vulnBp}), damage ${hit.damage} (without vuln ${Math.max(1, withoutVuln)}), wolf HP ${hpBefore} -> ${hpAfter}`,
          );
          phase = "done";
        }
        if (phase !== "done") await send(bran.key, [await endTurnIx(bran.actor, acc.session, party.players[WARRIOR]!.character)]);
      } else {
        // Wolves walk up to the warrior and bite; the others wait.
        const mi = unit - MONSTER_BASE;
        const m = party.monsters[mi]!;
        if (wolves.includes(mi) && m.alive && s.aliveMask & (1 << WARRIOR)) {
          let ap = m.actionPoints;
          if (cheb(m, target) > 1) {
            const steps = route({ from: m, goal: (x, y) => cheb({ x, y }, target) === 1, tiles: party.ground!, width: s.width, height: s.height, tileCost: tileCost(party), taken: taken(party, unit) });
            const take = steps ? affordable(steps, ap, 2) : [];
            if (take.length) {
              await send(dmKey, [await movePathIx(dmActor, acc.session, s.layout, { kind: "monster", index: mi }, take)]);
              ap = await afterMonsterMove(acc.session, tiles, mi, oracle);
            }
          }
          const now = (await readParty(acc.session, tiles))!;
          if (ap >= 2 && cheb(now.monsters[mi]!, target) === 1) {
            await send(dmKey, [await monsterActIx(dmActor, acc.session, mi, 0, WARRIOR, first.players[WARRIOR]!.character, s.layout)]);
            await oracle(await pickSeed("random"));
          }
        }
        if (currentUnit((await readParty(acc.session, tiles))!.session) === unit) await send(dmKey, [await endTurnIx(dmActor, acc.session, null)]);
      }
    }
    expect(phase).toBe("done");
  });
});

// Patch 13: the builder checks a draft with crates/mapcheck through mapcheck.wasm, on the registry
// tables read from the chain; finalize_map must give the same verdict (docs/MAP_DESCRIPTION.md 1, 10).
describe.skipIf(!ON)("localnet: the builder's verdict is finalize_map's", () => {
  it("valid and broken drafts", { timeout: 5 * 60_000 }, async () => {
    const wasm = await MapcheckWasm.instantiate(readFileSync(resolve(import.meta.dirname, "wasm/mapcheck.wasm")));
    const [bestiary, monsterSkills, rewards, tileset] = await Promise.all([registryBestiary(), registryMonsterSkills(), registryRewards(), registryTileset()]);
    wasm.setTables({ bestiary: bestiary!, monsterSkills: monsterSkills!, rewards: rewards!, tileset: tileset! });
    const preset = demoMap as MapPresetData & { tier: number };
    const draft = () => ({ width: preset.width, height: preset.height, tier: preset.tier, ...presetToLayout(preset) });
    const at = (d: ReturnType<typeof draft>, x: number, y: number, kind: number, variant = 0) => (d.tiles[y * MAP_MAX + x] = cellByte(kind, variant));
    const cases: [string, (d: ReturnType<typeof draft>) => void][] = [
      ["demo", () => {}],
      ["exit on a chasm", (d) => at(d, d.exit!.x, d.exit!.y, Tile.Chasm)],
      ["corridor in deep water", (d) => at(d, 11, 7, Tile.DeepWater)],
      ["corridor in rubble", (d) => at(d, 11, 7, Tile.Rubble, 3)],
      // Patch 13: a diagonal never cuts a corner.
      ["a diagonal past wall corners", (d) => (at(d, 11, 7, Tile.Wall), at(d, 12, 7, Tile.StoneFloor))],
      ["reserved tile type", (d) => at(d, 2, 2, 11)],
      ["spawn on a wall", (d) => at(d, d.spawns[0]!.x, d.spawns[0]!.y, Tile.Wall)],
      ["entrance on the exit", (d) => (d.entrance = d.exit)],
      ["two chests", (d) => d.chests.push({ x: 2, y: 2, table: d.chests[0]!.table })],
      ["minimum players 0", (d) => (d.minPlayers = 0)],
    ];
    const dm = await funded(20);
    for (const [name, edit] of cases) {
      const d = draft();
      edit(d);
      const verdict = wasm.check(d);
      const built = await buildMapIxs(dm, d, keysOf(dm), isBoss);
      const fin = built.ixs.pop()!;
      // create_map refuses minimum players 0 itself (the same rule, its own error).
      if (name === "minimum players 0") {
        expect(verdict, name).toBe("MinPlayersOutOfRange");
        await expect(send(dm, [built.ixs[0]!])).rejects.toThrow(/MinPlayersOutOfRange/);
        continue;
      }
      for (const ix of built.ixs) await send(dm, [ix]);
      const chain = await send(dm, [fin]).then(
        () => null,
        (e: unknown) => errorName(e instanceof Error ? e.message : String(e)),
      );
      expect(chain, name).toBe(verdict);
    }
  });
});

describe.skipIf(!ON)("localnet: the builder templates", () => {
  it("each biome's template is published as the DM's map, with its biome and variants", { timeout: 5 * 60_000 }, async () => {
    const dm = await funded(20);
    for (const p of MAP_PRESETS.filter((m) => m.biome !== undefined)) {
      const layout = presetToLayout(p);
      const built = await buildMapIxs(dm, { width: p.width, height: p.height, tier: p.tier ?? 1, ...layout }, keysOf(dm), isBoss);
      for (const ix of built.ixs) await send(dm, [ix]);
      const map = (await getAccount("Map", built.map))!;
      expect([map.biome, map.core.version, mapStatus(map)], p.name).toEqual([p.biome, 2, "finalized"]);
      // The cells keep the variants the template picked.
      expect(Array.from((await mapTiles(built.layout))!), p.name).toEqual(Array.from(chainTiles({ ...layout, width: p.width, height: p.height, tier: 1 })));
    }
  });
});
