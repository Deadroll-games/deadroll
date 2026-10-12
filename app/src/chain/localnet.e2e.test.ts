// @vitest-environment node
// The demo script through the app's own builders (game.ts) against the shared local validator,
// the same flow as tests/demo.test.ts. Skipped unless DEADROLL_LOCALNET=1 (needs deploy-local).
//   DEADROLL_LOCALNET=1 pnpm --filter @deadroll/app exec vitest run src/chain/localnet.e2e.test.ts
import { describe, expect, it } from "vitest";
import { generateKeyPairSigner, lamports, type Address, type KeyPairSigner } from "@solana/kit";
import demoMap from "../../../assets/registry/maps/demo_16x16.json";
import { PROGRAMS } from "../config";
import { client } from "../wallet/client";
import { createSessionV2Instruction } from "../session-key/sessionKey";
import { decodeEvents, type ChainEvent } from "./events";
import {
  MONSTER_BASE,
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
  pickSeed,
  playerAp,
  readParty,
  sessionAccounts,
  setReadyIx,
  startSessionIx,
  status,
  type Actor,
  type Party,
} from "./game";
import { presetToLayout, type MapPresetData } from "./mock";
import { coders } from "./programs";
import { affordable, cheb, route } from "./route";
import { sendTx } from "./send";
import { toHitResolved } from "./hit";
import { Aspect, Class, MAP_MAX, Race } from "./types";

const ON = process.env.DEADROLL_LOCALNET === "1";
const WARRIOR = 0;
const MAGE = 1;

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

describe.skipIf(!ON)("localnet: the demo through the app builders", () => {
  it("plays create -> map -> party -> fight -> death -> victory -> claims", { timeout: 15 * 60_000 }, async () => {
    const preset = demoMap as MapPresetData & { tier: number };
    const [dm, dmKey] = await Promise.all([funded(), funded(1)]);
    const players: { wallet: KeyPairSigner; key: KeyPairSigner; character: Address; asset: Address; actor: Actor }[] = [];

    // 1. characters (Core NFT)
    for (const [cls, race, pred, stats, name] of [
      [Class.Warrior, Race.Human, Aspect.Piercing, [10, 7, 5, 8], "Bran"],
      [Class.Mage, Race.ForestElf, Aspect.Cold, [5, 7, 13, 5], "Ilyra"],
    ] as const) {
      const [wallet, key] = await Promise.all([funded(), funded(1)]);
      const c = await createCharacterIxs(wallet, { name, race, class: cls, predisposition: pred, stats: [...stats] });
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
    const PRICE = 100_000_000n;
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

    let party = (await readParty(acc.session))!;
    expect(status(party.session)).toBe("active");
    expect(party.session.core.pendingVrf).toBe(true);
    const oracle = async (seed: Uint8Array): Promise<ChainEvent[]> => {
      const p = (await readParty(acc.session, party.tiles))!;
      const pendingPart = p.session.pending.participation;
      const pl = p.players.find((x) => x.participation === pendingPart)!;
      return send(dmKey, [await consumeIx(acc.session, pendingPart, pl.character, seed)]);
    };
    const init = await oracle(await pickSeed("random"));
    expect(init.some((e) => e.name === "InitiativeRolled")).toBe(true);

    // 4-7. combat: wolves go for the mage, the mage casts Frost Burn (resonant seed), the warrior hunts.
    const tiles = party.tiles!;
    const taken = (p: Party, moverUnit: number) => {
      const t = new Set<number>();
      p.monsters.forEach((m, i) => m.alive && MONSTER_BASE + i !== moverUnit && t.add(m.y * MAP_MAX + m.x));
      p.players.forEach((pl) => p.session.aliveMask & (1 << pl.slot) && pl.slot !== moverUnit && t.add(pl.part.core.y * MAP_MAX + pl.part.core.x));
      return t;
    };
    const tileCost = (p: Party) => Array.from(p.session.tileCost);
    const wolves = preset.spawns.flatMap((s, i) => (s.bestiary_id === 0 ? [i] : []));
    const boss = preset.spawns.findIndex((s) => s.bestiary_id === 2);
    let resonance = false;
    let mageDied = false;
    let victory = false;
    let hits = 0;
    for (let guard = 0; guard < 400 && !victory; guard++) {
      party = (await readParty(acc.session, tiles))!;
      const s = party.session;
      if (status(s) !== "active") break;
      const unit = currentUnit(s)!;
      const mageAlive = (s.aliveMask & (1 << MAGE)) !== 0;
      const livingWolves = wolves.filter((i) => party.monsters[i]!.alive);
      if (unit < MONSTER_BASE) {
        const pl = players[unit]!;
        const me = party.players[unit]!;
        let ap = playerAp(me.part, s);
        const pos = { x: me.part.core.x, y: me.part.core.y };
        if (unit === MAGE && !resonance && livingWolves.length && cheb(pos, party.monsters[livingWolves[0]!]!) <= 6) {
          const evs = await (async () => {
            await send(pl.key, [await actIx(pl.actor, acc.session, pl.character, 1, livingWolves[0]!)]);
            return oracle(await pickSeed("resonant"));
          })();
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
          while (ap >= 2 && cheb(now, party.monsters[target]!) === 1 && party.monsters[target]!.alive) {
            await send(pl.key, [await actIx(pl.actor, acc.session, pl.character, 0, target)]);
            const evs = await oracle(await pickSeed("crit"));
            hits++;
            if (evs.some((e) => e.name === "SessionEnded")) victory = true;
            ap -= 2;
            party = (await readParty(acc.session, tiles))!;
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
});
