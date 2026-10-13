// @vitest-environment node
// The demo script through the app's own builders (game.ts) against the shared local validator,
// the same flow as tests/demo.test.ts, with the turn rules (start_turn, one attack per turn), and
// the support skills of PR #25 (heals and buffs on allies, debuffs on monsters).
// Skipped unless DEADROLL_LOCALNET=1 (needs deploy-local).
//   DEADROLL_LOCALNET=1 pnpm --filter @deadroll/app exec vitest run src/chain/localnet.e2e.test.ts
import { describe, expect, it } from "vitest";
import { generateKeyPairSigner, lamports, type Address, type KeyPairSigner } from "@solana/kit";
import demoMap from "../../../assets/registry/maps/demo_16x16.json";
import { PROGRAMS } from "../config";
import { client } from "../wallet/client";
import { createSessionV2Instruction } from "../session-key/sessionKey";
import { decodeEvents, type ChainEvent } from "./events";
import {
  BASE_ATTACK,
  DEFAULT_KEY,
  MONSTER_BASE,
  SKILL_PRESETS,
  actIx,
  buildMapIxs,
  claimEscrowIx,
  claimRewardsIx,
  consumeIx,
  createCharacterIxs,
  currentUnit,
  endTurnIx,
  freezeTombstoneIx,
  joinSessionIx,
  monsterActIx,
  monsterMoveIx,
  moveIx,
  openSessionIx,
  pendingTargetOf,
  pickSeed,
  playerAp,
  readParty,
  sessionAccounts,
  setReadyIx,
  startSessionIx,
  startTurnIx,
  status,
  type Actor,
  type Party,
} from "./game";
import { presetToLayout, type MapPresetData } from "./mock";
import { coders } from "./programs";
import { affordable, cheb, route } from "./route";
import { sendTx } from "./send";
import { toHitResolved } from "./hit";
import { Category, Effect, Form, Purpose, Source, makeBlocks, type NamedSkill } from "./skillcheck";
import { toEffectResolved } from "./support";
import { turnStarted } from "./turns";
import { Applied, Aspect, Class, HitOutcome, MAP_MAX, ModKind, Race } from "./types";

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

const IDS = { session: PROGRAMS.session, character: PROGRAMS.character, map: PROGRAMS.map };
async function send(payer: KeyPairSigner, ixs: Parameters<typeof sendTx>[1]): Promise<ChainEvent[]> {
  const { logs } = await sendTx(payer, ixs);
  return decodeEvents(logs, coders, IDS);
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
      const c = await createCharacterIxs(wallet, { name, race, class: cls, predisposition: pred, stats: [...stats], skills });
      if (c.profileIx) await send(wallet, [c.profileIx]);
      const evs = await send(wallet, [c.ix]);
      const created = evs.find((e) => e.name === "CharacterCreated")!.data;
      expect(created.nft).toBe(c.asset.address);
      players.push({ wallet, key, character: c.character, asset: c.asset.address, actor: { signer: key, token: null } });
    }

    // 2. map from the preset
    const layout = presetToLayout(preset);
    const built = await buildMapIxs(dm, { width: preset.width, height: preset.height, tier: preset.tier, ...layout });
    for (const ix of built.ixs) await send(dm, [ix]);
    const acc = await sessionAccounts(built.map);

    // 3. party: open with price, pay half each, session keys, ready, start
    await send(dm, [await openSessionIx(dm, built.map, PRICE, dmKey.address)]);
    for (const pl of players) {
      await send(pl.wallet, [await joinSessionIx(pl.wallet, acc.session, pl.character, PRICE / 2n, pl.key.address)]);
      const { instruction, sessionToken } = await createSessionV2Instruction({
        authority: pl.wallet,
        sessionSigner: pl.key,
        targetProgram: PROGRAMS.session,
        validUntil: BigInt(Math.floor(Date.now() / 1000) + 3600),
      });
      await send(pl.wallet, [instruction]);
      pl.actor.token = sessionToken;
    }
    const dmTok = await createSessionV2Instruction({ authority: dm, sessionSigner: dmKey, targetProgram: PROGRAMS.session, validUntil: BigInt(Math.floor(Date.now() / 1000) + 3600) });
    await send(dm, [dmTok.instruction]);
    const dmActor: Actor = { signer: dmKey, token: dmTok.sessionToken };
    for (const pl of players) await send(pl.wallet, [await setReadyIx(pl.wallet, acc.session, pl.character, true)]);
    await send(dm, [await startSessionIx(dm, acc.session)]);

    const party = (await readParty(acc.session))!;
    expect(status(party.session)).toBe("active");
    expect(party.session.core.pendingVrf).toBe(true);
    // The localnet oracle: the ally of a pending heal or buff rides along (GameSession.pendingTarget).
    const oracle = async (seed: Uint8Array): Promise<ChainEvent[]> => {
      const p = (await readParty(acc.session, party.tiles))!;
      const pendingPart = p.session.pending.participation;
      const pl = p.players.find((x) => x.participation === pendingPart)!;
      return send(dmKey, [await consumeIx(acc.session, pendingPart, pl.character, seed, pendingTargetOf(p.session))]);
    };
    const init = await oracle(await pickSeed("random"));
    expect(init.some((e) => e.name === "InitiativeRolled")).toBe(true);
    return { preset, dm, dmKey, dmActor, players, built, acc, oracle, party };
  }
}

const taken = (p: Party, moverUnit: number) => {
  const t = new Set<number>();
  p.monsters.forEach((m, i) => m.alive && MONSTER_BASE + i !== moverUnit && t.add(m.y * MAP_MAX + m.x));
  p.players.forEach((pl) => p.session.aliveMask & (1 << pl.slot) && pl.slot !== moverUnit && t.add(pl.part.core.y * MAP_MAX + pl.part.core.x));
  return t;
};
const tileCost = (p: Party) => Array.from(p.session.tileCost);

describe.skipIf(!ON)("localnet: the demo through the app builders", () => {
  it("plays create -> map -> party -> fight -> death -> victory -> claims", { timeout: 15 * 60_000 }, async () => {
    const { preset, dm, dmKey, dmActor, players, built, acc, oracle, party: first } = await openParty([
      { cls: Class.Warrior, race: Race.Human, pred: Aspect.Piercing, stats: [10, 7, 5, 8], name: "Bran" },
      { cls: Class.Mage, race: Race.ForestElf, pred: Aspect.Cold, stats: [5, 7, 13, 5], name: "Ilyra" },
    ]);
    let party = first;

    // 4-7. combat: wolves go for the mage, the mage casts Frost Burn (resonant seed), the warrior hunts.
    const tiles = party.tiles!;
    const wolves = preset.spawns.flatMap((s, i) => (s.bestiary_id === 0 ? [i] : []));
    const boss = preset.spawns.findIndex((s) => s.bestiary_id === 2);
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
      const unit = currentUnit(s)!;
      // Every turn begins with start_turn, sent alone by whoever plays the unit (the app's rule).
      if (!turnStarted(s)) {
        const pl = unit < MONSTER_BASE ? party.players[unit]! : null;
        const payer = pl ? players[unit]!.key : dmKey;
        if (!checkedNotStarted && pl) {
          await expect(send(payer, [await endTurnIx(players[unit]!.actor, acc.session, pl.character)])).rejects.toThrow(/TurnNotStarted/);
          checkedNotStarted = true;
        }
        const evs = await send(payer, [await startTurnIx(acc.session, pl ? { participation: pl.participation, character: pl.character } : null)]);
        const tick = evs.find((e) => e.name === "TurnTicked")!.data;
        expect([tick.unit, tick.turnSeq]).toEqual([unit, s.turnSeq]);
        ticks++;
        // A skip or a DoT death hands the turn on: the next unit needs its own start.
        continue;
      }
      const mageAlive = (s.aliveMask & (1 << MAGE)) !== 0;
      const livingWolves = wolves.filter((i) => party.monsters[i]!.alive);
      if (unit < MONSTER_BASE) {
        const pl = players[unit]!;
        const me = party.players[unit]!;
        let ap = playerAp(me.part, s);
        const pos = { x: me.part.core.x, y: me.part.core.y };
        if (unit === MAGE && !resonance && livingWolves.length && cheb(pos, party.monsters[livingWolves[0]!]!) <= 6) {
          const used = (await send(pl.key, [await actIx(pl.actor, acc.session, pl.character, 1, livingWolves[0]!, null)])).find((e) => e.name === "SkillUsed")!.data;
          expect(used.skillSlot).toBe(1);
          const after = (await readParty(acc.session, tiles))!;
          expect(Array.from(used.cooldowns as Uint8Array)).toEqual(Array.from(after.players[MAGE]!.part.cooldowns));
          expect(after.session.actedSeq).toBe(after.session.turnSeq);
          const evs = await oracle(await pickSeed("resonant"));
          const hit = toHitResolved(evs.find((e) => e.name === "HitResolved")!.data);
          hits++;
          resonance = hit.result.resonances.some((r) => r?.active);
          expect(resonance).toBe(true);
        } else if (unit === WARRIOR && (!mageAlive || livingWolves.length === 0)) {
          const targets = livingWolves.length ? livingWolves : [boss];
          const target = targets[0]!;
          const m = party.monsters[target]!;
          if (cheb(pos, m) > 1) {
            const steps = route({ from: pos, goal: (x, y) => cheb({ x, y }, m) === 1, tiles, width: s.width, height: s.height, tileCost: tileCost(party), taken: taken(party, unit) });
            const take = steps ? affordable(steps, ap, 2) : [];
            if (take.length) {
              await send(pl.key, [await moveIx(pl.actor, acc.session, pl.character, s.layout, take)]);
              ap -= take.reduce((n, st) => n + st.cost, 0);
            }
          }
          party = (await readParty(acc.session, tiles))!;
          const now = party.players[unit]!.part.core;
          // One attack per turn: Piercing Thrust off cooldown, else the weapon (no cooldown).
          if (ap >= 2 && cheb(now, party.monsters[target]!) === 1 && party.monsters[target]!.alive) {
            const skill = party.players[unit]!.part.cooldowns[0] === 0 ? 0 : BASE_ATTACK;
            await send(pl.key, [await actIx(pl.actor, acc.session, pl.character, skill, target, null)]);
            const evs = await oracle(await pickSeed("crit"));
            hits++;
            if (evs.some((e) => e.name === "SessionEnded")) victory = true;
            if (!victory && !checkedActed) {
              await expect(send(pl.key, [await actIx(pl.actor, acc.session, pl.character, BASE_ATTACK, target, null)])).rejects.toThrow(/AlreadyActed/);
              checkedActed = true;
            }
          }
        }
        if (!victory) await send(pl.key, [await endTurnIx(pl.actor, acc.session, pl.character)]);
      } else {
        const mi = unit - MONSTER_BASE;
        const m = party.monsters[mi]!;
        const mage = party.players[MAGE]!;
        if (wolves.includes(mi) && m.alive && mageAlive) {
          const target = { x: mage.part.core.x, y: mage.part.core.y };
          let ap = m.actionPoints;
          if (cheb(m, target) > 1) {
            const steps = route({ from: m, goal: (x, y) => cheb({ x, y }, target) === 1, tiles, width: s.width, height: s.height, tileCost: tileCost(party), taken: taken(party, unit) });
            const take = steps ? affordable(steps, ap, 2) : [];
            if (take.length) {
              await send(dmKey, [await monsterMoveIx(dmActor, acc.session, s.layout, mi, take)]);
              ap -= take.reduce((n, st) => n + st.cost, 0);
            }
          }
          party = (await readParty(acc.session, tiles))!;
          if (ap >= 2 && cheb(party.monsters[mi]!, target) === 1) {
            await send(dmKey, [await monsterActIx(dmActor, acc.session, mi, MAGE, mage.character)]);
            const evs = await oracle(await pickSeed("crit"));
            hits++;
            if (evs.some((e) => e.name === "PlayerDied")) {
              mageDied = true;
              expect(evs.some((e) => e.name === "CharacterDied")).toBe(true);
              await send(dmKey, [freezeTombstoneIx(mage.character, players[MAGE]!.asset, dmKey)]);
            }
          }
        }
        party = (await readParty(acc.session, tiles))!;
        if (currentUnit(party.session) === unit) await send(dmKey, [await endTurnIx(dmActor, acc.session, null)]);
      }
    }
    expect(hits).toBeGreaterThan(2);
    expect(ticks).toBeGreaterThan(hits);
    expect([checkedNotStarted, checkedActed]).toEqual([true, true]);
    expect(resonance).toBe(true);
    expect(mageDied).toBe(true);
    expect(victory).toBe(true);

    // 8. claims
    for (const pl of players) await send(dmKey, [await claimRewardsIx(acc.session, pl.character)]);
    const evs = await send(dm, [await claimEscrowIx(dm, acc.session, built.map)]);
    expect(BigInt(evs.find((e) => e.name === "EscrowClaimed")!.data.amount)).toBe(PRICE);
    party = (await readParty(acc.session))!;
    expect(status(party.session)).toBe("victory");
  });

  it("support skills: Quicken on self, Mend on an ally, Sap on a wolf", { timeout: 15 * 60_000 }, async () => {
    // The Support starting set with Mend (a heal, as in tests/session.test.ts) in place of Expose.
    const MEND: NamedSkill = { name: "Mend", blocks: makeBlocks(Category.Active, Purpose.Buff, Source.Magical, Form.Projectile, [Aspect.Blood], { effect: Effect.Heal }) };
    const supportSkills = SKILL_PRESETS[Class.Support].map((k) => (k.name === "Expose" ? MEND : k));
    const [QUICKEN, SAP, MEND_SLOT] = ["Quicken", "Sap", "Mend"].map((n) => supportSkills.findIndex((k) => k.name === n)) as [number, number, number];
    const { dmKey, dmActor, players, acc, oracle, party: first } = await openParty([
      { cls: Class.Warrior, race: Race.Human, pred: Aspect.Piercing, stats: [10, 7, 5, 8], name: "Bran" },
      { cls: Class.Support, race: Race.Human, pred: Aspect.Blood, stats: [5, 7, 13, 5], name: "Mira", skills: supportSkills },
    ]);
    const SUPPORT = 1;
    const tiles = first.tiles!;
    const wolves = first.monsters.flatMap((m, i) => (m.bestiaryId === 0 ? [i] : []));
    const mira = players[SUPPORT]!;
    const bran = first.players[WARRIOR]!;
    const effectOf = (evs: ChainEvent[]) => toEffectResolved(evs.find((e) => e.name === "EffectResolved")!.data);
    const act = async (skill: number, target: number, ally: Address | null) => send(mira.key, [await actIx(mira.actor, acc.session, mira.character, skill, target, ally)]);

    let phase: "quicken" | "mend" | "sap" | "done" = "quicken";
    let hasted = 0;
    let checkedHaste = false;
    for (let guard = 0; guard < 400 && phase !== "done"; guard++) {
      const party = (await readParty(acc.session, tiles))!;
      const s = party.session;
      expect(status(s)).toBe("active");
      const unit = currentUnit(s)!;
      if (!turnStarted(s)) {
        const pl = unit < MONSTER_BASE ? party.players[unit]! : null;
        const evs = await send(pl ? players[unit]!.key : dmKey, [await startTurnIx(acc.session, pl ? { participation: pl.participation, character: pl.character } : null)]);
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
          const haste = (await readParty(acc.session, tiles))!.players[SUPPORT]!.part.mods[ModKind.Haste]!;
          expect([haste.turns, haste.powerBp, haste.source]).toEqual([3, fx.roll.amount, SUPPORT]);
          hasted = Math.min(255, Math.max(1, Math.floor((me.part.maxAp * (10_000 + fx.roll.amount)) / 10_000)));
          expect(hasted).toBeGreaterThan(me.part.maxAp);
          phase = "mend";
        } else if (phase === "mend") {
          await act(MEND_SLOT, WARRIOR, bran.participation);
          expect((await readParty(acc.session, tiles))!.session.pendingTarget).toBe(bran.participation);
          const fx = effectOf(await oracle(await pickSeed("random")));
          expect([fx.caster, fx.target, fx.effect, fx.applied, fx.roll.outcome]).toEqual([SUPPORT, WARRIOR, Effect.Heal, Applied.None, HitOutcome.Hit]);
          const after = (await readParty(acc.session, tiles))!;
          const warrior = after.players[WARRIOR]!.part;
          expect(warrior.hpMax).toBeGreaterThan(0);
          expect(fx.hp).toBe(Math.min(warrior.hpMax, party.players[WARRIOR]!.part.core.hp + fx.roll.amount));
          expect(warrior.core.hp).toBe(fx.hp);
          expect(after.session.pendingTarget).toBe(DEFAULT_KEY);
          phase = "sap";
        } else {
          const pos = { x: me.part.core.x, y: me.part.core.y };
          const wolf = wolves.find((i) => party.monsters[i]!.alive && cheb(pos, party.monsters[i]!) >= 1 && cheb(pos, party.monsters[i]!) <= 6);
          if (wolf !== undefined) {
            await act(SAP, wolf, null);
            // A natural 20 always lands (and crits).
            const fx = effectOf(await oracle(await pickSeed("crit")));
            expect([fx.target, fx.effect, fx.roll.d20, fx.roll.crit, fx.applied]).toEqual([MONSTER_BASE + wolf, Effect.Weakening, 20, true, Applied.Added]);
            const weak = (await readParty(acc.session, tiles))!.monsterMods[wolf]![ModKind.Weakening]!;
            expect([weak.turns, weak.powerBp]).toEqual([3, fx.roll.amount]);
            phase = "done";
          }
        }
        await send(mira.key, [await endTurnIx(mira.actor, acc.session, mira.character)]);
      } else if (unit < MONSTER_BASE) {
        await send(players[unit]!.key, [await endTurnIx(players[unit]!.actor, acc.session, party.players[unit]!.character)]);
      } else {
        // Wolves walk up to the warrior on the entrance and bite (wounds for Mend, a target for Sap).
        const mi = unit - MONSTER_BASE;
        const m = party.monsters[mi]!;
        const target = { x: bran.part.core.x, y: bran.part.core.y };
        if (wolves.includes(mi) && m.alive && s.aliveMask & (1 << WARRIOR)) {
          let ap = m.actionPoints;
          if (cheb(m, target) > 1) {
            const steps = route({ from: m, goal: (x, y) => cheb({ x, y }, target) === 1, tiles, width: s.width, height: s.height, tileCost: tileCost(party), taken: taken(party, unit) });
            const take = steps ? affordable(steps, ap, 2) : [];
            if (take.length) {
              await send(dmKey, [await monsterMoveIx(dmActor, acc.session, s.layout, mi, take)]);
              ap -= take.reduce((n, st) => n + st.cost, 0);
            }
          }
          const now = (await readParty(acc.session, tiles))!;
          if (ap >= 2 && cheb(now.monsters[mi]!, target) === 1) {
            await send(dmKey, [await monsterActIx(dmActor, acc.session, mi, WARRIOR, bran.character)]);
            await oracle(await pickSeed("random"));
          }
        }
        if (currentUnit((await readParty(acc.session, tiles))!.session) === unit) await send(dmKey, [await endTurnIx(dmActor, acc.session, null)]);
      }
    }
    expect(phase).toBe("done");
    expect(checkedHaste).toBe(true);
  });
});
