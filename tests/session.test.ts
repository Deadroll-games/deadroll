// deadroll_session against the shared local validator (agents/TASKS.md, decision block).
//
// Fresh random keypairs everywhere; nothing assumes an empty ledger.
// Part 1 runs now. Part 2 (the party flow) needs real `Map`/`MapLayout`/`Character` accounts and the
// `map.set_status` / `character.lock_for_session` CPI targets: it skips itself until the character
// (L5) and map (L6) programs are deployed, then `createFinalizedMap` / `createCharacter` below must
// be filled in with their instructions.
import { before, describe, it } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
// Node loads the CJS build; its getter exports are invisible to ESM named imports.
import anchor from "@anchor-lang/core";
import type { DeadrollSession } from "../target/types/deadroll_session.ts";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";

const { AnchorProvider, BN, Program } = anchor;
type BN = InstanceType<typeof BN>;

const idl = JSON.parse(
  readFileSync(new URL("../target/idl/deadroll_session.json", import.meta.url), "utf8"),
) as DeadrollSession;

const provider = AnchorProvider.env();
const connection = provider.connection;
const program = new Program<DeadrollSession>(idl, provider);
const SESSION = program.programId;
const CHARACTER = new PublicKey("9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL");
const MAP = new PublicKey("2YeyBfQ5xkRKD2kagxRS2yZYCQdWt5qidikaKfQhLBXH");
const REGISTRY = new PublicKey("dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG");
const SESSION_KEYS = new PublicKey("KeyspM2ssCJbqUhQ4k7sveSiY4WjnYsrXkC8oDbwde5");

const PRICE = new BN(LAMPORTS_PER_SOL / 10);
const MIN_PRICE = new BN(10_000_000);
const [N, E, S, W] = [0, 1, 2, 3];
const MAX_PATH = 16;
const EMPTY_UNIT = 0xff;

// ---------------------------------------------------------------------------------------------
// helpers

const pda = (seeds: (Buffer | Uint8Array)[], programId: PublicKey) =>
  PublicKey.findProgramAddressSync(seeds, programId)[0];
const sighash = (preimage: string) =>
  createHash("sha256").update(preimage).digest().subarray(0, 8);

const sessionAuthority = pda([Buffer.from("authority")], SESSION);
const config = pda([Buffer.from("config")], REGISTRY);

function sessionAccounts(map: PublicKey) {
  const session = pda([Buffer.from("session"), map.toBuffer()], SESSION);
  return {
    session,
    monsters: pda([Buffer.from("monsters"), session.toBuffer()], SESSION),
    escrow: pda([Buffer.from("escrow"), session.toBuffer()], SESSION),
  };
}

function playerAccounts(session: PublicKey, character: PublicKey) {
  return {
    participation: pda(
      [Buffer.from("participation"), session.toBuffer(), character.toBuffer()],
      SESSION,
    ),
    stash: pda([Buffer.from("stash"), session.toBuffer(), character.toBuffer()], SESSION),
  };
}

async function funded(sol = 10): Promise<Keypair> {
  const kp = Keypair.generate();
  const sig = await connection.requestAirdrop(kp.publicKey, sol * LAMPORTS_PER_SOL);
  const latest = await connection.getLatestBlockhash();
  await connection.confirmTransaction({ signature: sig, ...latest }, "confirmed");
  return kp;
}

async function isProgram(id: PublicKey): Promise<boolean> {
  const info = await connection.getAccountInfo(id);
  return info?.executable ?? false;
}

/** Asserts the promise rejects with the Anchor error `code` (program or constraint error name). */
async function rejects(promise: Promise<unknown>, code: string): Promise<void> {
  try {
    await promise;
  } catch (e: any) {
    const name = e?.error?.errorCode?.code;
    const logs: string[] = e?.logs ?? e?.transactionLogs ?? [];
    const text = `${e?.message ?? e}\n${logs.join("\n")}`;
    assert.ok(
      name === code || text.includes(`Error Code: ${code}`),
      `expected ${code}, got ${name ?? text}`,
    );
    return;
  }
  assert.fail(`expected ${code}, transaction succeeded`);
}

function path(steps: number[]): [number[], number] {
  const padded = [...steps.slice(0, MAX_PATH)];
  while (padded.length < MAX_PATH) padded.push(0);
  return [padded, steps.length];
}

/** MagicBlock Session Keys `create_session_v2`, signed by the session key and the main wallet. */
async function issueToken(
  signer: Keypair,
  authority: Keypair,
  opts: { target?: PublicKey; validUntil?: number } = {},
): Promise<PublicKey> {
  const target = opts.target ?? SESSION;
  const token = pda(
    [
      Buffer.from("session_token_v2"),
      target.toBuffer(),
      signer.publicKey.toBuffer(),
      authority.publicKey.toBuffer(),
    ],
    SESSION_KEYS,
  );
  const validUntil =
    opts.validUntil === undefined
      ? Buffer.from([0])
      : Buffer.concat([Buffer.from([1]), new BN(opts.validUntil).toArrayLike(Buffer, "le", 8)]);
  const data = Buffer.concat([
    sighash("global:create_session_v2"),
    Buffer.from([0]), // top_up: None
    validUntil,
    Buffer.from([0]), // lamports: None
  ]);
  const ix = new TransactionInstruction({
    programId: SESSION_KEYS,
    keys: [
      { pubkey: token, isSigner: false, isWritable: true },
      { pubkey: signer.publicKey, isSigner: true, isWritable: true },
      { pubkey: authority.publicKey, isSigner: true, isWritable: true }, // fee payer
      { pubkey: authority.publicKey, isSigner: true, isWritable: false },
      { pubkey: target, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
  await provider.sendAndConfirm(new Transaction().add(ix), [signer, authority]);
  return token;
}

async function revokeToken(token: PublicKey, authority: PublicKey): Promise<void> {
  const ix = new TransactionInstruction({
    programId: SESSION_KEYS,
    keys: [
      { pubkey: token, isSigner: false, isWritable: true },
      { pubkey: authority, isSigner: false, isWritable: true }, // fee payer gets the rent back
      { pubkey: authority, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: sighash("global:revoke_session_v2"),
  });
  await provider.sendAndConfirm(new Transaction().add(ix), []);
}

function openSession(dm: Keypair, map: PublicKey, layout: PublicKey, price: BN, dmActor: PublicKey) {
  return program.methods
    .openSession(price, dmActor)
    .accountsStrict({
      dm: dm.publicKey,
      map,
      layout,
      ...sessionAccounts(map),
      sessionAuthority,
      config,
      mapProgram: MAP,
      systemProgram: SystemProgram.programId,
    })
    .signers([dm]);
}

// ---------------------------------------------------------------------------------------------
// Part 1: runs on the shared validator today

describe("deadroll_session: deployed program", () => {
  it("is deployed", async () => {
    assert.ok(await isProgram(SESSION), "anchor deploy -p deadroll_session first");
  });

  it("open_session rejects an account that is not a deadroll map", async () => {
    const dm = await funded();
    const map = Keypair.generate().publicKey;
    await rejects(
      openSession(dm, map, Keypair.generate().publicKey, PRICE, PublicKey.default).rpc(),
      "InvalidMap",
    );
  });

  it("open_session rejects a wrong GameConfig address", async () => {
    const dm = await funded();
    const map = Keypair.generate().publicKey;
    await rejects(
      openSession(dm, map, map, PRICE, PublicKey.default)
        .accountsPartial({ config: Keypair.generate().publicKey })
        .rpc(),
      "ConstraintSeeds",
    );
  });

  it("open_session rejects a wrong map program", async () => {
    const dm = await funded();
    const map = Keypair.generate().publicKey;
    await rejects(
      openSession(dm, map, map, PRICE, PublicKey.default)
        .accountsPartial({ mapProgram: CHARACTER })
        .rpc(),
      "ConstraintAddress",
    );
  });

  it("game instructions reject a session that was never opened", async () => {
    const signer = await funded();
    const { session, monsters, escrow } = sessionAccounts(Keypair.generate().publicKey);
    await rejects(
      program.methods
        .startSession()
        .accountsStrict({ dm: signer.publicKey, session, monsters, escrow })
        .signers([signer])
        .rpc(),
      "AccountNotInitialized",
    );
    await rejects(
      program.methods
        .endTurn()
        .accountsStrict({
          signer: signer.publicKey,
          session,
          monsters,
          participation: null,
          sessionToken: null,
        })
        .signers([signer])
        .rpc(),
      "AccountNotInitialized",
    );
  });

  it("CPI stubs use the Anchor sighashes of the callee instructions", () => {
    // The program hardcodes these; the party flow below checks the bytes it actually sends.
    assert.deepEqual([...sighash("global:lock_for_session")], [0xec, 0x7e, 0x7f, 0x53, 0xf6, 0x0b, 0xa2, 0xc0]);
    assert.deepEqual([...sighash("global:set_status")], [0xb5, 0xb8, 0xe0, 0xcb, 0xc1, 0x1d, 0xb1, 0xe0]);
    assert.deepEqual([...sighash("account:Character")], [0x8c, 0x73, 0xa5, 0x24, 0xf1, 0x99, 0x66, 0x54]);
    assert.deepEqual([...sighash("account:Map")], [0xb6, 0x1e, 0x8e, 0x97, 0x2a, 0xf1, 0xb4, 0xf4]);
    assert.deepEqual([...sighash("account:MapLayout")], [0x8e, 0x57, 0xdf, 0xf4, 0x3b, 0x7b, 0x3e, 0xca]);
  });
});

// ---------------------------------------------------------------------------------------------
// Part 2: party flow, needs the character (L5) and map (L6) programs

// Flip once `createFinalizedMap` / `createCharacter` below use the merged L5/L6 instructions.
const FIXTURES_IMPLEMENTED = false;
const depsReady = FIXTURES_IMPLEMENTED && (await isProgram(CHARACTER)) && (await isProgram(MAP));

/** Test map: 8x8 floor, entrance (0,0), shallow water (1,0), wall (0,2), two wolves at (5,5) and (6,5). */
interface MapFixture {
  map: PublicKey;
  layout: PublicKey;
}

// TODO(L6): create, write and finalize the test map above through `deadroll_map`.
async function createFinalizedMap(_dm: Keypair): Promise<MapFixture> {
  throw new Error("needs deadroll_map create_map / write_layout / finalize_map (L6)");
}

// TODO(L5): create a level-1 character (Dexterity 10, hp_max 30) through `deadroll_character`.
async function createCharacter(
  _owner: Keypair,
  _race: "human" | "forestElf",
  _class: "warrior" | "mage",
): Promise<PublicKey> {
  throw new Error("needs deadroll_character create_character (L5)");
}

describe(
  "deadroll_session: party flow",
  {
    skip: depsReady
      ? false
      : "needs map/character fixtures from the L5/L6 instructions and both programs deployed",
  },
  () => {
    let dm: Keypair, dmActor: Keypair, stranger: Keypair;
    const players: { wallet: Keypair; actor: Keypair; character: PublicKey; participation: PublicKey; stash: PublicKey }[] = [];
    let fixture: MapFixture;
    let acc: ReturnType<typeof sessionAccounts>;
    let p0Token: PublicKey, p1Token: PublicKey, dmToken: PublicKey;

    const move = (who: number, signer: Keypair, steps: number[], token: PublicKey | null = null) => {
      const [padded, len] = path(steps);
      return program.methods
        .moveUnit(padded, len)
        .accountsStrict({
          signer: signer.publicKey,
          session: acc.session,
          participation: players[who].participation,
          monsters: acc.monsters,
          layout: fixture.layout,
          sessionToken: token,
        })
        .signers([signer])
        .rpc();
    };
    const monsterMove = (signer: Keypair, monster: number, steps: number[], token: PublicKey | null = null) => {
      const [padded, len] = path(steps);
      return program.methods
        .monsterMove(monster, padded, len)
        .accountsStrict({
          signer: signer.publicKey,
          session: acc.session,
          monsters: acc.monsters,
          layout: fixture.layout,
          sessionToken: token,
        })
        .signers([signer])
        .rpc();
    };
    const endTurn = (signer: Keypair, who: number | null, token: PublicKey | null = null) =>
      program.methods
        .endTurn()
        .accountsStrict({
          signer: signer.publicKey,
          session: acc.session,
          monsters: acc.monsters,
          participation: who === null ? null : players[who].participation,
          sessionToken: token,
        })
        .signers([signer])
        .rpc();
    const join = (who: number, signer: Keypair, contribution: BN) =>
      program.methods
        .joinSession(contribution, players[who].actor.publicKey)
        .accountsStrict({
          player: signer.publicKey,
          session: acc.session,
          character: players[who].character,
          participation: players[who].participation,
          stash: players[who].stash,
          escrow: acc.escrow,
          sessionAuthority,
          config,
          characterProgram: CHARACTER,
          systemProgram: SystemProgram.programId,
        })
        .signers([signer])
        .rpc();
    const ready = (who: number, signer: Keypair, value: boolean) =>
      program.methods
        .setReady(value)
        .accountsStrict({ player: signer.publicKey, session: acc.session, participation: players[who].participation })
        .signers([signer])
        .rpc();
    const start = (signer: Keypair) =>
      program.methods
        .startSession()
        .accountsStrict({ dm: signer.publicKey, session: acc.session, monsters: acc.monsters, escrow: acc.escrow })
        .signers([signer])
        .rpc();
    const state = () => program.account.gameSession.fetch(acc.session) as Promise<any>;
    const part = (who: number) => program.account.participation.fetch(players[who].participation) as Promise<any>;
    const monsters = () => program.account.monsterState.fetch(acc.monsters) as Promise<any>;

    before(async () => {
      [dm, dmActor, stranger] = await Promise.all([funded(), funded(1), funded(1)]);
      fixture = await createFinalizedMap(dm);
      acc = sessionAccounts(fixture.map);
      for (const [race, cls] of [["human", "warrior"], ["forestElf", "mage"]] as const) {
        const [wallet, actor] = await Promise.all([funded(), funded(1)]);
        const character = await createCharacter(wallet, race, cls);
        players.push({ wallet, actor, character, ...playerAccounts(acc.session, character) });
      }
    });

    it("open_session: DM only, minimum price, then opens and spawns monsters", async () => {
      await rejects(openSession(players[0].wallet, fixture.map, fixture.layout, PRICE, dmActor.publicKey).rpc(), "NotDm");
      await rejects(openSession(dm, fixture.map, fixture.layout, MIN_PRICE.subn(1), dmActor.publicKey).rpc(), "PriceTooLow");
      const sig = await openSession(dm, fixture.map, fixture.layout, PRICE, dmActor.publicKey).rpc({ commitment: "confirmed" });

      const s = await state();
      assert.ok(s.core.status.lobby);
      assert.ok(s.core.dm.equals(dm.publicKey));
      assert.ok(s.core.priceLamports.eq(PRICE));
      assert.deepEqual([s.entrance.x, s.entrance.y], [0, 0]);
      assert.equal(s.core.turnOrder[0], EMPTY_UNIT);
      const m = await monsters();
      assert.equal(m.core.count, 2);
      assert.ok(m.core.monsters[0].alive && m.core.monsters[1].alive);
      assert.deepEqual([m.core.monsters[0].x, m.core.monsters[0].y], [5, 5]);

      const tx = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
      const inner = tx!.meta!.innerInstructions!.flatMap((i) => i.instructions);
      const keys = tx!.transaction.message.staticAccountKeys;
      const toMap = inner.find((ix) => keys[ix.programIdIndex].equals(MAP));
      assert.ok(toMap, "map.set_status CPI");

      await rejects(openSession(dm, fixture.map, fixture.layout, PRICE, dmActor.publicKey).rpc(), "already in use");
    });

    it("join_session: owner only, pays only the missing part of the price", async () => {
      await rejects(join(0, players[1].wallet, PRICE), "NotCharacterOwner");
      await rejects(join(0, players[0].actor, PRICE), "NotCharacterOwner");
      const before = await connection.getBalance(acc.escrow);
      await join(0, players[0].wallet, PRICE.muln(3));
      assert.equal((await connection.getBalance(acc.escrow)) - before, PRICE.toNumber());
      await join(1, players[1].wallet, PRICE);

      const [p0, p1] = await Promise.all([part(0), part(1)]);
      assert.ok(p0.contribution.eq(PRICE));
      assert.ok(p1.contribution.eqn(0));
      assert.equal(p0.core.slot, 0);
      assert.equal(p1.core.slot, 1);
      assert.equal(p0.core.hp, 30);
      assert.equal(p0.maxAp, 5);
      assert.ok(p0.core.actor.equals(players[0].actor.publicKey));
      assert.ok(p0.player.equals(players[0].wallet.publicKey));
      const escrow = (await program.account.escrow.fetch(acc.escrow)) as any;
      assert.ok(escrow.collected.eq(PRICE));
      await rejects(join(0, players[0].wallet, PRICE), "already in use");
    });

    it("set_ready / start_session: main wallets only, everyone ready, DM starts", async () => {
      await rejects(ready(0, players[0].actor, true), "NotPlayer");
      await rejects(ready(0, players[1].wallet, true), "NotPlayer");
      await ready(0, players[0].wallet, true);
      await rejects(start(dm), "NotAllReady");
      await ready(1, players[1].wallet, true);
      await ready(1, players[1].wallet, false);
      assert.equal((await state()).readyMask, 0b01);
      await ready(1, players[1].wallet, true);
      await rejects(start(players[0].wallet), "NotDm");
      await rejects(start(dmActor), "NotDm");
      await start(dm);

      const s = await state();
      assert.ok(s.core.status.active);
      assert.deepEqual(s.core.turnOrder.slice(0, 5), [0, 1, 4, 5, EMPTY_UNIT]);
      assert.equal(s.core.round, 1);
      assert.equal(s.turnSeq, 1);
      await rejects(start(dm), "NotInLobby");
      await rejects(ready(0, players[0].wallet, false), "NotInLobby");
    });

    it("auth: main wallet or bound session key with a live token from that wallet", async () => {
      const [p0, p1] = players;
      await rejects(move(0, stranger, [S]), "Unauthorized");
      await rejects(move(0, dm, [S]), "Unauthorized");
      await rejects(move(0, p0.actor, [S]), "SessionTokenMissing");
      await rejects(move(0, p0.actor, [S], p0.wallet.publicKey), "InvalidSessionToken"); // not a token
      const foreign = await issueToken(p0.actor, p1.wallet);
      await rejects(move(0, p0.actor, [S], foreign), "InvalidSessionToken");
      const otherTarget = await issueToken(p0.actor, p0.wallet, { target: SESSION_KEYS });
      await rejects(move(0, p0.actor, [S], otherTarget), "InvalidSessionToken");
      const now = (await connection.getBlockTime(await connection.getSlot()))!;
      const expired = await issueToken(p0.actor, p0.wallet, { validUntil: now - 60 });
      await rejects(move(0, p0.actor, [S], expired), "SessionTokenExpired");
      await revokeToken(expired, p0.wallet.publicKey);
      p0Token = await issueToken(p0.actor, p0.wallet);
      p1Token = await issueToken(p1.actor, p1.wallet);
      await rejects(move(0, p1.actor, [S], p1Token), "Unauthorized");
      const loose = await funded(1);
      const looseToken = await issueToken(loose, p0.wallet);
      await rejects(move(0, loose, [S], looseToken), "Unauthorized");
    });

    it("move_unit: path, tiles, occupancy and action points", async () => {
      const key = players[0].actor;
      await rejects(move(1, players[1].wallet, [S]), "NotYourTurn");
      await rejects(move(0, key, [], p0Token), "InvalidPathLength");
      await rejects(move(0, key, Array(17).fill(S), p0Token), "InvalidPathLength");
      await rejects(move(0, key, [4], p0Token), "InvalidDirection");
      await rejects(move(0, key, [N], p0Token), "OutOfBounds");
      await rejects(move(0, key, [W], p0Token), "OutOfBounds");
      await rejects(move(0, key, [S, S], p0Token), "CellBlocked");
      await rejects(move(0, key, [S, N], p0Token), "CellOccupied"); // player 1 on the entrance
      await rejects(move(0, key, [S, E, E, E, E, E], p0Token), "NotEnoughActionPoints");

      await move(0, key, [E, E], p0Token); // water 2 + floor 1
      let p = await part(0);
      assert.deepEqual([p.core.x, p.core.y, p.core.actionPoints], [2, 0, 2]);
      await rejects(move(0, key, [S, S, S], p0Token), "NotEnoughActionPoints");
      await move(0, players[0].wallet, [S]);
      p = await part(0);
      assert.deepEqual([p.core.x, p.core.y, p.core.actionPoints], [2, 1, 1]);
      assert.deepEqual([(await state()).playerPos[0].x, (await state()).playerPos[0].y], [2, 1]);
    });

    it("end_turn: the current unit's player or the DM, then round-robin", async () => {
      await rejects(endTurn(players[0].wallet, null), "ParticipationMissing");
      await rejects(endTurn(players[1].wallet, 1), "NotYourTurn");
      await rejects(endTurn(players[1].wallet, 0), "Unauthorized");
      await endTurn(players[0].actor, 0, p0Token);
      let s = await state();
      assert.deepEqual([s.core.turnIndex, s.turnSeq], [1, 2]);

      await move(1, players[1].actor, [E], p1Token);
      await endTurn(players[1].wallet, 1);
      s = await state();
      assert.equal(s.core.turnIndex, 2);
      assert.equal((await monsters()).core.monsters[0].actionPoints, 4);
    });

    it("monster_move: DM or DM session key, on that monster's turn", async () => {
      await rejects(monsterMove(players[0].wallet, 0, [N]), "Unauthorized");
      await rejects(monsterMove(players[0].actor, 0, [N], p0Token), "Unauthorized");
      await rejects(monsterMove(dm, 1, [N]), "NotYourTurn");
      await rejects(monsterMove(dm, 2, [N]), "InvalidMonster");
      await rejects(monsterMove(dm, 0, [E]), "CellOccupied");
      await rejects(monsterMove(dm, 0, [N, N, N, N, N]), "NotEnoughActionPoints");
      await rejects(monsterMove(dmActor, 0, [N]), "SessionTokenMissing");
      dmToken = await issueToken(dmActor, dm);
      await monsterMove(dmActor, 0, [N], dmToken);
      await monsterMove(dm, 0, [W]);
      const m = (await monsters()).core.monsters[0];
      assert.deepEqual([m.x, m.y, m.actionPoints], [4, 4, 2]);

      await rejects(endTurn(players[0].wallet, 0), "Unauthorized");
      await endTurn(dmActor, null, dmToken);
      await endTurn(dm, null);
      const s = await state();
      assert.deepEqual([s.core.turnIndex, s.core.round, s.turnSeq], [0, 2, 5]);
    });

    it("action points refill on the next own turn; monsters block", async () => {
      await move(0, players[0].wallet, [E]); // (3,1), fresh AP 5 -> 4
      assert.equal((await part(0)).core.actionPoints, 4);
      await rejects(move(0, players[0].wallet, [E, S, S, S]), "CellOccupied"); // wolf at (4,4)
    });

    it("lobby is closed after start", async () => {
      const late = await funded();
      const character = await createCharacter(late, "human", "warrior");
      players.push({ wallet: late, actor: late, character, ...playerAccounts(acc.session, character) });
      await rejects(join(2, late, PRICE), "NotInLobby");
    });
  },
);
