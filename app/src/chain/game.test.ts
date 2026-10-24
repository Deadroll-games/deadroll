// @vitest-environment node
// Byte-level tests for the demo builders (game.ts) and the sdk codecs they rely on, checked against
// Anchor's discriminator rule and hand-built encodings in the order of the programs' Accounts structs.
import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { AccountRole, generateKeyPairSigner, getAddressDecoder, getAddressEncoder, type Address, type Instruction, type ReadonlyUint8Array } from "@solana/kit";
import { decodeEvent } from "@deadroll/sdk";
import { getCreateCharacterInstructionDataEncoder } from "@deadroll/sdk/character";
import {
  ItemBase,
  SessionStatus,
  getAuraDataEncoder,
  getEffectResolvedEventEncoder,
  getGameSessionDecoder,
  getGameSessionEncoder,
  getHitResolvedEventEncoder,
  getMapSkillDataEncoder,
  getModsTickedEventEncoder,
  getMonsterAuraDataEncoder,
  getMonsterStateDecoder,
  getMonsterStateEncoder,
  getParticipationDecoder,
  getParticipationEncoder,
  getPendingMonsterReactiveDataEncoder,
  getPendingReactiveDataEncoder,
  getPlayerDiedEventEncoder,
  getReactiveDataEncoder,
  getSkillUsedEventEncoder,
  getTurnSkippedEventEncoder,
  getTurnTickedEventEncoder,
  EFFECT_RESOLVED_EVENT_DISCRIMINATOR,
  HIT_RESOLVED_EVENT_DISCRIMINATOR,
  MODS_TICKED_EVENT_DISCRIMINATOR,
  PLAYER_DIED_EVENT_DISCRIMINATOR,
  SKILL_USED_EVENT_DISCRIMINATOR,
  TURN_SKIPPED_EVENT_DISCRIMINATOR,
  TURN_TICKED_EVENT_DISCRIMINATOR,
} from "@deadroll/sdk/session";
import { PROGRAMS, VRF } from "../config";
import { concat, toHex } from "./bytes";
import { decodeEvents } from "./events";
import {
  SKILL_PRESETS,
  buildMapIxs,
  actIx,
  consumeIx,
  decodeAccount,
  endTurnIx,
  joinSessionIx,
  setReadyIx,
  mockVrfSigner,
  moveIx,
  partyAccountsOf,
  startTurnIx,
  computeLimitIx,
  deathAccountsOf,
  withDeaths,
  FINALIZE_CU,
  textBytes,
  type AccountName,
  type Actor,
} from "./game";
import { toHitResolved } from "./hit";
import { toEffectResolved } from "./support";
import { Effect } from "./skillcheck";
import { bagPda, bestiaryPda, gameConfigPda, monsterSkillsPda, rewardsPda, monstersPda, participationPda, programIdentityPda, sessionAuthorityPda, sessionTokenPda, skillExtrasPda, skillSetPda } from "./pda";
import { affordable, pathArgs, reachable, route } from "./route";
import { explainFailure } from "./send";
import { Applied, Aspect, Class, HitOutcome, JumpMode, MAP_MAX, ModKind, OpKind, StateKind, Tile } from "./types";

const sha = (s: string) => createHash("sha256").update(s).digest("hex").slice(0, 16);
const A = (n: number) => new Uint8Array(32).fill(n);
const addrOf = (n: number): Address => getAddressDecoder().decode(A(n));
const enc = getAddressEncoder();
const pk = (a: Address) => enc.encode(a) as Uint8Array;
const ixDisc = (name: string) => Buffer.from(sha(`global:${name}`), "hex");

type Fixed = { fixedSize: number; decode(bytes: ReadonlyUint8Array): unknown };
/** A zero value of a fixed-size codec; `prefix` is an event's discriminator (its decoder checks it). */
function zeroOf(decoder: Fixed, prefix: ReadonlyUint8Array = new Uint8Array()): any {
  const bytes = new Uint8Array(decoder.fixedSize);
  bytes.set(prefix);
  return decoder.decode(bytes);
}
const ACCOUNT_CODECS = {
  GameSession: [getGameSessionEncoder(), getGameSessionDecoder()],
  Participation: [getParticipationEncoder(), getParticipationDecoder()],
  MonsterState: [getMonsterStateEncoder(), getMonsterStateDecoder()],
} as const;
/** An account edited from zero, its bytes and the app's decode of them. */
function roundTrip(name: keyof typeof ACCOUNT_CODECS & AccountName, edit: (v: any) => void): { bytes: Uint8Array; back: any } {
  const [encoder, decoder] = ACCOUNT_CODECS[name];
  const v = zeroOf(decoder);
  edit(v);
  const bytes = Uint8Array.from((encoder as { encode(v: unknown): ReadonlyUint8Array }).encode(v));
  return { bytes, back: decodeAccount(name, bytes) };
}
/** A session event edited from zero, its bytes and the sdk's decode of them. */
function sessionEvent(
  codec: { encoder: { encode(v: any): ReadonlyUint8Array; fixedSize?: number; maxSize?: number }; disc: ReadonlyUint8Array },
  edit: (v: any) => void,
): { bytes: Uint8Array; name: string; data: any } {
  // Options decode as None from zeros, so the largest size fits every event.
  const zero = new Uint8Array(codec.encoder.fixedSize ?? codec.encoder.maxSize!);
  zero.set(codec.disc);
  const v = decodeEvent("session", zero)!.data as any;
  v.session = addrOf(3);
  edit(v);
  const bytes = Uint8Array.from(codec.encoder.encode(v));
  return { bytes, ...decodeEvent("session", bytes)! };
}
const EVENTS = {
  TurnTicked: { encoder: getTurnTickedEventEncoder(), disc: TURN_TICKED_EVENT_DISCRIMINATOR },
  TurnSkipped: { encoder: getTurnSkippedEventEncoder(), disc: TURN_SKIPPED_EVENT_DISCRIMINATOR },
  SkillUsed: { encoder: getSkillUsedEventEncoder(), disc: SKILL_USED_EVENT_DISCRIMINATOR },
  EffectResolved: { encoder: getEffectResolvedEventEncoder(), disc: EFFECT_RESOLVED_EVENT_DISCRIMINATOR },
  ModsTicked: { encoder: getModsTickedEventEncoder(), disc: MODS_TICKED_EVENT_DISCRIMINATOR },
  HitResolved: { encoder: getHitResolvedEventEncoder(), disc: HIT_RESOLVED_EVENT_DISCRIMINATOR },
  PlayerDied: { encoder: getPlayerDiedEventEncoder(), disc: PLAYER_DIED_EVENT_DISCRIMINATOR },
};

describe("sdk", () => {
  it("event and external discriminators follow Anchor's sha256 rule", () => {
    for (const [name, e] of Object.entries(EVENTS)) expect(toHex(Uint8Array.from(e.disc)), name).toBe(sha(`event:${name}`));
  });
});

describe("session builders", () => {
  const session = addrOf(7);
  const character = addrOf(8);

  it("move_unit: 16 direction bytes, len, accounts in Accounts order", async () => {
    const signer = await generateKeyPairSigner();
    const actor: Actor = { signer, token: addrOf(9) };
    const steps = [1, 1, 2].map((dir) => ({ dir, cost: 1, to: { x: 0, y: 0 } }));
    const ix = await moveIx(actor, session, character, addrOf(10), steps);
    const want = concat(ixDisc("move_unit"), Uint8Array.from([1, 1, 2, ...new Array(13).fill(0)]), Uint8Array.of(3));
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(want));
    expect(ix.accounts!.map((a) => [a.address, a.role])).toEqual([
      [signer.address, AccountRole.WRITABLE_SIGNER],
      [session, AccountRole.WRITABLE],
      [await participationPda(session, character), AccountRole.WRITABLE],
      [await monstersPda(session), AccountRole.READONLY],
      [addrOf(10), AccountRole.READONLY],
      [addrOf(9), AccountRole.READONLY],
      // Patch 6: a monster's EnemyAdjacent reaction is a VRF roll the mover pays for.
      [await programIdentityPda(), AccountRole.READONLY],
      [VRF.oracleQueue, AccountRole.WRITABLE],
      [VRF.program, AccountRole.READONLY],
      [VRF.slotHashes, AccountRole.READONLY],
      ['11111111111111111111111111111111', AccountRole.READONLY],
    ]);
  });

  it("act: skill slot and target, VRF accounts after the session token, the map layout last", async () => {
    const signer = await generateKeyPairSigner();
    const ix = await actIx({ signer, token: null }, session, character, 1, 3, null, addrOf(13));
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(concat(ixDisc("act"), Uint8Array.of(1, 3))));
    const addrs = ix.accounts!.map((a) => a.address);
    expect(addrs[4]).toBe(await skillSetPda(character));
    // The bag rides along to the callback, which burns it if the character dies.
    expect(addrs[5]).toBe(await bagPda(character));
    // No session token: Anchor's None is the program id.
    expect(addrs[7]).toBe(PROGRAMS.session);
    expect(addrs.slice(8, 13)).toEqual([await programIdentityPda(), VRF.oracleQueue, VRF.program, VRF.slotHashes, "11111111111111111111111111111111"]);
    expect(ix.accounts![9]!.role).toBe(AccountRole.WRITABLE);
    // An enemy target (or a self-cast): no ally account, the optional is None.
    expect(ix.accounts).toHaveLength(15);
    expect(ix.accounts![13]).toEqual({ address: PROGRAMS.session, role: AccountRole.READONLY });
    // Patch 5: the layout, for the walls that end a line.
    expect(ix.accounts![14]).toEqual({ address: addrOf(13), role: AccountRole.READONLY });
  });

  it("act: a buff on another ally targets its player slot and passes its participation last", async () => {
    const signer = await generateKeyPairSigner();
    const ally = addrOf(14);
    const ix = await actIx({ signer, token: null }, session, character, 2, 1, ally, addrOf(13));
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(concat(ixDisc("act"), Uint8Array.of(2, 1))));
    expect(ix.accounts![13]).toEqual({ address: ally, role: AccountRole.READONLY });
  });

  it("end_turn for a monster passes None for the participation", async () => {
    const signer = await generateKeyPairSigner();
    const ix = await endTurnIx({ signer, token: addrOf(9) }, session, null);
    expect(ix.accounts![3]).toEqual({ address: PROGRAMS.session, role: AccountRole.READONLY });
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(ixDisc("end_turn")));
  });

  it("patch 6: start_turn finalizes the pending deaths: a writable Character, SkillSet and Bag per queued slot, a compute limit in front", async () => {
    const chars = [addrOf(30), addrOf(31), addrOf(32)];
    const s = { core: { playerCount: 3, players: chars }, pendingDeaths: 0b101 } as never;
    const deaths = await deathAccountsOf(s);
    expect(deaths).toEqual([
      { address: chars[0], writable: true },
      { address: await skillSetPda(chars[0]!), writable: true },
      { address: await bagPda(chars[0]!), writable: true },
      { address: chars[2], writable: true },
      { address: await skillSetPda(chars[2]!), writable: true },
      { address: await bagPda(chars[2]!), writable: true },
    ]);
    const payer = await generateKeyPairSigner();
    const ix = await startTurnIx(session, null, payer, deaths);
    expect(ix.accounts!.slice(-6).map((a) => [a.address, a.role])).toEqual(deaths.map((d) => [d.address, AccountRole.WRITABLE]));
    const [limit, same] = withDeaths(deaths, ix);
    expect(same).toBe(ix);
    expect(Array.from(limit!.data as Uint8Array)).toEqual(Array.from(computeLimitIx(FINALIZE_CU).data as Uint8Array));
    expect((limit!.data as Uint8Array)[0]).toBe(2);
    expect(withDeaths([], ix)).toEqual([ix]);
  });

  it("start_turn: no args, the participation and character of a player turn, its SkillSet and Bag, then the payer and the VRF accounts", async () => {
    const participation = addrOf(13);
    const payer = await generateKeyPairSigner();
    const ix = await startTurnIx(session, { participation, character }, payer);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(ixDisc("start_turn")));
    expect(ix.accounts!.map((a) => [a.address, a.role])).toEqual([
      [session, AccountRole.WRITABLE],
      [await monstersPda(session), AccountRole.WRITABLE],
      [participation, AccountRole.WRITABLE],
      [character, AccountRole.WRITABLE],
      [await sessionAuthorityPda(), AccountRole.READONLY],
      [await gameConfigPda(), AccountRole.READONLY],
      [PROGRAMS.character, AccountRole.READONLY],
      // Burned if over-time damage kills.
      [await skillSetPda(character), AccountRole.WRITABLE],
      [await bagPda(character), AccountRole.WRITABLE],
      // Patch 5: pays the VRF fee when over-time damage triggers a reactive skill.
      [payer.address, AccountRole.WRITABLE_SIGNER],
      [await programIdentityPda(), AccountRole.READONLY],
      [VRF.oracleQueue, AccountRole.WRITABLE],
      [VRF.program, AccountRole.READONLY],
      [VRF.slotHashes, AccountRole.READONLY],
      ["11111111111111111111111111111111", AccountRole.READONLY],
    ]);
  });

  it("start_turn for a monster passes None for the participation, the character, SkillSet and Bag", async () => {
    const ix = await startTurnIx(session, null, await generateKeyPairSigner());
    expect(ix.accounts![2]).toEqual({ address: PROGRAMS.session, role: AccountRole.READONLY });
    expect(ix.accounts![3]).toEqual({ address: PROGRAMS.session, role: AccountRole.READONLY });
    expect(ix.accounts![7]).toEqual({ address: PROGRAMS.session, role: AccountRole.READONLY });
    expect(ix.accounts![8]).toEqual({ address: PROGRAMS.session, role: AccountRole.READONLY });
  });

  it("join_session: contribution u64 LE and the actor key; no CPI, the character read only", async () => {
    const player = await generateKeyPairSigner();
    const ix = await joinSessionIx(player, session, character, 50_000_000n, addrOf(11));
    expect(ix.accounts![2]).toEqual({ address: character, role: AccountRole.READONLY });
    expect(ix.accounts!.at(-1)).toEqual({ address: "11111111111111111111111111111111", role: AccountRole.READONLY });
    const amount = new Uint8Array(8);
    new DataView(amount.buffer).setBigUint64(0, 50_000_000n, true);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(concat(ixDisc("join_session"), amount, A(11))));
    expect(ix.accounts![0]!.role).toBe(AccountRole.WRITABLE_SIGNER);
  });

  it("set_ready: the flag and the 6-slot selection; the character (w) and its SkillSet for the lock CPI, its SkillExtras last", async () => {
    const player = await generateKeyPairSigner();
    const ix = await setReadyIx(player, session, character, true, [0, 1, 2, 3, 4, 5]);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(concat(ixDisc("set_ready"), Uint8Array.of(1, 0, 1, 2, 3, 4, 5))));
    expect(ix.accounts!.map((a) => [a.address, a.role])).toEqual([
      [player.address, AccountRole.READONLY_SIGNER],
      [session, AccountRole.WRITABLE],
      [await participationPda(session, character), AccountRole.WRITABLE],
      [character, AccountRole.WRITABLE],
      [await skillSetPda(character), AccountRole.READONLY],
      [await sessionAuthorityPda(), AccountRole.READONLY],
      [await gameConfigPda(), AccountRole.READONLY],
      [PROGRAMS.character, AccountRole.READONLY],
      [await skillExtrasPda(character), AccountRole.READONLY],
    ]);
    const off = await setReadyIx(player, session, character, false, [0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    expect(toHex(off.data as Uint8Array)).toBe(toHex(concat(ixDisc("set_ready"), Uint8Array.of(0, 255, 255, 255, 255, 255, 255))));
  });

  it("consume_randomness is signed by the mock-vrf identity of programs/session/src/vrf.rs", async () => {
    expect((await mockVrfSigner()).address).toBe("5RotSMue7vx9xBPp3L8iQEKbKSc3ezbY4GqiJxDEe6sN");
    const ix = await consumeIx(session, addrOf(12), character, A(0x5a), addrOf(13), []);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(concat(ixDisc("consume_randomness"), A(0x5a))));
    expect(ix.accounts![0]!.role).toBe(AccountRole.READONLY_SIGNER);
    expect(ix.accounts![3]!.address).toBe(addrOf(12));
    // skill_set and bag of that character: burned by mark_dead if the roll kills it.
    expect(ix.accounts!.slice(5, 7).map((a) => a.role)).toEqual([AccountRole.WRITABLE, AccountRole.WRITABLE]);
    expect(ix.accounts).toHaveLength(12);
    expect(ix.accounts![10]).toEqual({ address: PROGRAMS.session, role: AccountRole.READONLY });
    // The map layout, read-only, last (patch 4).
    expect(ix.accounts![11]).toEqual({ address: addrOf(13), role: AccountRole.READONLY });
  });

  it("consume_randomness: the other living players' Participation (w) and Character after the layout, by slot", async () => {
    const parts = [addrOf(20), addrOf(21), addrOf(22), addrOf(23)];
    const chars = [addrOf(30), addrOf(31), addrOf(32), addrOf(33)];
    // Slot 1 is the pending player (its accounts are named), slot 2 is dead.
    const s = { core: { playerCount: 4, players: chars }, aliveMask: 0b1011, participations: parts } as never;
    const party = await partyAccountsOf(s, parts[1]!);
    expect(party.map((a) => [a.address, a.writable])).toEqual([
      [parts[0], true],
      [chars[0], false],
      [parts[3], true],
      [chars[3], false],
    ]);
    const ix = await consumeIx(session, parts[1]!, chars[1]!, A(1), addrOf(13), party);
    // Since patch 5 an ally comes with the party accounts: the optional is None.
    expect(ix.accounts![10]).toEqual({ address: PROGRAMS.session, role: AccountRole.READONLY });
    expect(ix.accounts!.slice(12).map((a) => [a.address, a.role])).toEqual([
      [parts[0], AccountRole.WRITABLE],
      [chars[0], AccountRole.READONLY],
      [parts[3], AccountRole.WRITABLE],
      [chars[3], AccountRole.READONLY],
    ]);
  });
});

describe("map builders", () => {
  it("patch 11: create_map takes min_players, write_chests sends the points and the boss table, finalize_map reads the rewards table", async () => {
    const dm = await generateKeyPairSigner();
    const tiles = new Uint8Array(1024);
    tiles[0] = Tile.Entrance;
    tiles[3] = Tile.Exit;
    const chests = [
      { x: 1, y: 2, table: 0 },
      { x: 3, y: 1, table: 5 },
    ];
    const b = await buildMapIxs(dm, { width: 4, height: 3, tier: 1, minPlayers: 3, tiles, spawns: [], chests, bossTable: 1 });
    expect(b.ixs.length).toBe(5);
    const [create, , , writeChests, finalize] = b.ixs as [Instruction, Instruction, Instruction, Instruction, Instruction];
    const index = new Uint8Array(4);
    new DataView(index.buffer).setUint32(0, b.index, true);
    expect(toHex(create.data as Uint8Array)).toBe(toHex(concat(ixDisc("create_map"), index, Uint8Array.of(4, 3, 1, 3))));
    // Vec<MapPoint>: u32 LE count, then (kind = Chest 0, x, y, table) each; boss table u8.
    const points = Uint8Array.of(2, 0, 0, 0, 0, 1, 2, 0, 0, 3, 1, 5);
    expect(toHex(writeChests.data as Uint8Array)).toBe(toHex(concat(ixDisc("write_chests"), points, Uint8Array.of(1))));
    expect(writeChests.accounts!.map((a) => [a.address, a.role])).toEqual([
      [dm.address, AccountRole.READONLY_SIGNER],
      [b.map, AccountRole.READONLY],
      [b.layout, AccountRole.WRITABLE],
    ]);
    expect(finalize.accounts!.map((a) => a.address).slice(3)).toEqual([await bestiaryPda(), await monsterSkillsPda(), await rewardsPda()]);
  });
});

describe("create_character", () => {
  it("encodes CreateCharacterArgs as the program reads it", () => {
    const data = getCreateCharacterInstructionDataEncoder().encode({
      name: textBytes("Ilyra", 32),
      race: 1,
      class: Class.Mage,
      predisposition: Aspect.Cold,
      stats: [5, 7, 13, 5],
      appearance: new Array(20).fill({ spriteId: 0x0102, materialRamp: 3, accentRamp: 4, elementRamp: 5 }),
      skills: SKILL_PRESETS[Class.Mage].map((s) => ({ blocks: s.blocks, name: textBytes(s.name, 24), ext: s.ext })),
    });
    expect(toHex(Uint8Array.from(data.subarray(0, 8)))).toBe(sha("global:create_character"));
    // 8 disc + 32 name + race, class, predisposition + 4 x u16 + 20 x LayerRef(5) + vec(6 x (12 blocks + 24 name + 3 ext))
    expect(data.length).toBe(8 + 32 + 3 + 8 + 100 + 4 + 6 * 39);
    expect(Array.from(data.subarray(40, 43))).toEqual([1, 1, Aspect.Cold]);
    expect(Array.from(data.subarray(43, 51))).toEqual([5, 0, 7, 0, 13, 0, 5, 0]);
    expect(Array.from(data.subarray(51, 56))).toEqual([2, 1, 3, 4, 5]);
    const vec = 151;
    expect(Array.from(data.subarray(vec, vec + 4))).toEqual([6, 0, 0, 0]);
    // Frost Burn (second skill): aspects [cold, fire, fire], count 2; ranged, instant, no threshold.
    const frostBurn = vec + 4 + 39;
    expect(Array.from(data.subarray(frostBurn + 5, frostBurn + 9))).toEqual([Aspect.Cold, Aspect.Fire, Aspect.Fire, 2]);
    expect(Array.from(data.subarray(frostBurn + 36, frostBurn + 39))).toEqual([1, 0, 0]);
  });
});

describe("decoders", () => {
  it("GameSession round-trips through the sdk", () => {
    const { bytes, back } = roundTrip("GameSession", (s) => {
      s.core.status = SessionStatus.Active;
      s.core.priceLamports = 100_000_000n;
      s.core.turnOrder[0] = 6;
      s.aliveMask = 3;
      s.xpPool = 70;
    });
    expect(toHex(bytes.subarray(0, 8))).toBe(sha("account:GameSession"));
    expect(back.core.status).toBe(SessionStatus.Active);
    expect(back.core.priceLamports).toBe(100_000_000n);
    expect(back.core.turnOrder[0]).toBe(6);
    expect([back.aliveMask, back.xpPool]).toEqual([3, 70]);
    expect(() => decodeAccount("Participation", bytes)).toThrow(/not a Participation account/);
  });

  it("reads the turn-rule fields appended to GameSession, Participation and MonsterState", () => {
    const ally = addrOf(16);
    const s = roundTrip("GameSession", (v) => {
      v.turnSeq = 9;
      v.actedSeq = 9;
      v.startedSeq = 8;
      v.pendingTarget = ally;
      v.participations[2] = ally;
      v.auras[1][1].turns = 3;
      v.pendingReactive.kind = 2;
      v.pendingDeaths = 0b101;
      v.auraWeights[1][1] = Uint8Array.of(0, 100, 0, 65, 0, 0);
    });
    // Patch 9 appends aura_weights (4 x 2 x 6); the offsets below count from before them.
    const p9 = 4 * 2 * 6;
    expect(Array.from(s.bytes.subarray(-p9 + 18, -p9 + 24))).toEqual([0, 100, 0, 65, 0, 0]);
    expect(Array.from(s.back.auraWeights[1][1])).toEqual([0, 100, 0, 65, 0, 0]);
    s.bytes = s.bytes.subarray(0, -p9);
    expect(s.bytes[s.bytes.length - 1]).toBe(0b101);
    expect(s.back.pendingDeaths).toBe(0b101);
    // Patch 5 appends participations (4 x 32), reactives (4), auras (4 x 2) and pending_reactive;
    // patch 6 pending_monster_reactive.
    // ... and pending_deaths (u8).
    const p6 = getPendingMonsterReactiveDataEncoder().fixedSize + 1;
    const p5 = 4 * 32 + 4 * getReactiveDataEncoder().fixedSize + 8 * getAuraDataEncoder().fixedSize + getPendingReactiveDataEncoder().fixedSize + p6;
    // Before them: acted_seq and started_seq, then pending_target (32).
    expect(Array.from(s.bytes.subarray(-p5 - 40, -p5 - 32))).toEqual([9, 0, 0, 0, 8, 0, 0, 0]);
    expect(Array.from(s.bytes.subarray(-p5 - 32, -p5))).toEqual(Array.from(pk(ally)));
    expect(Array.from(s.bytes.subarray(-p5 + 64, -p5 + 96))).toEqual(Array.from(pk(ally)));
    expect([s.back.turnSeq, s.back.actedSeq, s.back.startedSeq, s.back.pendingTarget]).toEqual([9, 9, 8, ally]);
    expect([s.back.participations[2], s.back.auras[1][1].turns, s.back.pendingReactive.kind]).toEqual([ally, 3, 2]);
    const mod = { kind: ModKind.Weakening, aspect: Aspect.Blood, turns: 3, powerBp: 1_800, source: 2 };
    const p = roundTrip("Participation", (v) => {
      v.cooldowns[1] = 2;
      v.hpMax = 30;
      v.mods[ModKind.Weakening] = mod;
      v.equipment = Uint8Array.of(0, 0xff, 0xff, 3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff);
      v.skills = Uint8Array.of(2, 0, 0xff, 5, 1, 3);
      v.classTier = 9;
      v.accuracy = { aspect: Aspect.Fire, turns: 3, bonus: 4 };
      v.mapSkills[5].apCost = 3;
      v.resistMod = { data: { kind: ModKind.Resistance, aspect: Aspect.Piercing, turns: 2, powerBp: 3_000, source: 1 }, weightsPct: Uint8Array.of(0, 0, 0, 100, 0, 100) };
    });
    // Patch 9 appends resist_mod (TimedModData + 6 weights), patch 5 map_skills (6 x
    // MapSkillData) before it; the offsets below count from before them.
    expect(Array.from(p.bytes.subarray(-14, -11))).toEqual([ModKind.Resistance, Aspect.Piercing, 2]);
    expect(Array.from(p.bytes.subarray(-6))).toEqual([0, 0, 0, 100, 0, 100]);
    expect(p.back.resistMod.data.powerBp).toBe(3_000);
    const mapSkill = getMapSkillDataEncoder().fixedSize;
    expect(p.back.mapSkills).toHaveLength(6);
    expect(p.back.mapSkills[5].apCost).toBe(3);
    p.bytes = p.bytes.subarray(0, -6 * mapSkill - 14);
    expect(p.back.cooldowns).toHaveLength(16);
    // cooldowns (16), hp_max (4), mods (6 x 8 bytes), equipment (11), skills (6), class_tier (1),
    // then accuracy (aspect, turns, bonus u32 = 6, patch 4).
    expect(Array.from(p.bytes.subarray(-92, -90))).toEqual([0, 2]);
    expect(Array.from(p.bytes.subarray(-76, -72))).toEqual([30, 0, 0, 0]);
    // TimedModData: kind, aspect, turns, power_bp u32, source = 8 bytes.
    expect(Array.from(p.bytes.subarray(-72 + 8 * 3, -72 + 8 * 4))).toEqual([ModKind.Weakening, Aspect.Blood, 3, 0x08, 0x07, 0, 0, 2]);
    expect(Array.from(p.bytes.subarray(-24, -13))).toEqual([0, 0xff, 0xff, 3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    expect(Array.from(p.bytes.subarray(-13, -7))).toEqual([2, 0, 0xff, 5, 1, 3]);
    expect(Array.from(p.bytes.subarray(-7, -6))).toEqual([9]);
    expect(Array.from(p.bytes.subarray(-6))).toEqual([Aspect.Fire, 3, 4, 0, 0, 0]);
    expect(p.back.accuracy).toEqual({ aspect: Aspect.Fire, turns: 3, bonus: 4 });
    expect(p.back.classTier).toBe(9);
    expect(Array.from(p.back.skills)).toEqual([2, 0, 0xff, 5, 1, 3]);
    expect(p.back.hpMax).toBe(30);
    expect(Array.from(p.back.equipment)).toEqual([0, 0xff, 0xff, 3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    expect(p.back.mods[ModKind.Weakening]).toEqual(mod);
    const m = roundTrip("MonsterState", (v) => {
      v.cooldowns[2] = Uint8Array.of(1, 0, 3, 0);
      v.mods[2][ModKind.Slow] = { kind: ModKind.Slow, aspect: Aspect.Cold, turns: 2, powerBp: 2_500, source: 1 };
      v.defense[1] = { resist: 2, immuneAspects: 0, immuneStates: 4 };
      v.burningCells[0] = { x: 3, y: 4, untilRound: 7, power: 5, source: 1 };
      v.records[1] = Uint8Array.from({ length: 48 }, (_, i) => i);
      v.skillTable[0] = Uint8Array.from({ length: 16 }, (_, i) => 100 + i);
      v.skillCount = 1;
      v.skills[1] = Uint8Array.of(0, 0xff, 0xff, 0xff, 0xff, 0xff, 0);
      v.skillCooldowns[1] = Uint8Array.of(2, 0, 0, 0, 0, 0, 0);
      v.auras[1][0].aura.turns = 255;
      v.auras[1][0].kinOnly = true;
      v.resistMods[3] = { data: { kind: ModKind.Resistance, aspect: Aspect.Cold, turns: 1, powerBp: 2_000, source: 7 }, weightsPct: Uint8Array.of(0, 100, 0, 0, 0, 0) };
      v.auraWeights[1][0] = Uint8Array.of(100, 0, 0, 0, 0, 0);
    });
    // Patch 9 appends resist_mods (24 x 14) and aura_weights (24 x 2 x 6).
    const p9m = 24 * 14 + 24 * 2 * 6;
    expect(m.bytes[m.bytes.length - p9m + 3 * 14]).toBe(ModKind.Resistance);
    expect(m.bytes[m.bytes.length - 24 * 2 * 6 + 12]).toBe(100);
    expect(Array.from(m.back.resistMods[3].weightsPct)).toEqual([0, 100, 0, 0, 0, 0]);
    expect(Array.from(m.back.auraWeights[1][0])).toEqual([100, 0, 0, 0, 0, 0]);
    m.bytes = m.bytes.subarray(0, -p9m);
    // Patch 6 appends records (24 x 48), skill_table (32 x 16), skill_count, skills and
    // skill_cooldowns (24 x 7 each), auras (24 x 2).
    const p6m = 24 * 48 + 32 * 16 + 1 + 2 * 24 * 7 + 48 * getMonsterAuraDataEncoder().fixedSize;
    expect(Array.from(m.bytes.subarray(-p6m + 48, -p6m + 52))).toEqual([0, 1, 2, 3]);
    expect(m.bytes[m.bytes.length - p6m + 24 * 48]).toBe(100);
    expect(m.bytes[m.bytes.length - p6m + 24 * 48 + 32 * 16]).toBe(1);
    expect([m.back.skillCount, Array.from(m.back.skills[1]), Array.from(m.back.skillCooldowns[1]).slice(0, 1)]).toEqual([1, [0, 0xff, 0xff, 0xff, 0xff, 0xff, 0], [2]]);
    expect([m.back.auras[1][0].aura.turns, m.back.auras[1][0].kinOnly]).toEqual([255, true]);
    m.bytes = m.bytes.subarray(0, -p6m);
    expect(m.back.cooldowns).toHaveLength(24);
    expect(Array.from(m.back.cooldowns[2])).toEqual([1, 0, 3, 0]);
    // After the mods (24 x 6 x 8): defense (24 x 3), the tables (48 + 192 + 576), burning cells (16 x 9).
    const tail = 24 * 3 + 48 + 192 + 576 + 16 * 9;
    expect(m.back.mods).toHaveLength(24);
    expect(Array.from(m.bytes.subarray(-tail - 1152 + 2 * 48 + 8, -tail - 1152 + 2 * 48 + 11))).toEqual([ModKind.Slow, Aspect.Cold, 2]);
    expect(Array.from(m.bytes.subarray(-tail + 3, -tail + 6))).toEqual([2, 0, 4]);
    expect(Array.from(m.bytes.subarray(-16 * 9, -16 * 9 + 9))).toEqual([3, 4, 7, 0, 5, 0, 0, 0, 1]);
    expect(m.back.burningCells[0]).toEqual({ x: 3, y: 4, untilRound: 7, power: 5, source: 1 });
    expect(m.back.mods[2][ModKind.Slow].turns).toBe(2);
    // A pre-upgrade account (no trailing fields) no longer decodes.
    expect(() => decodeAccount("Participation", p.bytes)).toThrow();
  });

  it("decodes TurnTicked, TurnSkipped and SkillUsed", () => {
    const tick = sessionEvent(EVENTS.TurnTicked, (v) => {
      Object.assign(v, { unit: 5, turnSeq: 12, dotDamage: 4, hp: 6, died: false, skipped: true, expired: 1 << 1 });
      v.states[0] = { kind: StateKind.Burning, turns: 1, power: 4, source: 1 };
      v.cooldowns[0] = 1;
    });
    expect(tick.name).toBe("TurnTicked");
    expect(tick.data).toMatchObject({ unit: 5, turnSeq: 12, dotDamage: 4, hp: 6, died: false, skipped: true, expired: 2 });
    expect(tick.data.states[0]).toEqual({ kind: StateKind.Burning, turns: 1, power: 4, source: 1 });
    expect(tick.data.cooldowns[0]).toBe(1);
    const skip = sessionEvent(EVENTS.TurnSkipped, (v) => Object.assign(v, { unit: 4, turnSeq: 3, state: StateKind.Stun }));
    expect(skip.data).toMatchObject({ unit: 4, turnSeq: 3, state: StateKind.Stun });
    const used = sessionEvent(EVENTS.SkillUsed, (v) => {
      Object.assign(v, { unit: 1, skillSlot: 1, cooldown: 2 });
      v.cooldowns[1] = 3;
    });
    expect(used.data).toMatchObject({ unit: 1, skillSlot: 1, cooldown: 2 });
    expect(used.data.cooldowns[1]).toBe(3);
  });

  it("decodes EffectResolved and ModsTicked", () => {
    const sap = sessionEvent(EVENTS.EffectResolved, (v) => {
      Object.assign(v, { caster: 2, target: 5, actionCounter: 7, skillSlot: 1, effect: Effect.Weakening, rider: false, applied: Applied.Added, hp: 9 });
      Object.assign(v.roll, { outcome: HitOutcome.Hit, d20: 20, attackTotal: 23, defense: 12, crit: true, critRoll: null, chanceBp: 0, chanceRoll: null, d100: 40, amount: 2_160, rollsUsed: 2 });
      v.mods[ModKind.Weakening] = { kind: ModKind.Weakening, aspect: Aspect.Blood, turns: 3, powerBp: 2_160, source: 2 };
    });
    expect(sap.name).toBe("EffectResolved");
    const e = toEffectResolved(sap.data);
    expect([e.caster, e.target, e.actionCounter, e.skillSlot, e.effect, e.rider, e.applied, e.hp]).toEqual([2, 5, 7, 1, Effect.Weakening, false, Applied.Added, 9]);
    expect(e.roll).toMatchObject({ outcome: HitOutcome.Hit, d20: 20, crit: true, critRoll: null, chanceRoll: null, d100: 40, amount: 2_160 });
    expect(e.mods).toEqual([{ kind: ModKind.Weakening, aspect: Aspect.Blood, turns: 3, powerBp: 2_160, source: 2 }]);
    const mend = toEffectResolved(
      sessionEvent(EVENTS.EffectResolved, (v) => {
        Object.assign(v, { caster: 2, target: 0, effect: Effect.Heal, applied: Applied.None, hp: 25 });
        Object.assign(v.roll, { outcome: HitOutcome.Hit, critRoll: 9_000, d100: 70, amount: 7 });
      }).data,
    );
    expect([mend.effect, mend.roll.critRoll, mend.roll.amount, mend.mods]).toEqual([Effect.Heal, 9_000, 7, []]);
    const tick = sessionEvent(EVENTS.ModsTicked, (v) => {
      Object.assign(v, { unit: 2, turnSeq: 11, expired: 1 << ModKind.Haste, actionPoints: 5 });
    });
    expect(tick.data).toMatchObject({ unit: 2, turnSeq: 11, expired: 1, actionPoints: 5 });
    expect(tick.data.mods).toHaveLength(6);
  });

  it("HitResolved maps to the app HitResult, used entries only", () => {
    const decoded = sessionEvent(EVENTS.HitResolved, (ev) => {
      ev.attacker = 1;
      ev.target = 4;
      ev.actionCounter = 2;
      const r = ev.result;
      r.outcome = HitOutcome.Hit;
      r.d20 = 20;
      r.crit = true;
      r.tintRollCount = 1;
      r.tintRolls[0] = { base: Aspect.Fire, tint: Aspect.Cold, carrier: 0, tier: 14, chanceBp: 700, roll: 39, success: true };
      r.resonances[0] = {
        base: Aspect.Fire,
        tint: Aspect.Cold,
        tier: 14,
        carrier: 0,
        id: 1,
        active: true,
        secondTint: Aspect.Blood,
        secondTier: 12,
        secondCarrier: 2,
        generated: false,
      };
      // Pale Flame: Burning longer, and the accuracy buff.
      r.resOps[0][0] = { ...r.resOps[0][0], kind: OpKind.NativeTurns, acted: true, amount: 1, tint: Aspect.Cold, carrier: 0, tier: 14 };
      r.resOps[0][1] = { ...r.resOps[0][1], kind: OpKind.AccuracyBuff, arg: 0, acted: true, amount: 1, origin: 0 };
      r.jumpCount = 1;
      r.jumps[0] = { mode: JumpMode.Line, state: { kind: StateKind.Shock, turns: 2, power: 2000, source: 1 }, filter: StateKind.Shock, limit: 1, base: Aspect.Lightning };
      r.knockback = 1;
      r.accuracyBuff = { aspect: Aspect.Fire, turns: 3, bonus: 1 };
      r.packetCount = 2;
      r.packets[1].aspect = Aspect.Fire;
      r.packets[1].rolled = 30_000n;
      r.damage = 8;
      r.states[0] = { kind: StateKind.Chill, turns: 2, power: 1000, source: 1 };
      ev.items.attacker = { weapon: ItemBase.Sword, armor: 0, defense: 0, attack: 1 };
      ev.items.defender.armor = 10;
    });
    expect(decoded.name).toBe("HitResolved");
    const h = toHitResolved(decoded.data);
    expect([h.attacker, h.target, h.actionCounter]).toEqual([1, 4, 2]);
    expect(h.result.outcome).toBe(HitOutcome.Hit);
    expect(h.result.tintRolls).toHaveLength(1);
    expect(h.result.resonances[0]).toMatchObject({ base: Aspect.Fire, tint: Aspect.Cold, active: true, secondTint: Aspect.Blood, secondTier: 12, secondCarrier: 2 });
    expect(h.result.resOps[0]!.map((o) => [o.kind, o.acted, o.amount])).toEqual([
      [OpKind.NativeTurns, true, 1],
      [OpKind.AccuracyBuff, true, 1],
    ]);
    expect(h.result.resOps[1]).toEqual([]);
    expect(h.result.jumps).toEqual([{ mode: 2, state: { kind: 4, turns: 2, power: 2000, source: 1 }, filter: 4, limit: 1, base: Aspect.Lightning }]);
    expect([h.result.knockback, h.result.accuracyBuff]).toEqual([1, { aspect: Aspect.Fire, turns: 3, bonus: 1 }]);
    expect(h.result.resonances[1]).toBeNull();
    expect(h.result.critRoll).toBeNull();
    expect(h.result.packets).toHaveLength(2);
    expect(h.result.packets[1]).toMatchObject({ aspect: Aspect.Fire, rolled: 30_000n });
    expect(h.result.states).toHaveLength(1);
    expect(h.result.damage).toBe(8);
    // HitResolved.items, after the result: weapon Option<ItemBase>, armor, defense, attack.
    expect(h.items).toEqual({ attacker: { weapon: "sword", armor: 0, defense: 0, attack: 1 }, defender: { weapon: null, armor: 10, defense: 0, attack: 0 } });
    const tail = decoded.bytes.subarray(-2 * 13 - 1);
    expect(Array.from(tail.subarray(0, 2))).toEqual([1, 0]);
  });

  it("a resonance second tint of Fire (0) is a tint, None is none", () => {
    const decoded = sessionEvent(EVENTS.HitResolved, (ev) => {
      const r = { base: Aspect.Cold, tint: Aspect.Blood, tier: 1, carrier: 0, id: 2, active: true, secondTier: 3, secondCarrier: 1, generated: false };
      ev.result.resonances[0] = { ...r, secondTint: Aspect.Fire };
      ev.result.resonances[1] = { ...r, secondTint: null };
    });
    const [first, second] = toHitResolved(decoded.data).result.resonances;
    expect([first?.secondTint, second?.secondTint]).toEqual([Aspect.Fire, null]);
  });

  it("decodeEvents reads session events inside the session invocation only", () => {
    const ev = sessionEvent(EVENTS.PlayerDied, (v) => (v.slot = 1));
    const b64 = Buffer.from(ev.bytes).toString("base64");
    const logs = [
      `Program ${PROGRAMS.session} invoke [1]`,
      `Program data: ${b64}`,
      `Program ${PROGRAMS.character} invoke [2]`,
      `Program data: ${b64}`,
      `Program ${PROGRAMS.character} success`,
      `Program ${PROGRAMS.session} success`,
    ];
    const evs = decodeEvents(logs);
    expect(evs.map((e) => [e.program, e.name])).toEqual([["session", "PlayerDied"]]);
    expect(evs[0]!.data.slot).toBe(1);
  });
});

describe("route", () => {
  // 5x3 room, a wall in the middle column except the bottom row, water costs 2.
  const tiles = new Uint8Array(MAP_MAX * MAP_MAX).fill(Tile.Wall);
  for (let y = 0; y < 3; y++) for (let x = 0; x < 5; x++) tiles[y * MAP_MAX + x] = Tile.Floor;
  tiles[0 * MAP_MAX + 2] = Tile.Wall;
  tiles[1 * MAP_MAX + 2] = Tile.Wall;
  tiles[2 * MAP_MAX + 3] = Tile.Water;
  const tileCost = [1, 0, 2, 1, 1, 1, 1];
  const base = { tiles, width: 5, height: 3, tileCost, taken: new Set<number>() };

  it("finds the cheapest path around walls and through water", () => {
    const steps = route({ ...base, from: { x: 0, y: 0 }, goal: (x, y) => x === 4 && y === 0 })!;
    // Around the wall through (2,2); its only way east is the water at (3,2), which costs 2.
    expect(steps.at(-1)!.to).toEqual({ x: 4, y: 0 });
    expect(steps.find((s) => s.to.x === 3 && s.to.y === 2)?.cost).toBe(2);
    expect(steps.reduce((n, s) => n + s.cost, 0)).toBe(9);
  });

  it("cuts a path to the action points and pads move args to 16", () => {
    const steps = route({ ...base, from: { x: 0, y: 0 }, goal: (x, y) => x === 4 && y === 0 })!;
    const take = affordable(steps, 5, 2);
    expect(take.reduce((n, s) => n + s.cost, 0)).toBeLessThanOrEqual(3);
    const args = pathArgs(take);
    expect(args.steps).toHaveLength(16);
    expect(args.len).toBe(take.length);
  });

  it("reachable stays within the action points and off taken cells", () => {
    const cells = reachable({ ...base, from: { x: 0, y: 0 }, ap: 2, taken: new Set([1 * MAP_MAX + 0]) });
    expect(cells).toContainEqual({ x: 1, y: 0 });
    expect(cells).toContainEqual({ x: 1, y: 1 });
    expect(cells).not.toContainEqual({ x: 0, y: 1 });
  });
});

describe("errors", () => {
  it("names the Anchor error from logs", () => {
    const logs = ["Program log: AnchorError occurred. Error Code: NotYourTurn. Error Number: 6005. Error Message: Not this unit's turn."];
    expect(explainFailure(new Error("x"), logs)).toBe("NotYourTurn: Not this unit's turn.");
  });

  it("names a custom error of the failing instruction's program without logs", async () => {
    const ix: Instruction = { programAddress: PROGRAMS.character, accounts: [], data: new Uint8Array() };
    expect(explainFailure(new Error("Custom program error: #6016 (instruction #1)"), [], [ix])).toBe("AlreadyFrozen: Tombstone NFT is already frozen");
  });

  it("knows the turn-rule errors of the session program", () => {
    const ix: Instruction = { programAddress: PROGRAMS.session, accounts: [], data: new Uint8Array() };
    const name = (code: number) => explainFailure(new Error(`Custom program error: #${code} (instruction #1)`), [], [ix]).split(":")[0];
    expect([6046, 6047, 6048, 6049].map(name)).toEqual(["AlreadyActed", "SkillOnCooldown", "TurnNotStarted", "TurnAlreadyStarted"]);
  });

  it("knows the ally-target errors of the session program", () => {
    const ix: Instruction = { programAddress: PROGRAMS.session, accounts: [], data: new Uint8Array() };
    const explain = (code: number) => explainFailure(new Error(`Custom program error: #${code} (instruction #1)`), [], [ix]);
    expect(explain(6050)).toBe("InvalidAllyTarget: Target is not a living ally");
    expect(explain(6051)).toBe("AllyAccountMissing: A heal or buff on another ally needs that ally's participation");
    expect(explain(6052)).toBe("UnexpectedAllyAccount: target_participation is only for a heal or buff on another ally");
  });
});

describe("session keys", () => {
  it("token PDA uses session_token_v2 (session-keys 3.1.1 SessionTokenV2::SEED_PREFIX)", async () => {
    const [signer, authority] = [addrOf(1), addrOf(2)];
    const { getProgramDerivedAddress } = await import("@solana/kit");
    const [want] = await getProgramDerivedAddress({
      programAddress: "KeyspM2ssCJbqUhQ4k7sveSiY4WjnYsrXkC8oDbwde5" as Address,
      seeds: [new TextEncoder().encode("session_token_v2"), pk(PROGRAMS.session), pk(signer), pk(authority)],
    });
    expect(await sessionTokenPda(PROGRAMS.session, signer, authority)).toBe(want);
  });
});
