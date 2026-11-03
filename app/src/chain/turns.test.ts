import { describe, expect, it } from "vitest";
import { getAddressDecoder, type Address } from "@solana/kit";
import type { GameSessionAcc, Party, PartyPlayer } from "./game";
import { errorName } from "./send";
import { StateKind } from "./types";
import { SessionStatus } from "@deadroll/sdk/session";
import { auditLeft, cooldownLabel, exitVoteView, revealStepFor, skillCooldowns, skillUsedText, skipText, arenaStepFor, startTurnFor, stuckView, tickText, turnActed, turnStarted } from "./turns";
import { TIMEOUTS } from "./timeouts";

const A = (n: number) => getAddressDecoder().decode(new Uint8Array(32).fill(n)) as Address;
const DM = A(1);
const [ALICE, BOB] = [A(2), A(3)];

/** Two players (slots 0, 1) and a monster (unit 4); turn order 0, 4, 1; `turn` is the index in it. */
function party(turn: number, over: Partial<GameSessionAcc> = {}, core: Partial<GameSessionAcc["core"]> = {}): Party {
  const session = {
    core: { status: SessionStatus.Active, dm: DM, turnOrder: Uint8Array.of(0, 4, 1), turnIndex: turn, pendingVrf: false, ...core },
    turnOrderLen: 3,
    turnSeq: 7,
    startedSeq: 6,
    actedSeq: 6,
    dmActor: A(9),
    // Patch C: no reveal owed, every pack revealed.
    obligationCount: 0,
    packsRevealed: 0xffn,
    arenaPack: 0,
    // Stuck party: the DM present, nothing timed yet.
    dmMissingAt: 0n,
    dmBackAt: 0n,
    dmConfirmMask: 0,
    revealsOwedAt: 0n,
    turnClock: 0n,
    vrfRequest: 0,
    vrfRequestedSlot: 0n,
    ...over,
  } as unknown as GameSessionAcc;
  const player = (slot: number, owner: Address) =>
    ({ slot, character: A(20 + slot), participation: A(30 + slot), part: { player: owner, core: { actor: A(40 + slot) } } }) as unknown as PartyPlayer;
  return { address: A(5), session, monsters: [], players: [player(0, ALICE), player(1, BOB)], monsterCooldowns: [], monsterRecords: [], monsterSkills: [], monsterAuras: [], monsterMods: [], monsterDefense: [], resonanceTables: null, burningCells: [], escrow: null, map: null, tiles: null, points: [], spawns: [], arenas: null, ground: null, secretsBlob: null, board: null };
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
    expect(startTurnFor(party(1), DM)).toEqual({ unit: 4, turnSeq: 7, player: null, finalize: false });
    expect(startTurnFor(party(1), ALICE)).toBeNull();
  });

  it("patch 6: pending deaths: anyone in the party finalizes them, also once the turn started", () => {
    const started = party(0, { startedSeq: 7, pendingDeaths: 0b10 } as never);
    expect(startTurnFor(started, BOB)).toMatchObject({ unit: 0, finalize: true, player: { slot: 0 } });
    expect(startTurnFor(started, DM)).toMatchObject({ unit: 0, finalize: true });
    expect(startTurnFor(started, A(99))).toBeNull();
    expect(startTurnFor(party(1, { pendingDeaths: 1 } as never), ALICE)).toMatchObject({ unit: 4, player: null, finalize: true });
    expect(startTurnFor(party(0, { pendingDeaths: 1 } as never, { pendingVrf: true }), ALICE)).toBeNull();
  });

  it("nothing once started, while a roll is pending, outside an active party or without a wallet", () => {
    expect(startTurnFor(party(0, { startedSeq: 7 }), ALICE)).toBeNull();
    expect(startTurnFor(party(0, {}, { pendingVrf: true }), ALICE)).toBeNull();
    expect(startTurnFor(party(0, {}, { status: SessionStatus.Victory } as never), ALICE)).toBeNull();
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
  const STATES = ["burning", "chill", "freeze", "bleeding", "shock", "stun", "penetration"];
  const st = (kind: string, turns: number) => ({ kind: STATES.indexOf(kind), turns });
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
    expect(skipText({ state: StateKind.Freeze }, "Wolf 1")).toBe("Wolf 1 is frozen: turn skipped");
    expect(skipText({ state: StateKind.Stun }, "Bran")).toBe("Bran is stunned: turn skipped");
    expect(skillUsedText({ cooldown: 2 }, "Ilyra", "Frost Burn")).toBe("Ilyra used Frost Burn: cooldown 2 turns");
    expect(skillUsedText({ cooldown: 1 }, "Ilyra", "Frost Bolt")).toBe("Ilyra used Frost Bolt: cooldown 1 turn");
    expect(skillUsedText({ cooldown: 0 }, "Bran", "Jab")).toBe("Bran used Jab");
  });
});

describe("arenaStepFor (arena update)", () => {
  it("any party member enters an engaged pack's arena and leaves a won one; nothing on the map or in play", () => {
    for (const [mode, step] of [[0, null], [1, "enter"], [2, null], [3, "leave"], [4, "leave"]] as const) {
      expect(arenaStepFor(party(0, { mode }), BOB)).toBe(step);
    }
    // Not a member, a pending roll, deaths waiting: nothing.
    expect(arenaStepFor(party(0, { mode: 1 }), A(77))).toBeNull();
    expect(arenaStepFor(party(0, { mode: 3, pendingDeaths: 1 }), ALICE)).toBeNull();
  });

  it("between the map and an arena no turn starts, but the deaths are finalized", () => {
    expect(startTurnFor(party(0, { mode: 1, startedSeq: 6 }), ALICE)).toBeNull();
    expect(startTurnFor(party(0, { mode: 3, pendingDeaths: 1 }), ALICE)?.finalize).toBe(true);
    // Patch B: after a retreat too.
    expect(startTurnFor(party(0, { mode: 4, startedSeq: 6 }), ALICE)).toBeNull();
  });
});

describe("exitVoteView (patch B)", () => {
  const view = (over: Partial<GameSessionAcc>) =>
    exitVoteView(party(0, { exitKind: 0, fleeMask: 0, lastMoves: 0, retreats: 0, aliveMask: 0b11, ...over }, { playerCount: 2 }).session);
  it("no proposal: nobody voted, every retreat left", () => {
    expect(view({})).toEqual({ kind: null, voted: [], waiting: [], leaving: false, retreatsLeft: 2 });
  });
  it("a retreat proposal: who confirmed and who is still to vote; the dead do not vote", () => {
    expect(view({ exitKind: 1, fleeMask: 0b01, retreats: 1 })).toEqual({ kind: "retreat", voted: [0], waiting: [1], leaving: false, retreatsLeft: 1 });
    expect(view({ exitKind: 2, fleeMask: 0b10, aliveMask: 0b10 })).toMatchObject({ kind: "escape", voted: [1], waiting: [] });
  });
  it("the vote passed: the monsters take their last moves; no retreat below zero", () => {
    expect(view({ exitKind: 1, fleeMask: 0b11, lastMoves: 0b1, retreats: 5 })).toMatchObject({ leaving: true, retreatsLeft: 0 });
  });
});

describe("patch C: the DM's reveals", () => {
  it("a cell first, then the engaged pack; only the DM's tab, never during a roll", () => {
    const p = party(0, { obligationCount: 2, obligations: [70, 71], obligedBy: 1, packsRevealed: 0n, mode: 1, arenaPack: 2 } as never);
    expect(revealStepFor(p, DM)).toEqual({ kind: "cell", leaf: 70, by: 1 });
    expect(revealStepFor(p, ALICE)).toBeNull();
    expect(arenaStepFor(p, ALICE)).toBeNull();
    p.session.obligationCount = 0;
    expect(revealStepFor(p, DM)).toEqual({ kind: "pack", pack: 2 });
    expect(arenaStepFor(p, ALICE)).toBeNull();
    p.session.packsRevealed = 0b100n;
    expect(revealStepFor(p, DM)).toBeNull();
    expect(arenaStepFor(p, ALICE)).toBe("enter");
    p.session.obligationCount = 1;
    p.session.core.pendingVrf = true;
    expect(revealStepFor(p, DM)).toBeNull();
  });

  it("the audit lists what is still hidden", () => {
    // Patch D: the revealed leaves are on the party board.
    const s = party(0, { packsRevealed: 0b10n } as never).session;
    const board = { revealed: (leaf: number) => leaf === 65 };
    expect(auditLeft(s, board, [65, 66, 300], [1, 3])).toEqual({ cells: [66, 300], packs: [3] });
    expect(auditLeft(s, null, [65], [])).toEqual({ cells: [65], packs: [] });
  });
});

describe("stuck party (docs/PARTY_AND_TURNS.md 3.2, 5.3)", () => {
  const T0 = 1_000_000;
  /** Two living players, a 600 s issue deadline, Alice's turn (index 0). */
  const stuck = (over: Record<string, unknown> = {}, core: Record<string, unknown> = {}) =>
    party(0, { aliveMask: 0b11, settledMask: 0, settleTimeoutSecs: 600, mode: 0, ...over } as never, { playerCount: 2, ...core } as never);

  it("a reveal owed long enough: the living players may mark the DM as missing", () => {
    const p = stuck({ obligationCount: 1, revealsOwedAt: BigInt(T0) });
    const due = T0 + TIMEOUTS.revealTimeoutSecs;
    expect(stuckView(p, ALICE, due - 1, null)).toMatchObject({ markFrom: due, mark: [] });
    expect(stuckView(p, ALICE, due, null).mark.map((x) => x.slot)).toEqual([0]);
    expect(stuckView(p, DM, due, null).mark).toEqual([]);
    // Not during a roll, nor for a tombstone.
    expect(stuckView(stuck({ obligationCount: 1, revealsOwedAt: BigInt(T0) }, { pendingVrf: true }), ALICE, due, null).mark).toEqual([]);
    expect(stuckView(stuck({ obligationCount: 1, revealsOwedAt: BigInt(T0), aliveMask: 0b10 }), ALICE, due, null).mark).toEqual([]);
    // Nothing owed: nothing to mark.
    expect(stuckView(stuck(), ALICE, due, null).markFrom).toBeNull();
  });

  it("frozen: no turn start, no reveal, no arena step; the DM may come back, then the players confirm", () => {
    const p = stuck({ obligationCount: 1, dmMissingAt: BigInt(T0) });
    expect([startTurnFor(p, ALICE), revealStepFor(p, DM), arenaStepFor(p, ALICE)]).toEqual([null, null, null]);
    expect(stuckView(p, DM, T0 + 600, null)).toMatchObject({ phase: { kind: "missing", until: T0 + 600 }, dmReturn: true, skip: null });
    expect(stuckView(p, ALICE, T0 + 600, null).dmReturn).toBe(false);
    const back = stuck({ obligationCount: 1, dmMissingAt: BigInt(T0), dmBackAt: BigInt(T0 + 10), dmConfirmMask: 0b10 });
    const v = stuckView(back, ALICE, T0 + 20, null);
    expect(v).toMatchObject({ phase: { kind: "back" }, confirmed: [1], waiting: [0] });
    expect(v.confirm.map((x) => x.slot)).toEqual([0]);
    expect(stuckView(back, BOB, T0 + 20, null).confirm).toEqual([]);
  });

  it("the DM left: the DM settles before the deadline, the players claim after", () => {
    const never = stuck({ dmMissingAt: BigInt(T0) });
    expect(stuckView(never, ALICE, T0 + 600, null).claimFrom).toBeNull();
    const gone = stuckView(never, ALICE, T0 + 601, null);
    expect(gone).toMatchObject({ phase: { kind: "gone", endedAt: T0 }, claimFrom: T0 + 600, dmSettle: false });
    expect(gone.claim.map((x) => x.slot)).toEqual([0]);
    const end = T0 + 10 + TIMEOUTS.dmReturnConfirmSecs;
    const left = stuck({ dmMissingAt: BigInt(T0), dmBackAt: BigInt(T0 + 10), dmConfirmMask: 0b01 });
    expect(stuckView(left, DM, end + 1, null)).toMatchObject({ phase: { kind: "unconfirmed", endedAt: end }, dmSettle: true, claim: [] });
    expect(stuckView(left, DM, end + 601, null).dmSettle).toBe(false);
    expect(stuckView(left, BOB, end + 601, null).claim.map((x) => x.slot)).toEqual([1]);
    // A settled player has nothing to claim.
    expect(stuckView(stuck({ dmMissingAt: BigInt(T0), settledMask: 0b01 }), ALICE, T0 + 601, null).claim).toEqual([]);
  });

  it("an idle unit's turn may be skipped by any participant once the timeout passed", () => {
    const p = stuck({ turnClock: BigInt(T0) });
    const due = T0 + TIMEOUTS.turnTimeoutSecs;
    expect(stuckView(p, BOB, due - 1, null)).toMatchObject({ skipFrom: due, skip: null });
    expect(stuckView(p, BOB, due, null).skip).toMatchObject({ kind: "player", player: { slot: 1 } });
    expect(stuckView(p, DM, due, null).skip).toEqual({ kind: "dm" });
    expect(stuckView(p, A(99), due, null).skip).toBeNull();
    // Not while a reveal is owed, a roll is pending, or between the map and an arena.
    expect(stuckView(stuck({ turnClock: BigInt(T0), obligationCount: 1 }), BOB, due, null).skip).toBeNull();
    expect(stuckView(stuck({ turnClock: BigInt(T0) }, { pendingVrf: true }), BOB, due, null).skip).toBeNull();
    expect(stuckView(stuck({ turnClock: BigInt(T0), mode: 1, packsRevealed: 0xffn }), BOB, due, null).skip).toBeNull();
    // In an arena too.
    expect(stuckView(stuck({ turnClock: BigInt(T0), mode: 2 }), BOB, due, null).skip).not.toBeNull();
  });

  it("a silent oracle: any participant may retry once the slots passed", () => {
    const p = stuck({ vrfRequestedSlot: 500n }, { pendingVrf: true });
    const due = 500n + BigInt(TIMEOUTS.vrfRetrySlots);
    expect(stuckView(p, ALICE, T0, due - 1n).retry).toBeNull();
    expect(stuckView(p, ALICE, T0, due).retry).toMatchObject({ kind: "player" });
    expect(stuckView(p, DM, T0, due).retry).toEqual({ kind: "dm" });
    expect(stuckView(p, ALICE, T0, null).retry).toBeNull();
    expect(stuckView(stuck({ vrfRequestedSlot: 500n }), ALICE, T0, due).retry).toBeNull();
  });
});
