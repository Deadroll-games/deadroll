import { describe, expect, it } from "vitest";
import { getAddressDecoder, type Address } from "@solana/kit";
import type { GameSessionAcc, Party, PartyPlayer } from "./game";
import { errorName } from "./send";
import { cooldownLabel, skillCooldowns, skillUsedText, skipText, startTurnFor, tickText, turnActed, turnStarted } from "./turns";

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
  return { address: A(5), session, monsters: [], players: [player(0, ALICE), player(1, BOB)], monsterCooldowns: [], monsterMods: [], monsterDefense: [], resonanceTables: null, burningCells: [], escrow: null, map: null, tiles: null };
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

describe("log texts", () => {
  const st = (kind: string, turns: number) => ({ kind: { [kind]: {} }, turns });
  const empty = st("burning", 0);
  const tick = (over: object) => ({ dotDamage: 0, hp: 10, died: false, expired: 0, states: [empty, empty], ...over });

  it("TurnTicked names the over-time damage, a death and what wore off", () => {
    expect(tickText(tick({ dotDamage: 4, hp: 6, states: [st("burning", 1), empty] }), "Wolf 0")).toBe("Wolf 0 takes 4 burning damage (6 hp)");
    // Bleeding ended with this tick but still dealt damage; chill (bit 1) wore off too.
    expect(tickText(tick({ dotDamage: 7, hp: 0, died: true, expired: (1 << 3) | (1 << 1), states: [st("burning", 2), empty] }), "Ilyra")).toBe(
      "Ilyra takes 7 burning + bleeding damage (0 hp) and dies; chill, bleeding wore off",
    );
    expect(tickText(tick({ expired: 1 << 4 }), "Bran")).toBe("Bran: shock wore off");
  });

  it("a quiet TurnTicked has no line", () => {
    expect(tickText(tick({ states: [st("chill", 2), empty] }), "Bran")).toBeNull();
  });

  it("TurnSkipped and SkillUsed", () => {
    expect(skipText({ state: { freeze: {} } }, "Wolf 1")).toBe("Wolf 1 is frozen: turn skipped");
    expect(skipText({ state: { stun: {} } }, "Bran")).toBe("Bran is stunned: turn skipped");
    expect(skillUsedText({ cooldown: 2 }, "Ilyra", "Frost Burn")).toBe("Ilyra used Frost Burn: cooldown 2 turns");
    expect(skillUsedText({ cooldown: 1 }, "Ilyra", "Frost Bolt")).toBe("Ilyra used Frost Bolt: cooldown 1 turn");
    expect(skillUsedText({ cooldown: 0 }, "Bran", "Jab")).toBe("Bran used Jab");
  });
});
