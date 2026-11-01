// @vitest-environment node
// Byte-level tests for the demo builders (game.ts) and the sdk codecs they rely on, checked against
// Anchor's discriminator rule and hand-built encodings in the order of the programs' Accounts structs.
import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { AccountRole, generateKeyPairSigner, getAddressDecoder, getAddressEncoder, type Address, type Instruction, type ReadonlyUint8Array } from "@solana/kit";
import { decodeEvent } from "@deadroll/sdk";
import { getBagDecoder, getBagEncoder, getCreateCharacterInstructionDataEncoder } from "@deadroll/sdk/character";
import { getMapDecoder, getMapEncoder } from "@deadroll/sdk/map";
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
  getRewardCacheDecoder,
  getRewardCacheEncoder,
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
  claimAfterTimeoutIx,
  closeCacheIx,
  leavePartyIx,
  openChestIx,
  settlePartyIx,
  actIx,
  consumeIx,
  decodeAccount,
  endTurnIx,
  joinSessionIx,
  setReadyIx,
  mockVrfSigner,
  movePathIx,
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
import { bagPda, bestiaryPda, gameConfigPda, monsterSkillsPda, rewardCachePda, rewardsPda, monstersPda, participationPda, programIdentityPda, sessionAuthorityPda, sessionTokenPda, skillExtrasPda, skillSetPda, tilesetPda } from "./pda";
import { affordable, extendPath, reachable, route } from "./route";
import { explainFailure } from "./send";
import { Applied, Aspect, Biome, Class, HitOutcome, JumpMode, MAP_MAX, ModKind, OpKind, StateKind, Tile, cellByte } from "./types";
import { SHIPPED_TILE_COST } from "./tiles";

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
  Map: [getMapEncoder(), getMapDecoder()],
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

  it("move_path (patch 13): the unit, the cells as a Vec, accounts in Accounts order", async () => {
    const signer = await generateKeyPairSigner();
    const actor: Actor = { signer, token: addrOf(9) };
    const cells = [{ x: 1, y: 2 }, { x: 2, y: 3 }, { x: 3, y: 3 }];
    const steps = cells.map((to) => ({ cost: 1, to }));
    const vrf = async () => [
      // A reaction of the enemy next to the new cell is a VRF roll the mover pays for.
      [await programIdentityPda(), AccountRole.READONLY],
      [VRF.oracleQueue, AccountRole.WRITABLE],
      [VRF.program, AccountRole.READONLY],
      [VRF.slotHashes, AccountRole.READONLY],
      ["11111111111111111111111111111111", AccountRole.READONLY],
    ];
    const player = await movePathIx(actor, session, addrOf(10), { kind: "player", slot: 2, character }, steps);
    // unit u8, then Vec<Cell>: u32 LE count and (x, y) per cell.
    const want = concat(ixDisc("move_path"), Uint8Array.of(2, 3, 0, 0, 0, 1, 2, 2, 3, 3, 3));
    expect(toHex(player.data as Uint8Array)).toBe(toHex(want));
    expect(player.accounts!.map((a) => [a.address, a.role])).toEqual([
      [signer.address, AccountRole.WRITABLE_SIGNER],
      [session, AccountRole.WRITABLE],
      [await participationPda(session, character), AccountRole.WRITABLE],
      [await monstersPda(session), AccountRole.WRITABLE],
      [addrOf(10), AccountRole.READONLY],
      [addrOf(9), AccountRole.READONLY],
      ...(await vrf()),
    ]);
    // A monster: unit MONSTER_BASE + index, no participation (the program id in its place).
    const dm: Actor = { signer, token: null };
    const monster = await movePathIx(dm, session, addrOf(10), { kind: "monster", index: 3 }, steps.slice(0, 1));
    expect(toHex(monster.data as Uint8Array)).toBe(toHex(concat(ixDisc("move_path"), Uint8Array.of(7, 1, 0, 0, 0, 1, 2))));
    const roles = monster.accounts!.map((a) => a.address);
    expect(roles[2]).toBe(PROGRAMS.session);
    expect(roles[5]).toBe(PROGRAMS.session);
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
    // Patch 11: the character's reward cache, created or rebound here.
    expect(ix.accounts![4]).toEqual({ address: await rewardCachePda(character), role: AccountRole.WRITABLE });
  });

  it("patch 11: join_session passes the GameSession of every open cache entry, read only, after its accounts", async () => {
    const player = await generateKeyPairSigner();
    const ix = await joinSessionIx(player, session, character, 1n, addrOf(0), [addrOf(41), addrOf(42)]);
    expect(ix.accounts!.slice(-2)).toEqual([
      { address: addrOf(41), role: AccountRole.READONLY },
      { address: addrOf(42), role: AccountRole.READONLY },
    ]);
  });

  it("patch 11: leave_party: no args; the owner signs, the character (w) and the CPI accounts", async () => {
    const owner = await generateKeyPairSigner();
    const ix = await leavePartyIx(owner, session, character);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(ixDisc("leave_party")));
    expect(ix.accounts!.map((a) => [a.address, a.role])).toEqual([
      [owner.address, AccountRole.READONLY_SIGNER],
      [session, AccountRole.READONLY],
      [character, AccountRole.WRITABLE],
      [await sessionAuthorityPda(), AccountRole.READONLY],
      [await gameConfigPda(), AccountRole.READONLY],
      [PROGRAMS.character, AccountRole.READONLY],
    ]);
  });

  it("patch 11: open_chest: the chest index, the VRF accounts after the session token, the map layout last", async () => {
    const signer = await generateKeyPairSigner();
    const ix = await openChestIx({ signer, token: null }, session, character, addrOf(13), 2);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(concat(ixDisc("open_chest"), Uint8Array.of(2))));
    const addrs = ix.accounts!.map((a) => a.address);
    expect(addrs.slice(0, 6)).toEqual([signer.address, session, await participationPda(session, character), character, await skillSetPda(character), await bagPda(character)]);
    expect(addrs.at(-1)).toBe(addrOf(13));
    expect(addrs).toContain(VRF.oracleQueue);
  });

  it("patch 11: settle_party: the DM signs; the pending deaths, then every player's participation, cache and character", async () => {
    const dm = await generateKeyPairSigner();
    const s = zeroOf(getGameSessionDecoder());
    s.core.playerCount = 2;
    s.core.players = [addrOf(21), addrOf(22), addrOf(0), addrOf(0)];
    s.core.map = addrOf(30);
    s.pendingDeaths = 0b10;
    const ix = await settlePartyIx(dm, session, s);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(ixDisc("settle_party")));
    expect([ix.accounts![0]!.address, ix.accounts![0]!.role]).toEqual([dm.address, AccountRole.WRITABLE_SIGNER]);
    const tail = ix.accounts!.slice(-9).map((a) => [a.address, a.role]);
    const w = AccountRole.WRITABLE;
    expect(tail).toEqual([
      [addrOf(22), w],
      [await skillSetPda(addrOf(22)), w],
      [await bagPda(addrOf(22)), w],
      [await participationPda(session, addrOf(21)), w],
      [await rewardCachePda(addrOf(21)), w],
      [addrOf(21), w],
      [await participationPda(session, addrOf(22)), w],
      [await rewardCachePda(addrOf(22)), w],
      [addrOf(22), w],
    ]);
  });

  it("patch 11: claim_after_timeout: the player signs; its own accounts first, then the pending deaths", async () => {
    const player = await generateKeyPairSigner();
    const s = zeroOf(getGameSessionDecoder());
    s.core.playerCount = 1;
    s.core.players = [addrOf(21), addrOf(0), addrOf(0), addrOf(0)];
    const ix = await claimAfterTimeoutIx(player, session, s, addrOf(21));
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(ixDisc("claim_after_timeout")));
    expect(ix.accounts!.slice(-3).map((a) => a.address)).toEqual([await participationPda(session, addrOf(21)), await rewardCachePda(addrOf(21)), addrOf(21)]);
  });

  it("patch 11: close_cache: the party, then the layout as 40 + 5 BagSlots (kind u8, id u16 LE, qty u8)", async () => {
    const owner = await generateKeyPairSigner();
    const empty = { kind: 0, id: 0, qty: 0 };
    const layout = { resources: Array.from({ length: 40 }, () => empty), consumables: Array.from({ length: 5 }, () => empty) };
    layout.resources[0] = { kind: 1, id: 0x0203, qty: 4 };
    layout.consumables[4] = { kind: 2, id: 7, qty: 5 };
    const ix = await closeCacheIx(owner, character, addrOf(30), layout);
    const body = new Uint8Array(45 * 4);
    body.set([1, 0x03, 0x02, 4], 0);
    body.set([2, 7, 0, 5], 44 * 4);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(concat(ixDisc("close_cache"), A(30), body)));
    expect(ix.accounts!.map((a) => a.address).slice(0, 4)).toEqual([owner.address, await rewardCachePda(character), character, await bagPda(character)]);
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
  it("patch 13: create_map takes min_players, write_points sends the entrance, the exit, the chests and the boss table, finalize_map reads the rewards and the tileset", async () => {
    const dm = await generateKeyPairSigner();
    const tiles = new Uint8Array(1024).fill(cellByte(Tile.StoneFloor));
    tiles[1] = cellByte(Tile.Rubble, 6);
    const chests = [
      { x: 1, y: 2, table: 0 },
      { x: 3, y: 1, table: 5 },
    ];
    // Arena update: pack 2, a 2x2 arena; the other numbers unused.
    const packs = new Array(8).fill(null);
    packs[2] = { visible: 0, x: 3, y: 2, facing: 5, width: 2, height: 2, cells: Uint8Array.of(8, 9, 10, 11) };
    const draft = { width: 4, height: 3, tier: 1, minPlayers: 3, chestRadius: 4, settleTimeoutSecs: 7200, biome: Biome.Crypt, tiles, entrance: { x: 0, y: 0 }, exit: { x: 3, y: 0 }, spawns: [], chests, bossTable: 1, packs };
    const b = await buildMapIxs(dm, draft);
    expect(b.ixs.length).toBe(6);
    const [create, layout, spawns, writePoints, arena, finalize] = b.ixs as [Instruction, Instruction, Instruction, Instruction, Instruction, Instruction];
    // write_arena: pack u8, PackLayout (visible, x, y, facing, width, height), Vec<u8> of the rows.
    expect(toHex(arena.data as Uint8Array)).toBe(toHex(concat(ixDisc("write_arena"), Uint8Array.of(2, 0, 3, 2, 5, 2, 2, 4, 0, 0, 0, 8, 9, 10, 11))));
    const index = new Uint8Array(4);
    new DataView(index.buffer).setUint32(0, b.index, true);
    // MapSettings: min_players, chest_radius, settle_timeout_secs (u32 LE, 7200 = 0x1c20), biome (Crypt = 2).
    expect(toHex(create.data as Uint8Array)).toBe(toHex(concat(ixDisc("create_map"), index, Uint8Array.of(4, 3, 1, 3, 4, 0x20, 0x1c, 0, 0, 2))));
    // write_layout: offset u16, Vec<u8> of cell bytes up to the last cell of the map (2 rows of 32 + 4).
    const cells = (layout.data as Uint8Array).slice(8 + 2 + 4);
    expect(cells.length).toBe(2 * 32 + 4);
    expect(cells[1]).toBe((Tile.Rubble << 3) | 6);
    // write_spawns: only the spawns now (an empty Vec).
    expect(toHex(spawns.data as Uint8Array)).toBe(toHex(concat(ixDisc("write_spawns"), Uint8Array.of(0, 0, 0, 0))));
    // Vec<MapPoint>: u32 LE count, then (kind, x, y, data): entrance (1), exit (2), the chests (0) with
    // their tables; boss table u8.
    const points = Uint8Array.of(4, 0, 0, 0, 1, 0, 0, 0, 2, 3, 0, 0, 0, 1, 2, 0, 0, 3, 1, 5);
    expect(toHex(writePoints.data as Uint8Array)).toBe(toHex(concat(ixDisc("write_points"), points, Uint8Array.of(1))));
    expect(writePoints.accounts!.map((a) => [a.address, a.role])).toEqual([
      [dm.address, AccountRole.READONLY_SIGNER],
      [b.map, AccountRole.READONLY],
      [b.layout, AccountRole.WRITABLE],
    ]);
    expect(finalize.accounts!.map((a) => a.address).slice(3)).toEqual([await bestiaryPda(), await monsterSkillsPda(), await rewardsPda(), await tilesetPda()]);
    await expect(buildMapIxs(dm, { ...draft, exit: null })).rejects.toThrow(/entrance and an exit/);
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
      s.unpaidKills = 70;
    });
    expect(toHex(bytes.subarray(0, 8))).toBe(sha("account:GameSession"));
    expect(back.core.status).toBe(SessionStatus.Active);
    expect(back.core.priceLamports).toBe(100_000_000n);
    expect(back.core.turnOrder[0]).toBe(6);
    expect([back.aliveMask, back.unpaidKills]).toEqual([3, 70]);
    expect(() => decodeAccount("Participation", bytes)).toThrow(/not a Participation account/);
  });

  it("reads Map.biome, and a map from before it (one byte shorter) as Caves", () => {
    const { bytes, back } = roundTrip("Map", (v) => {
      v.core.version = 2;
      v.core.width = 16;
      v.tier = 2;
      v.settleTimeoutSecs = 3600;
      v.biome = Biome.Forest;
    });
    expect(bytes.length).toBe(8 + 105 + 1 + 1 + 1 + 4 + 1);
    expect(toHex(bytes.subarray(0, 8))).toBe(sha("account:Map"));
    expect(bytes[bytes.length - 1]).toBe(Biome.Forest);
    expect([back.biome, back.tier, back.settleTimeoutSecs]).toEqual([Biome.Forest, 2, 3600]);
    const old = decodeAccount("Map", bytes.slice(0, -1));
    expect([old.biome, old.core.width, old.tier, old.settleTimeoutSecs]).toEqual([Biome.Caves, 16, 2, 3600]);
    expect(() => decodeAccount("Map", bytes.slice(0, -2))).toThrow();
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
      v.endedAt = 0x0102n;
      v.chestRadius = 4;
      v.settleTimeoutSecs = 600;
      v.tileCost = Uint8Array.from({ length: 32 }, (_, i) => (i === 10 ? 7 : 0));
      v.mode = 2;
      v.arenaPack = 3;
      v.packs[3] = { visible: 4, x: 12, y: 13, facing: 5, width: 8, height: 9 };
      v.packsAlive = 0b1110;
      v.unitTime[11] = 0x0201;
      v.playerDex[3] = 0x0403;
    });
    // Arena update appends mode, arena_pack, engaged_by, packs (8 x 6), packs_alive, map_width,
    // map_height, map_pos (4 x 2), map_order (4), map_order_len, map_turn_index, map_round (u16),
    // unit_time (12 x u32), player_dex (4 x u16): 126 bytes.
    const arena = 3 + 48 + 1 + 2 + 8 + 4 + 2 + 2 + 48 + 8;
    expect(arena).toBe(126);
    expect(Array.from(s.bytes.subarray(-arena, -arena + 2))).toEqual([2, 3]);
    expect(Array.from(s.bytes.subarray(-arena + 3 + 18, -arena + 3 + 24))).toEqual([4, 12, 13, 5, 8, 9]);
    expect(s.bytes[s.bytes.length - arena + 51]).toBe(0b1110);
    expect(Array.from(s.bytes.subarray(-12, -8))).toEqual([1, 2, 0, 0]);
    expect(Array.from(s.bytes.subarray(-2))).toEqual([3, 4]);
    expect([s.back.mode, s.back.arenaPack, s.back.packs[3].y, s.back.unitTime[11], s.back.playerDex[3]]).toEqual([2, 3, 13, 0x0201, 0x0403]);
    s.bytes = s.bytes.subarray(0, -arena);
    // Patch 13 appends tile_cost by tile type (32 bytes) and drops the patch 1 `[u8; 7]`.
    expect(s.bytes[s.bytes.length - 32 + 10]).toBe(7);
    expect(s.back.tileCost[10]).toBe(7);
    s.bytes = s.bytes.subarray(0, -32);
    // Patch 11 appends tier, min_players, chests (4 x 4), chest_count, boss_table, chests_opened,
    // pending_chest, xp_bp (4 x u16), loot_mask, unpaid_kills (u32), ended_at (i64), chest_radius
    // and settle_timeout_secs (u32): 48 bytes.
    const p11 = 48;
    expect(Array.from(s.bytes.subarray(-5))).toEqual([4, 0x58, 0x02, 0, 0]);
    expect(Array.from(s.bytes.subarray(-13, -11))).toEqual([2, 1]);
    expect(s.back.endedAt).toBe(0x0102n);
    s.bytes = s.bytes.subarray(0, -p11);
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
      v.cacheXp = 0x0304;
      v.cache = Uint8Array.from({ length: 160 }, (_, i) => (i < 5 ? [1, 2, 0, 3, 0][i]! : 0));
    });
    // Patch 11 appends cache_xp (u32) and the raw cache (32 x 5 bytes).
    expect(Array.from(p.bytes.subarray(-164, -160))).toEqual([4, 3, 0, 0]);
    expect(Array.from(p.bytes.subarray(-160, -155))).toEqual([1, 2, 0, 3, 0]);
    expect(p.back.cacheXp).toBe(0x0304);
    p.bytes = p.bytes.subarray(0, -164);
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
    // Arena update (version 2): the live slots are the arena's pack (8), the copies per template.
    const m = roundTrip("MonsterState", (v) => {
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
      v.lootIds = Uint8Array.from({ length: 12 }, (_, i) => (i === 0 ? 4 : 0xff));
      v.lootTables[0] = Uint8Array.from({ length: 108 }, (_, i) => (i === 0 ? 1 : 0));
      v.resTriples = Uint8Array.from({ length: 1920 }, (_, i) => (i === 0 ? 7 : i === 1919 ? 9 : 0));
      v.spawnOf = Uint8Array.of(6, 5, 4, 0, 0, 0, 0, 0);
      v.templateIds = Uint8Array.from({ length: 12 }, (_, i) => (i === 0 ? 2 : 0xff));
      v.templates[0] = Uint8Array.from({ length: 48 }, (_, i) => 200 - i);
      v.skillIds = Uint16Array.from({ length: 32 }, (_, i) => (i === 0 ? 0x0102 : 0));
    });
    expect(m.bytes.length).toBe(8_100);
    // The arena update appends spawn_of (8), template_ids (12), templates (12 x 48), skill_ids
    // (32 x u16), last in the account.
    const arenaM = 8 + 12 + 12 * 48 + 32 * 2;
    expect(Array.from(m.bytes.subarray(-arenaM, -arenaM + 3))).toEqual([6, 5, 4]);
    expect(m.bytes[m.bytes.length - arenaM + 8]).toBe(2);
    expect(m.bytes[m.bytes.length - arenaM + 20]).toBe(200);
    expect(Array.from(m.bytes.subarray(-64, -62))).toEqual([2, 1]);
    expect([m.back.spawnOf[1], m.back.templateIds[0], m.back.skillIds[0]]).toEqual([5, 2, 0x0102]);
    m.bytes = m.bytes.subarray(0, -arenaM);
    // The triples pass: res_triples (120 x 16).
    expect(m.bytes[m.bytes.length - 1920]).toBe(7);
    expect(m.bytes[m.bytes.length - 1]).toBe(9);
    m.bytes = m.bytes.subarray(0, -1920);
    // Patch 11: loot_ids (12) and loot_tables (12 x 108).
    const p11m = 12 + 12 * 108;
    expect(Array.from(m.bytes.subarray(-p11m, -p11m + 2))).toEqual([4, 0xff]);
    expect(m.bytes[m.bytes.length - 12 * 108]).toBe(1);
    m.bytes = m.bytes.subarray(0, -p11m);
    // Patch 9: resist_mods (8 x 14) and aura_weights (8 x 2 x 6).
    const p9m = 8 * 14 + 8 * 2 * 6;
    expect(m.bytes[m.bytes.length - p9m + 3 * 14]).toBe(ModKind.Resistance);
    expect(m.bytes[m.bytes.length - 8 * 2 * 6 + 12]).toBe(100);
    m.bytes = m.bytes.subarray(0, -p9m);
    // Patch 6: records (8 x 48), skill_table (32 x 16), skill_count, skills and skill_cooldowns
    // (8 x 7 each), auras (8 x 2).
    const p6m = 8 * 48 + 32 * 16 + 1 + 2 * 8 * 7 + 16 * getMonsterAuraDataEncoder().fixedSize;
    expect(Array.from(m.bytes.subarray(-p6m + 48, -p6m + 52))).toEqual([0, 1, 2, 3]);
    expect(m.bytes[m.bytes.length - p6m + 8 * 48]).toBe(100);
    expect(m.bytes[m.bytes.length - p6m + 8 * 48 + 32 * 16]).toBe(1);
    expect([m.back.skillCount, Array.from(m.back.skills[1]), Array.from(m.back.skillCooldowns[1]).slice(0, 1)]).toEqual([1, [0, 0xff, 0xff, 0xff, 0xff, 0xff, 0], [2]]);
    expect([m.back.auras[1][0].aura.turns, m.back.auras[1][0].kinOnly]).toEqual([255, true]);
    m.bytes = m.bytes.subarray(0, -p6m);
    // After the core: the mods (8 x 6 x 8), defense (8 x 3), the tables (48 + 192 + 576), burning
    // cells (16 x 9).
    const tail = 8 * 3 + 48 + 192 + 576 + 16 * 9;
    expect(m.back.mods).toHaveLength(8);
    expect(Array.from(m.bytes.subarray(-tail - 384 + 2 * 48 + 8, -tail - 384 + 2 * 48 + 11))).toEqual([ModKind.Slow, Aspect.Cold, 2]);
    expect(Array.from(m.bytes.subarray(-tail + 3, -tail + 6))).toEqual([2, 0, 4]);
    expect(Array.from(m.bytes.subarray(-16 * 9, -16 * 9 + 9))).toEqual([3, 4, 7, 0, 5, 0, 0, 0, 1]);
    expect(m.back.burningCells[0]).toEqual({ x: 3, y: 4, untilRound: 7, power: 5, source: 1 });
    expect(m.back.mods[2][ModKind.Slow].turns).toBe(2);
    // A pre-upgrade account (no trailing fields) no longer decodes.
    expect(() => decodeAccount("Participation", p.bytes)).toThrow();
  });

  it("patch 11: decodes the bag in slots and the reward cache", () => {
    const bagZero = zeroOf(getBagDecoder());
    bagZero.layout.resources[2] = { kind: 1, id: 0x0102, qty: 9 };
    bagZero.layout.consumables[0] = { kind: 2, id: 3, qty: 5 };
    const bagBytes = Uint8Array.from(getBagEncoder().encode(bagZero));
    expect(toHex(bagBytes.subarray(0, 8))).toBe(sha("account:Bag"));
    // version, character, items (20 x 32), item_count, bump, then the layout: 40 + 5 slots of 4 bytes.
    const at = 8 + 1 + 32 + 20 * 32 + 2;
    expect(Array.from(bagBytes.subarray(at + 2 * 4, at + 3 * 4))).toEqual([1, 0x02, 0x01, 9]);
    const bag = decodeAccount("Bag", bagBytes);
    expect(bag.layout.consumables[0]).toEqual({ kind: 2, id: 3, qty: 5 });
    const cacheZero = zeroOf(getRewardCacheDecoder());
    // Entry 1 of 3: session, status (Claimed 2), xp u32, then 32 slots of 5 bytes (kind, id u16, qty u16).
    cacheZero.entries[1] = { ...cacheZero.entries[1], session: addrOf(31), status: 2, xp: 250 };
    cacheZero.entries[1].slots[0] = { kind: 0, id: 4, qty: 12 };
    const cacheBytes = Uint8Array.from(getRewardCacheEncoder().encode(cacheZero));
    expect(toHex(cacheBytes.subarray(0, 8))).toBe(sha("account:RewardCache"));
    const entry = 32 + 1 + 4 + 32 * 5;
    const entryAt = 8 + 1 + 32 + entry;
    expect(Array.from(cacheBytes.subarray(entryAt, entryAt + 32))).toEqual(Array.from(A(31)));
    expect(Array.from(cacheBytes.subarray(entryAt + 32, entryAt + 32 + 1 + 4 + 5))).toEqual([2, 250, 0, 0, 0, 0, 4, 0, 12, 0]);
    const cache = decodeAccount("RewardCache", cacheBytes);
    expect([cache.entries[1]!.status, cache.entries[1]!.xp, cache.entries[1]!.slots[0]]).toEqual([2, 250, { kind: 0, id: 4, qty: 12 }]);
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
    expect(h.result.jumps).toEqual([{ mode: 2, state: { kind: 4, turns: 2, power: 2000, source: 1 }, filter: 4, limit: 1, base: Aspect.Lightning, filterAny: false }]);
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
  // 5x3 room, a wall in the middle column except the bottom row, water costs 2 (the shipped tileset).
  const tiles = new Uint8Array(MAP_MAX * MAP_MAX).fill(cellByte(Tile.Wall));
  for (let y = 0; y < 3; y++) for (let x = 0; x < 5; x++) tiles[y * MAP_MAX + x] = cellByte(Tile.StoneFloor, x);
  tiles[0 * MAP_MAX + 2] = cellByte(Tile.Wall);
  tiles[1 * MAP_MAX + 2] = cellByte(Tile.Wall);
  tiles[2 * MAP_MAX + 3] = cellByte(Tile.ShallowWater, 7);
  const tileCost = SHIPPED_TILE_COST;
  const base = { tiles, width: 5, height: 3, tileCost, taken: new Set<number>() };

  it("finds the cheapest path over the 8 neighbors, around walls and water, without cutting corners", () => {
    const steps = route({ ...base, from: { x: 0, y: 0 }, goal: (x, y) => x === 4 && y === 0 })!;
    // (1,1) -> (2,2) and (2,2) -> (3,1) would cut the wall (2,1): the way goes (1,2), (2,2), then
    // the water (3,2) for 2, then up: 3 + 2 + 2 = 7.
    expect(steps.at(-1)!.to).toEqual({ x: 4, y: 0 });
    expect(steps.reduce((n, s) => n + s.cost, 0)).toBe(7);
    expect(steps.some((s) => s.to.x === 2 && s.to.y === 2)).toBe(true);
    expect(steps.find((s) => s.to.x === 3 && s.to.y === 2)?.cost).toBe(2);
    for (let i = 1; i < steps.length; i++) {
      const [a, b] = [steps[i - 1]!.to, steps[i]!.to];
      // No step passes between two cells where one is the wall column.
      if (a.x !== b.x && a.y !== b.y) expect([a.x, b.x].includes(2) && [a.y, b.y].includes(1)).toBe(false);
    }
    // With (3,1) taken the only way is the water, which costs 2.
    const around = route({ ...base, taken: new Set([1 * MAP_MAX + 3]), from: { x: 0, y: 0 }, goal: (x, y) => x === 4 && y === 0 })!;
    expect(around.find((s) => s.to.x === 3 && s.to.y === 2)?.cost).toBe(2);
  });

  it("refuses a diagonal past a wall corner, allows an open one", () => {
    // (1,1) -> (2,2) passes (2,1), a wall: refused; (1,2) -> (2,2) is straight.
    expect(extendPath(base, { x: 1, y: 1 }, [], { x: 2, y: 2 }, 9)!.map((s) => s.to)).toEqual([{ x: 1, y: 2 }, { x: 2, y: 2 }]);
    // Between two walls touching at the corner: no way at all.
    const corner = new Uint8Array(MAP_MAX * MAP_MAX).fill(cellByte(Tile.Wall));
    corner[0] = cellByte(Tile.StoneFloor);
    corner[MAP_MAX + 1] = cellByte(Tile.StoneFloor);
    const g = { tiles: corner, width: 2, height: 2, tileCost, taken: new Set<number>() };
    expect(extendPath(g, { x: 0, y: 0 }, [], { x: 1, y: 1 }, 9)).toBeNull();
    expect(reachable({ ...g, from: { x: 0, y: 0 }, ap: 9 })).toEqual([]);
    // Open on both sides (a unit on one does not matter): one step.
    const open = new Uint8Array(MAP_MAX * MAP_MAX).fill(cellByte(Tile.StoneFloor));
    const g2 = { tiles: open, width: 2, height: 2, tileCost, taken: new Set<number>([1]) };
    expect(extendPath(g2, { x: 0, y: 0 }, [], { x: 1, y: 1 }, 9)).toEqual([{ cost: 1, to: { x: 1, y: 1 } }]);
  });

  it("cuts a path to the action points and to 16 cells", () => {
    const steps = route({ ...base, from: { x: 0, y: 0 }, goal: (x, y) => x === 4 && y === 0 })!;
    const take = affordable(steps, 5, 2);
    expect(take.reduce((n, s) => n + s.cost, 0)).toBeLessThanOrEqual(3);
    const long = Array.from({ length: 20 }, () => ({ cost: 1, to: { x: 0, y: 0 } }));
    expect(affordable(long, 99)).toHaveLength(16);
  });

  it("a path marked cell by cell", () => {
    const from = { x: 0, y: 0 };
    let path = extendPath(base, from, [], { x: 1, y: 1 }, 5)!;
    expect(path.map((s) => s.to)).toEqual([{ x: 1, y: 1 }]);
    // A far cell adds the cheapest way to it.
    path = extendPath(base, from, path, { x: 2, y: 2 }, 5)!;
    expect(path.map((s) => s.to)).toEqual([{ x: 1, y: 1 }, { x: 1, y: 2 }, { x: 2, y: 2 }]);
    // A neighbor past a corner is reached around it: (2,2) -> (3,2) water -> (3,1).
    path = extendPath(base, from, path, { x: 3, y: 1 }, 9)!;
    expect(path.map((s) => s.to)).toEqual([{ x: 1, y: 1 }, { x: 1, y: 2 }, { x: 2, y: 2 }, { x: 3, y: 2 }, { x: 3, y: 1 }]);
    // Not past the action points, not into a wall, not onto the end again.
    expect(extendPath(base, from, path, { x: 4, y: 0 }, path.reduce((n, s) => n + s.cost, 0))).toBeNull();
    expect(extendPath(base, from, path, { x: 2, y: 1 }, 5)).toBeNull();
    expect(extendPath(base, from, path, { x: 3, y: 1 }, 5)).toBeNull();
    // A cell on the path cuts it back; the start empties it.
    expect(extendPath(base, from, path, { x: 1, y: 1 }, 5)!.length).toBe(1);
    expect(extendPath(base, from, path, from, 5)).toEqual([]);
    // Water costs 2.
    expect(extendPath(base, { x: 4, y: 2 }, [], { x: 3, y: 2 }, 5)![0]!.cost).toBe(2);
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
