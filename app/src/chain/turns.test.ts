import { describe, expect, it } from "vitest";
import { getAddressDecoder, type Address } from "@solana/kit";
import type { GameSessionAcc, Party, PartyPlayer } from "./game";
import { errorName } from "./send";
import { cooldownLabel, skillCooldowns, startTurnFor, turnActed, turnStarted } from "./turns";

const A = (n: number) => getAddressDecoder().decode(new Uint8Array(32).fill(n)) as Address;
const DM = A(1);
const [ALICE, BOB] = [A(2), A(3)];

/** Two players (slots 0, 1) and a monster (unit 4); turn order 0, 4, 1; `turn` is the index in it. */
function party(turn: number, over: Partial<GameSessionAcc> = {}, core: Partial<GameSessionAcc["core"]> = {}): Party {
  const session = {
    core: { status: { active: {} }, dm: DM, turnOrder: Uint8Array.of(0, 4, 1), turnIndex: turn, pendingVrf: false, ...core },
    turnOrderLen: 3,
    turnSeq: 7,
    startedSeq: 6,
    actedSeq: 6,
    dmActor: A(9),
    ...over,
  } as unknown as GameSessionAcc;
  const player = (slot: number, owner: Address) =>
    ({ slot, character: A(20 + slot), participation: A(30 + slot), part: { player: owner, core: { actor: A(40 + slot) } } }) as unknown as PartyPlayer;
  return { address: A(5), session, monsters: [], players: [player(0, ALICE), player(1, BOB)], monsterCooldowns: [], escrow: null, map: null, tiles: null };
}

describe("turn state", () => {
  it("started and acted compare their seq with turnSeq", () => {
    const p = party(0, { startedSeq: 7, actedSeq: 6 });
    expect([turnStarted(p.session), turnActed(p.session)]).toEqual([true, false]);
    p.session.actedSeq = 7;
    expect(turnActed(p.session)).toBe(true);
  });
});

describe("startTurnFor", () => {
  it("the owner of the next player sends it, with that player's accounts", () => {
    const p = party(0);
    expect(startTurnFor(p, ALICE)).toMatchObject({ unit: 0, turnSeq: 7, player: { participation: A(30), character: A(20) } });
    expect(startTurnFor(p, BOB)).toBeNull();
    expect(startTurnFor(p, DM)).toBeNull();
    expect(startTurnFor(party(2), BOB)?.unit).toBe(1);
  });

  it("the DM sends it for a monster, without player accounts", () => {
    expect(startTurnFor(party(1), DM)).toEqual({ unit: 4, turnSeq: 7, player: null });
    expect(startTurnFor(party(1), ALICE)).toBeNull();
  });

  it("nothing once started, while a roll is pending, outside an active party or without a wallet", () => {
    expect(startTurnFor(party(0, { startedSeq: 7 }), ALICE)).toBeNull();
    expect(startTurnFor(party(0, {}, { pendingVrf: true }), ALICE)).toBeNull();
    expect(startTurnFor(party(0, {}, { status: { victory: {} } } as never), ALICE)).toBeNull();
    expect(startTurnFor(party(0, { turnOrderLen: 0 }), ALICE)).toBeNull();
    expect(startTurnFor(party(0), null)).toBeNull();
  });
});

describe("errorName", () => {
  it("reads the Anchor error name of an explainFailure text", () => {
    expect(errorName("TurnNotStarted: The turn has not started: call start_turn first")).toBe("TurnNotStarted");
    expect(errorName("transaction failed: {}")).toBeNull();
    expect(errorName("not confirmed: 5x")).toBeNull();
  });
});

describe("skillCooldowns", () => {
  const part = { cooldowns: Uint8Array.of(0, 1, 0, ...new Array(13).fill(0)) } as never;
  const cds = (...xs: number[]) => Uint8Array.from([...xs, ...new Array(16 - xs.length).fill(0)]);
  const ticked = (unit: number, turnSeq: number) => ({ name: "TurnTicked", data: { unit, turnSeq, cooldowns: cds() } });
  const used = (unit: number, ...xs: number[]) => ({ name: "SkillUsed", data: { unit, cooldowns: cds(...xs) } });

  it("is the participation's value without events of this turn", () => {
    expect(skillCooldowns(part, [], 0, 7).slice(0, 3)).toEqual([0, 1, 0]);
    // A SkillUsed of an earlier turn (before this unit's tick for turn 7) does not count.
    expect(skillCooldowns(part, [used(0, 3), ticked(0, 7)], 0, 7).slice(0, 3)).toEqual([0, 1, 0]);
  });

  it("takes a SkillUsed after this turn's tick at once, for this unit only", () => {
    expect(skillCooldowns(part, [ticked(0, 7), used(1, 0, 0, 4), used(0, 2)], 0, 7).slice(0, 3)).toEqual([2, 1, 0]);
    // The tick of another turn does not open the window.
    expect(skillCooldowns(part, [ticked(0, 5), used(0, 2)], 0, 7)[0]).toBe(0);
  });

  it("labels the turns left", () => {
    expect(cooldownLabel(1)).toBe("ready in 1 turn");
    expect(cooldownLabel(3)).toBe("ready in 3 turns");
  });
});
