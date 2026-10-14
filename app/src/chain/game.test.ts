// @vitest-environment node
// Byte-level tests for the IDL coder and the demo builders (game.ts), checked against Anchor's
// discriminator rule and hand-built encodings in the order of the programs' Accounts structs.
import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { AccountRole, generateKeyPairSigner, getAddressDecoder, getAddressEncoder, type Address, type Instruction } from "@solana/kit";
import { PROGRAMS, VRF } from "../config";
import { concat, toHex } from "./bytes";
import { camel, type IdlType } from "./coder";
import { decodeEvents } from "./events";
import {
  DEFAULT_KEY,
  SKILL_PRESETS,
  actIx,
  consumeIx,
  pendingTargetOf,
  endTurnIx,
  joinSessionIx,
  setReadyIx,
  mockVrfSigner,
  moveIx,
  startTurnIx,
  textBytes,
  type Actor,
} from "./game";
import { toHitResolved } from "./hit";
import { toEffectResolved } from "./support";
import { Effect } from "./skillcheck";
import { coders } from "./programs";
import { bagPda, gameConfigPda, monstersPda, participationPda, programIdentityPda, sessionAuthorityPda, sessionTokenPda, skillSetPda } from "./pda";
import { affordable, pathArgs, reachable, route } from "./route";
import { explainFailure } from "./send";
import { Applied, Aspect, Class, HitOutcome, MAP_MAX, ModKind, Tile } from "./types";

const sha = (s: string) => createHash("sha256").update(s).digest("hex").slice(0, 16);
const A = (n: number) => new Uint8Array(32).fill(n);
const addrOf = (n: number): Address => getAddressDecoder().decode(A(n));
const enc = getAddressEncoder();
const pk = (a: Address) => enc.encode(a) as Uint8Array;
const ixDisc = (name: string) => Buffer.from(sha(`global:${name}`), "hex");

/** A zero value of any IDL type (first enum variant, None for options). */
function zero(coder: (typeof coders)["session"], t: IdlType): unknown {
  if (typeof t === "string") return t === "pubkey" ? addrOf(0) : t === "bool" ? false : t === "u64" || t === "i64" ? 0n : t === "bytes" ? new Uint8Array() : 0;
  if ("array" in t) return t.array[0] === "u8" ? new Uint8Array(t.array[1]) : Array.from({ length: t.array[1] }, () => zero(coder, t.array[0]));
  if ("option" in t) return null;
  if ("vec" in t) return [];
  const def = coder.idl.types!.find((x) => x.name === t.defined.name)!;
  if (def.type.kind === "enum") return 0;
  return Object.fromEntries((def.type.fields ?? []).map((f) => [camel(f.name), zero(coder, f.type)]));
}

describe("IDL snapshot", () => {
  it("is for the program ids in config and CONTRACTS.md", () => {
    expect(coders.session.idl.address).toBe(PROGRAMS.session);
    expect(coders.character.idl.address).toBe(PROGRAMS.character);
    expect(coders.map.idl.address).toBe(PROGRAMS.map);
  });

  it("discriminators follow Anchor's sha256 rule", () => {
    for (const c of Object.values(coders)) {
      for (const ix of c.idl.instructions) expect(Buffer.from(ix.discriminator).toString("hex"), ix.name).toBe(sha(`global:${ix.name}`));
      for (const a of c.idl.accounts ?? []) expect(Buffer.from(a.discriminator).toString("hex"), a.name).toBe(sha(`account:${a.name}`));
      for (const e of c.idl.events ?? []) expect(Buffer.from(e.discriminator).toString("hex"), e.name).toBe(sha(`event:${e.name}`));
    }
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
      [signer.address, AccountRole.READONLY_SIGNER],
      [session, AccountRole.WRITABLE],
      [await participationPda(session, character), AccountRole.WRITABLE],
      [await monstersPda(session), AccountRole.READONLY],
      [addrOf(10), AccountRole.READONLY],
      [addrOf(9), AccountRole.READONLY],
    ]);
  });

  it("act: skill slot and target, VRF accounts after the session token", async () => {
    const signer = await generateKeyPairSigner();
    const ix = await actIx({ signer, token: null }, session, character, 1, 3, null);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(concat(ixDisc("act"), Uint8Array.of(1, 3))));
    const addrs = ix.accounts!.map((a) => a.address);
    expect(addrs[4]).toBe(await skillSetPda(character));
    // The bag rides along to the callback, which burns it if the character dies.
    expect(addrs[5]).toBe(await bagPda(character));
    // No session token: Anchor's None is the program id.
    expect(addrs[7]).toBe(PROGRAMS.session);
    expect(addrs.slice(8, 13)).toEqual([await programIdentityPda(), VRF.oracleQueue, VRF.program, VRF.slotHashes, "11111111111111111111111111111111"]);
    expect(ix.accounts![9]!.role).toBe(AccountRole.WRITABLE);
    // An enemy target (or a self-cast): no ally account, the trailing optional is None.
    expect(ix.accounts).toHaveLength(14);
    expect(ix.accounts![13]).toEqual({ address: PROGRAMS.session, role: AccountRole.READONLY });
  });

  it("act: a buff on another ally targets its player slot and passes its participation last", async () => {
    const signer = await generateKeyPairSigner();
    const ally = addrOf(14);
    const ix = await actIx({ signer, token: null }, session, character, 2, 1, ally);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(concat(ixDisc("act"), Uint8Array.of(2, 1))));
    expect(ix.accounts![13]).toEqual({ address: ally, role: AccountRole.READONLY });
  });

  it("end_turn for a monster passes None for the participation", async () => {
    const signer = await generateKeyPairSigner();
    const ix = await endTurnIx({ signer, token: addrOf(9) }, session, null);
    expect(ix.accounts![3]).toEqual({ address: PROGRAMS.session, role: AccountRole.READONLY });
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(ixDisc("end_turn")));
  });

  it("start_turn: no args, no signer, the participation and character of a player turn, its SkillSet and Bag last", async () => {
    const participation = addrOf(13);
    const ix = await startTurnIx(session, { participation, character });
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
    ]);
  });

  it("start_turn for a monster passes None for the participation, the character, SkillSet and Bag", async () => {
    const ix = await startTurnIx(session, null);
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

  it("set_ready: the flag and the 6-slot selection; the character (w) and its SkillSet for the lock CPI", async () => {
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
    ]);
    const off = await setReadyIx(player, session, character, false, [0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    expect(toHex(off.data as Uint8Array)).toBe(toHex(concat(ixDisc("set_ready"), Uint8Array.of(0, 255, 255, 255, 255, 255, 255))));
  });

  it("consume_randomness is signed by the mock-vrf identity of programs/session/src/vrf.rs", async () => {
    expect((await mockVrfSigner()).address).toBe("5RotSMue7vx9xBPp3L8iQEKbKSc3ezbY4GqiJxDEe6sN");
    const ix = await consumeIx(session, addrOf(12), character, A(0x5a), null);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(concat(ixDisc("consume_randomness"), A(0x5a))));
    expect(ix.accounts![0]!.role).toBe(AccountRole.READONLY_SIGNER);
    expect(ix.accounts![3]!.address).toBe(addrOf(12));
    // skill_set and bag of that character: burned by mark_dead if the roll kills it.
    expect(ix.accounts!.slice(5, 7).map((a) => a.role)).toEqual([AccountRole.WRITABLE, AccountRole.WRITABLE]);
    expect(ix.accounts).toHaveLength(11);
    expect(ix.accounts![10]).toEqual({ address: PROGRAMS.session, role: AccountRole.READONLY });
  });

  it("consume_randomness passes GameSession.pendingTarget, writable, for a heal or buff on an ally", async () => {
    const ally = addrOf(15);
    expect(pendingTargetOf({ pendingTarget: DEFAULT_KEY } as never)).toBeNull();
    expect(pendingTargetOf({ pendingTarget: ally } as never)).toBe(ally);
    const ix = await consumeIx(session, addrOf(12), character, A(1), ally);
    expect(ix.accounts![10]).toEqual({ address: ally, role: AccountRole.WRITABLE });
  });
});

describe("create_character", () => {
  it("encodes CreateCharacterArgs as the program reads it", () => {
    const data = coders.character.instructionData("create_character", {
      args: {
        name: textBytes("Ilyra", 32),
        race: "forestElf",
        class: "mage",
        predisposition: "cold",
        stats: [5, 7, 13, 5],
        appearance: new Array(20).fill({ spriteId: 0x0102, materialRamp: 3, accentRamp: 4, elementRamp: 5 }),
        skills: SKILL_PRESETS[Class.Mage].map((s) => ({ blocks: s.blocks, name: textBytes(s.name, 24) })),
      },
    });
    // 8 disc + 32 name + race, class, predisposition + 4 x u16 + 20 x LayerRef(5) + vec(6 x (12 blocks + 24 name))
    expect(data.length).toBe(8 + 32 + 3 + 8 + 100 + 4 + 6 * 36);
    expect(Array.from(data.subarray(40, 43))).toEqual([1, 1, Aspect.Cold]);
    expect(Array.from(data.subarray(43, 51))).toEqual([5, 0, 7, 0, 13, 0, 5, 0]);
    expect(Array.from(data.subarray(51, 56))).toEqual([2, 1, 3, 4, 5]);
    const vec = 151;
    expect(Array.from(data.subarray(vec, vec + 4))).toEqual([6, 0, 0, 0]);
    // Frost Burn (second skill): aspects [cold, fire, fire], count 2.
    const frostBurn = vec + 4 + 36;
    expect(Array.from(data.subarray(frostBurn + 5, frostBurn + 9))).toEqual([Aspect.Cold, Aspect.Fire, Aspect.Fire, 2]);
  });
});

describe("decoders", () => {
  it("GameSession round-trips through the coder", () => {
    const c = coders.session;
    const s = zero(c, { defined: { name: "GameSession" } }) as Record<string, any>;
    s.core.status = "active";
    s.core.priceLamports = 100_000_000n;
    s.core.turnOrder[0] = 6;
    s.aliveMask = 3;
    s.xpPool = 70;
    const out: Uint8Array[] = [c.accountDiscriminator("GameSession")];
    c.encode({ defined: { name: "GameSession" } }, s, out);
    const back = c.decodeAccount<any>("GameSession", concat(...out));
    expect(back.core.status).toEqual({ active: {} });
    expect(back.core.priceLamports).toBe(100_000_000n);
    expect(back.core.turnOrder[0]).toBe(6);
    expect([back.aliveMask, back.xpPool]).toEqual([3, 70]);
  });

  it("reads the turn-rule fields appended to GameSession, Participation and MonsterState", () => {
    const c = coders.session;
    const roundTrip = (name: string, edit: (v: Record<string, any>) => void) => {
      const v = zero(c, { defined: { name } }) as Record<string, any>;
      edit(v);
      const out: Uint8Array[] = [c.accountDiscriminator(name)];
      c.encode({ defined: { name } }, v, out);
      const bytes = concat(...out);
      return { bytes, back: c.decodeAccount<any>(name, bytes) };
    };
    const ally = addrOf(16);
    const s = roundTrip("GameSession", (v) => {
      v.turnSeq = 9;
      v.actedSeq = 9;
      v.startedSeq = 8;
      v.pendingTarget = ally;
    });
    // acted_seq and started_seq, then pending_target (32) are the last 40 bytes.
    expect(Array.from(s.bytes.subarray(-40, -32))).toEqual([9, 0, 0, 0, 8, 0, 0, 0]);
    expect(Array.from(s.bytes.subarray(-32))).toEqual(Array.from(pk(ally)));
    expect([s.back.turnSeq, s.back.actedSeq, s.back.startedSeq, s.back.pendingTarget]).toEqual([9, 9, 8, ally]);
    const mod = { kind: "weakening", aspect: "blood", turns: 3, powerBp: 1_800, source: 2 };
    const p = roundTrip("Participation", (v) => {
      v.cooldowns[1] = 2;
      v.hpMax = 30;
      v.mods[ModKind.Weakening] = mod;
      v.equipment = Uint8Array.of(0, 0xff, 0xff, 3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff);
      v.skills = Uint8Array.of(2, 0, 0xff, 5, 1, 3);
    });
    expect(p.back.cooldowns).toHaveLength(16);
    // cooldowns (16), hp_max (4), mods (6 x 8 bytes), equipment (11), skills (6).
    expect(Array.from(p.bytes.subarray(-85, -83))).toEqual([0, 2]);
    expect(Array.from(p.bytes.subarray(-69, -65))).toEqual([30, 0, 0, 0]);
    // TimedModData: kind, aspect, turns, power_bp u32, source = 8 bytes.
    expect(Array.from(p.bytes.subarray(-65 + 8 * 3, -65 + 8 * 4))).toEqual([ModKind.Weakening, Aspect.Blood, 3, 0x08, 0x07, 0, 0, 2]);
    expect(Array.from(p.bytes.subarray(-17, -6))).toEqual([0, 0xff, 0xff, 3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    expect(Array.from(p.bytes.subarray(-6))).toEqual([2, 0, 0xff, 5, 1, 3]);
    expect(Array.from(p.back.skills)).toEqual([2, 0, 0xff, 5, 1, 3]);
    expect(p.back.hpMax).toBe(30);
    expect(Array.from(p.back.equipment)).toEqual([0, 0xff, 0xff, 3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    expect(p.back.mods[ModKind.Weakening]).toEqual({ ...mod, kind: { weakening: {} }, aspect: { blood: {} } });
    const m = roundTrip("MonsterState", (v) => {
      v.cooldowns[2] = Uint8Array.of(1, 0, 3, 0);
      v.mods[2][ModKind.Slow] = { kind: "slow", aspect: "cold", turns: 2, powerBp: 2_500, source: 1 };
    });
    expect(m.back.cooldowns).toHaveLength(24);
    expect(Array.from(m.back.cooldowns[2])).toEqual([1, 0, 3, 0]);
    // mods: 24 monsters x 6 slots x 8 bytes after the cooldowns.
    expect(m.back.mods).toHaveLength(24);
    expect(Array.from(m.bytes.subarray(-1152 + 2 * 48 + 8, -1152 + 2 * 48 + 11))).toEqual([ModKind.Slow, Aspect.Cold, 2]);
    expect(m.back.mods[2][ModKind.Slow].turns).toBe(2);
    // A pre-upgrade account (no trailing fields) no longer decodes.
    expect(() => c.decodeAccount("Participation", p.bytes.subarray(0, -11))).toThrow(/read past end/);
  });

  it("decodes TurnTicked, TurnSkipped and SkillUsed", () => {
    const c = coders.session;
    const event = (name: string, edit: (v: Record<string, any>) => void) => {
      const v = zero(c, { defined: { name } }) as Record<string, any>;
      v.session = addrOf(3);
      edit(v);
      const out: Uint8Array[] = [Uint8Array.from(c.idl.events!.find((e) => e.name === name)!.discriminator)];
      c.encode({ defined: { name } }, v, out);
      return c.decodeEvent(concat(...out))!;
    };
    const tick = event("TurnTicked", (v) => {
      Object.assign(v, { unit: 5, turnSeq: 12, dotDamage: 4, hp: 6, died: false, skipped: true, expired: 1 << 1 });
      v.states[0] = { kind: "burning", turns: 1, power: 4, source: 1 };
      v.cooldowns[0] = 1;
    });
    expect(tick.name).toBe("TurnTicked");
    expect(tick.data).toMatchObject({ unit: 5, turnSeq: 12, dotDamage: 4, hp: 6, died: false, skipped: true, expired: 2 });
    expect(tick.data.states[0]).toEqual({ kind: { burning: {} }, turns: 1, power: 4, source: 1 });
    expect(tick.data.cooldowns[0]).toBe(1);
    const skip = event("TurnSkipped", (v) => Object.assign(v, { unit: 4, turnSeq: 3, state: "stun" }));
    expect(skip.data).toMatchObject({ unit: 4, turnSeq: 3, state: { stun: {} } });
    const used = event("SkillUsed", (v) => {
      Object.assign(v, { unit: 1, skillSlot: 1, cooldown: 2 });
      v.cooldowns[1] = 3;
    });
    expect(used.data).toMatchObject({ unit: 1, skillSlot: 1, cooldown: 2 });
    expect(used.data.cooldowns[1]).toBe(3);
  });

  it("decodes EffectResolved and ModsTicked", () => {
    const c = coders.session;
    const event = (name: string, edit: (v: Record<string, any>) => void) => {
      const v = zero(c, { defined: { name } }) as Record<string, any>;
      v.session = addrOf(3);
      edit(v);
      const out: Uint8Array[] = [Uint8Array.from(c.idl.events!.find((e) => e.name === name)!.discriminator)];
      c.encode({ defined: { name } }, v, out);
      return c.decodeEvent(concat(...out))!;
    };
    const sap = event("EffectResolved", (v) => {
      Object.assign(v, { caster: 2, target: 5, actionCounter: 7, skillSlot: 1, effect: "weakening", rider: false, applied: "added", hp: 9 });
      Object.assign(v.roll, { outcome: "hit", d20: 20, attackTotal: 23, defense: 12, crit: true, critRoll: null, chanceBp: 0, chanceRoll: null, d100: 40, amount: 2_160, rollsUsed: 2 });
      v.mods[ModKind.Weakening] = { kind: "weakening", aspect: "blood", turns: 3, powerBp: 2_160, source: 2 };
    });
    expect(sap.name).toBe("EffectResolved");
    const e = toEffectResolved(sap.data);
    expect([e.caster, e.target, e.actionCounter, e.skillSlot, e.effect, e.rider, e.applied, e.hp]).toEqual([2, 5, 7, 1, Effect.Weakening, false, Applied.Added, 9]);
    expect(e.roll).toMatchObject({ outcome: HitOutcome.Hit, d20: 20, crit: true, critRoll: null, chanceRoll: null, d100: 40, amount: 2_160 });
    expect(e.mods).toEqual([{ kind: ModKind.Weakening, aspect: Aspect.Blood, turns: 3, powerBp: 2_160, source: 2 }]);
    const mend = toEffectResolved(
      event("EffectResolved", (v) => {
        Object.assign(v, { caster: 2, target: 0, effect: "heal", applied: "none", hp: 25 });
        Object.assign(v.roll, { outcome: "hit", critRoll: 9_000, d100: 70, amount: 7 });
      }).data,
    );
    expect([mend.effect, mend.roll.critRoll, mend.roll.amount, mend.mods]).toEqual([Effect.Heal, 9_000, 7, []]);
    const tick = event("ModsTicked", (v) => {
      Object.assign(v, { unit: 2, turnSeq: 11, expired: 1 << ModKind.Haste, actionPoints: 5 });
    });
    expect(tick.data).toMatchObject({ unit: 2, turnSeq: 11, expired: 1, actionPoints: 5 });
    expect(tick.data.mods).toHaveLength(6);
  });

  it("HitResolved maps to the app HitResult, used entries only", () => {
    const c = coders.session;
    const ev = zero(c, { defined: { name: "HitResolved" } }) as Record<string, any>;
    ev.session = addrOf(3);
    ev.attacker = 1;
    ev.target = 4;
    ev.actionCounter = 2;
    const r = ev.result;
    r.outcome = "hit";
    r.d20 = 20;
    r.crit = true;
    r.tintRollCount = 1;
    r.tintRolls[0] = { base: "fire", tint: "cold", carrier: "race", tier: 14, chanceBp: 700, roll: 39, success: true };
    r.resonances[0] = { base: "fire", tint: "cold", tier: 14, carrier: "race", id: 1, active: true, secondTint: null };
    r.packetCount = 2;
    r.packets[1].aspect = "fire";
    r.packets[1].rolled = 30_000n;
    r.damage = 8;
    r.states[0] = { kind: "chill", turns: 2, power: 1000, source: 1 };
    ev.items.attacker = { weapon: "sword", armor: 0, defense: 0, attack: 1 };
    ev.items.defender.armor = 10;
    const out: Uint8Array[] = [Uint8Array.from(c.idl.events!.find((e) => e.name === "HitResolved")!.discriminator)];
    c.encode({ defined: { name: "HitResolved" } }, ev, out);
    const decoded = c.decodeEvent(concat(...out))!;
    expect(decoded.name).toBe("HitResolved");
    const h = toHitResolved(decoded.data);
    expect([h.attacker, h.target, h.actionCounter]).toEqual([1, 4, 2]);
    expect(h.result.outcome).toBe(HitOutcome.Hit);
    expect(h.result.tintRolls).toHaveLength(1);
    expect(h.result.resonances[0]).toMatchObject({ base: Aspect.Fire, tint: Aspect.Cold, active: true, secondTint: null });
    expect(h.result.resonances[1]).toBeNull();
    expect(h.result.packets).toHaveLength(2);
    expect(h.result.packets[1]).toMatchObject({ aspect: Aspect.Fire, rolled: 30_000n });
    expect(h.result.states).toHaveLength(1);
    expect(h.result.damage).toBe(8);
    // HitResolved.items, after the result: weapon Option<ItemBase>, armor, defense, attack.
    expect(h.items).toEqual({ attacker: { weapon: "sword", armor: 0, defense: 0, attack: 1 }, defender: { weapon: null, armor: 10, defense: 0, attack: 0 } });
    const tail = concat(...out).subarray(-2 * 13 - 1);
    expect(Array.from(tail.subarray(0, 2))).toEqual([1, 0]);
  });

  it("decodeEvents reads session events inside the session invocation only", () => {
    const c = coders.session;
    const ev = zero(c, { defined: { name: "PlayerDied" } }) as Record<string, any>;
    ev.session = addrOf(3);
    ev.slot = 1;
    const out: Uint8Array[] = [Uint8Array.from(c.idl.events!.find((e) => e.name === "PlayerDied")!.discriminator)];
    c.encode({ defined: { name: "PlayerDied" } }, ev, out);
    const b64 = Buffer.from(concat(...out)).toString("base64");
    const logs = [
      `Program ${PROGRAMS.session} invoke [1]`,
      `Program data: ${b64}`,
      `Program ${PROGRAMS.character} invoke [2]`,
      `Program data: ${b64}`,
      `Program ${PROGRAMS.character} success`,
      `Program ${PROGRAMS.session} success`,
    ];
    const evs = decodeEvents(logs, coders, { session: PROGRAMS.session, character: PROGRAMS.character, items: PROGRAMS.items, map: PROGRAMS.map });
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
