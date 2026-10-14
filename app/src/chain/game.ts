// The demo script as instructions and reads, mirroring tests/demo.test.ts (class Demo) call for
// call: same instructions, same account lists, same seeds. Screens call these; nothing here
// touches React.
import {
  createKeyPairSignerFromPrivateKeyBytes,
  generateKeyPairSigner,
  getAddressEncoder,
  type Address,
  type Base64EncodedDataResponse,
  type Instruction,
  type KeyPairSigner,
  type TransactionSigner,
} from "@solana/kit";
import { MPL_CORE_PROGRAM, PROGRAMS, SYSTEM_PROGRAM, VRF } from "../config";
import { client } from "../wallet/client";
import { characterAppearance } from "./appearance";
import { fromBase64, sha256 } from "./bytes";
import { initLoadoutIx } from "./items";
import { buildInstruction, coders } from "./programs";
import {
  bagPda,
  bestiaryPda,
  characterPda,
  escrowPda,
  gameConfigPda,
  layoutPda,
  loadoutPda,
  mapPda,
  monstersPda,
  participationPda,
  profilePda,
  programIdentityPda,
  sessionAuthorityPda,
  sessionPda,
  skillSetPda,
  stashPda,
  tilesetPda,
} from "./pda";
import { pathArgs, type Step } from "./route";
import { PRESETS, SKILL_NAME_LEN, type NamedSkill } from "./skillcheck";
import { Aspect, Class, MAP_CELLS, MAP_MAX, Race, Tile, type Cell, type Spawn } from "./types";

// ---------------------------------------------------------------------------------------------
// decoded account shapes (Anchor TS coder conventions, see coder.ts)

export type Enum = Record<string, Record<string, unknown>>;
export type ActiveStateAcc = { kind: Enum; turns: number; power: number; source: number };
/** A buff or debuff slot (session TimedModData): active while `turns > 0`. One per kind, in the first free slot: find by `kind`. */
export type TimedModAcc = { kind: Enum; aspect: Enum; turns: number; powerBp: number; source: number };
export type MonsterAcc = {
  bestiaryId: number;
  level: number;
  x: number;
  y: number;
  hp: number;
  hpMax: number;
  actionPoints: number;
  alive: boolean;
  pack: number;
  flags: number;
  states: ActiveStateAcc[];
};
export type GameSessionAcc = {
  core: {
    version: number;
    map: Address;
    dm: Address;
    status: Enum;
    priceLamports: bigint;
    players: Address[];
    playerCount: number;
    turnOrder: Uint8Array;
    turnIndex: number;
    round: number;
    actionCounter: number;
    pendingVrf: boolean;
    lastSeed: Uint8Array;
    bump: number;
  };
  layout: Address;
  width: number;
  height: number;
  entrance: Cell;
  dmActor: Address;
  turnOrderLen: number;
  turnSeq: number;
  turnStartedAt: bigint;
  playerPos: Cell[];
  aliveMask: number;
  readyMask: number;
  pending: { attacker: number; target: number; skillSlot: number; participation: Address };
  fleeMask: number;
  settledMask: number;
  xpPool: number;
  victorySeed: Uint8Array;
  monsterSpeed: Uint8Array;
  monsterInitiative: number[];
  playerInitiative: number[];
  tileCost: Uint8Array;
  /** `turnSeq` in which the current unit used its one attack or active skill. */
  actedSeq: number;
  /** `turnSeq` whose `start_turn` ran; every other turn instruction waits for it. */
  startedSeq: number;
  /** The ally Participation of a pending heal or buff on another player; the default key = none. */
  pendingTarget: Address;
};
export type ParticipationAcc = {
  core: {
    session: Address;
    character: Address;
    actor: Address;
    slot: number;
    x: number;
    y: number;
    hp: number;
    actionPoints: number;
    states: ActiveStateAcc[];
    ready: boolean;
  };
  player: Address;
  maxAp: number;
  apSeq: number;
  contribution: bigint;
  /** Per SkillSet slot: own turn starts until the skill is usable again, 0 = usable. */
  cooldowns: Uint8Array;
  /** The character's maximum HP, copied at join_session (heals stop here). */
  hpMax: number;
  /** Buffs and debuffs on this player (one per kind, any slot: read them with support.ts activeMods). */
  mods: TimedModAcc[];
  /** What the character wears, `ItemBase` bytes by `EquipSlot` (0xFF = none), copied from its items loadout at join_session. */
  equipment: Uint8Array;
};
export type CharacterAcc = {
  core: {
    owner: Address;
    index: number;
    nft: Address | null;
    race: Enum;
    class: Enum;
    level: number;
    xp: number;
    stats: number[];
    predisposition: Enum;
    hpMax: number;
    alive: boolean;
    inSession: Address | null;
    appearance: { spriteId: number; materialRamp: number; accentRamp: number; elementRamp: number }[];
    stateVersion: number;
  };
  name: Uint8Array;
  unspentPoints: number;
  nftFrozen: boolean;
};
/** programs/character SkillRecord: crates/skillcheck SkillBlocks + derived values. */
export type SkillRecordAcc = {
  blocks: {
    category: Enum;
    purpose: Enum;
    source: Enum;
    form: Enum;
    target: Enum;
    aspects: Enum[];
    aspectCount: number;
    effect: Enum;
    trigger: Enum;
    auraRadius: number;
  };
  sharesBp: number[];
  powerBp: number;
  fusion: number;
  cooldown: number;
  name: Uint8Array;
};
export type SkillSetAcc = { character: Address; count: number; skills: SkillRecordAcc[] };
export const variantKey = (e: Enum): string => Object.keys(e)[0] ?? "";
/** Enum variant as a label: { enemyAdjacent: {} } -> "Enemy adjacent". */
export function variantLabel(e: Enum): string {
  const words = variantKey(e).replace(/([a-z0-9])([A-Z])/g, "$1 $2").toLowerCase();
  return words.charAt(0).toUpperCase() + words.slice(1);
}
export type MapAcc = {
  core: { dm: Address; index: number; width: number; height: number; status: Enum; layoutHash: Uint8Array };
  tier: number;
};

export const MONSTER_BASE = 4;
export const BASE_ATTACK = 0xff;
export const status = (s: GameSessionAcc) => Object.keys(s.core.status)[0] as "lobby" | "active" | "victory" | "wiped" | "fled" | "abandoned";
export const mapStatus = (m: MapAcc) => Object.keys(m.core.status)[0] as "draft" | "finalized" | "inSession" | "finished";
export const enumIndex = (e: Enum, names: readonly string[]) => names.indexOf(Object.keys(e)[0] ?? "");
export const ASPECT_KEYS = ["fire", "cold", "lightning", "blood", "blunt", "piercing"] as const;
export const RACE_KEYS = ["human", "forestElf"] as const;
export const CLASS_KEYS = ["warrior", "mage", "support"] as const;
/** ModKindData variants in on-chain order; `expired` bit i is `1 << i` (deadroll_combat::support::mod_bit). */
export const MOD_KEYS = ["haste", "slow", "empowerment", "weakening", "protection", "vulnerability"] as const;
/** `Pubkey::default()`: no account. */
export const DEFAULT_KEY = "11111111111111111111111111111111" as Address;
export const currentUnit = (s: GameSessionAcc): number | null =>
  status(s) === "active" && s.turnOrderLen > 0 ? (s.core.turnOrder[s.core.turnIndex] ?? null) : null;

export function textBytes(s: string, len: number): Uint8Array {
  const out = new Uint8Array(len);
  out.set(new TextEncoder().encode(s.replace(/[^\x20-\x7e]/g, "?")).subarray(0, len));
  return out;
}
export function bytesText(b: Uint8Array): string {
  const end = b.indexOf(0);
  return new TextDecoder().decode(end < 0 ? b : b.subarray(0, end));
}

// ---------------------------------------------------------------------------------------------
// reads

export async function getAccounts(addresses: Address[]): Promise<(Uint8Array | null)[]> {
  if (addresses.length === 0) return [];
  const { value } = await client.rpc.getMultipleAccounts(addresses, { encoding: "base64", commitment: "confirmed" }).send();
  return value.map((v) => (v ? fromBase64((v.data as Base64EncodedDataResponse)[0]) : null));
}

export async function getAccount<T>(program: keyof typeof coders, name: string, address: Address): Promise<T | null> {
  const [raw] = await getAccounts([address]);
  return raw ? coders[program].decodeAccount<T>(name, raw) : null;
}

const addressEncoder = getAddressEncoder();
/** Accounts of `program` whose data matches the discriminator of `name` and the given memcmp. */
export async function findAccounts<T>(
  program: keyof typeof coders,
  name: string,
  memcmp: { offset: number; bytes: Uint8Array }[] = [],
): Promise<{ address: Address; data: T }[]> {
  const coder = coders[program];
  const filters = [
    { memcmp: { offset: 0n, bytes: base64(coder.accountDiscriminator(name)), encoding: "base64" as const } },
    ...memcmp.map((m) => ({ memcmp: { offset: BigInt(m.offset), bytes: base64(m.bytes), encoding: "base64" as const } })),
  ];
  const res = await client.rpc
    .getProgramAccounts(PROGRAMS[program], { encoding: "base64", commitment: "confirmed", filters } as never)
    .send();
  const list = res as unknown as { pubkey: Address; account: { data: Base64EncodedDataResponse } }[];
  const out: { address: Address; data: T }[] = [];
  for (const { pubkey, account } of list) {
    try {
      out.push({ address: pubkey, data: coder.decodeAccount<T>(name, fromBase64(account.data[0])) });
    } catch {
      // older layout: skip
    }
  }
  return out;
}

function base64(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
}

// ---------------------------------------------------------------------------------------------
// characters

/** crates/skillcheck presets, the starting sets the program accepts per class (typed mirror in ./skillcheck). */
export const SKILL_PRESETS: Record<Class, NamedSkill[]> = PRESETS;

/** Predispositions by race (create_character doc: human physical, forest elf elemental). */
export const RACE_PREDISPOSITIONS: Record<Race, Aspect[]> = {
  [Race.Human]: [Aspect.Piercing, Aspect.Blunt, Aspect.Blood],
  [Race.ForestElf]: [Aspect.Fire, Aspect.Cold, Aspect.Lightning],
};

/** `skills` defaults to the class presets. */
export type NewCharacter = { name: string; race: Race; class: Class; predisposition: Aspect; stats: number[]; skills?: NamedSkill[] };

export function createProfileIx(owner: TransactionSigner, profile: Address): Instruction {
  return buildInstruction("character", "create_profile", {}, { owner, profile, systemProgram: SYSTEM_PROGRAM });
}

/**
 * create_profile if missing, then create_character with a fresh Core NFT asset keypair and the
 * items loadout with the class starter kit (`loadoutIx`, sent in the same transaction as `ix`).
 */
export async function createCharacterIxs(
  owner: TransactionSigner,
  c: NewCharacter,
): Promise<{ profileIx: Instruction | null; ix: Instruction; loadoutIx: Instruction; character: Address; asset: KeyPairSigner }> {
  const profile = await profilePda(owner.address);
  const prof = await getAccount<{ characterCount: number }>("character", "PlayerProfile", profile);
  const index = prof?.characterCount ?? 0;
  const character = await characterPda(owner.address, index);
  const asset = await generateKeyPairSigner();
  const args = {
    args: {
      name: textBytes(c.name, 32),
      race: RACE_KEYS[c.race],
      class: CLASS_KEYS[c.class],
      predisposition: ASPECT_KEYS[c.predisposition],
      stats: c.stats,
      appearance: characterAppearance(c.race, c.class, c.predisposition),
      skills: (c.skills ?? SKILL_PRESETS[c.class]).map((s) => ({ blocks: s.blocks, name: textBytes(s.name, SKILL_NAME_LEN) })),
    },
  };
  const ix = buildInstruction("character", "create_character", args, {
    owner,
    profile,
    character,
    skillSet: await skillSetPda(character),
    bag: await bagPda(character),
    config: await gameConfigPda(),
    asset,
    mplCoreProgram: MPL_CORE_PROGRAM,
    systemProgram: SYSTEM_PROGRAM,
  });
  return { profileIx: prof ? null : createProfileIx(owner, profile), ix, loadoutIx: await initLoadoutIx(owner, character), character, asset };
}

/** All characters of `owner`, by profile count (indices 0..count). */
export async function myCharacters(owner: Address): Promise<{ address: Address; data: CharacterAcc }[]> {
  const prof = await getAccount<{ characterCount: number }>("character", "PlayerProfile", await profilePda(owner));
  const count = prof?.characterCount ?? 0;
  const addrs = await Promise.all(Array.from({ length: count }, (_, i) => characterPda(owner, i)));
  const raws = await getAccounts(addrs);
  return addrs.flatMap((address, i) => {
    const raw = raws[i];
    return raw ? [{ address, data: coders.character.decodeAccount<CharacterAcc>("Character", raw) }] : [];
  });
}

export function freezeTombstoneIx(character: Address, asset: Address, payer: TransactionSigner): Instruction {
  return buildInstruction("character", "freeze_tombstone", {}, { character, asset, payer, mplCoreProgram: MPL_CORE_PROGRAM, systemProgram: SYSTEM_PROGRAM });
}

// ---------------------------------------------------------------------------------------------
// maps

export type MapDraft = { width: number; height: number; tier: number; tiles: Uint8Array; spawns: Spawn[] };

/** Tiles exactly as the chain stores them: stride 32, cells outside the map Floor (0). */
export function chainTiles(d: MapDraft): Uint8Array {
  const out = new Uint8Array(MAP_CELLS);
  for (let y = 0; y < d.height; y++) for (let x = 0; x < d.width; x++) out[y * MAP_MAX + x] = d.tiles[y * MAP_MAX + x] ?? Tile.Wall;
  return out;
}

export function findTile(d: MapDraft, tile: Tile): Cell | null {
  for (let y = 0; y < d.height; y++) for (let x = 0; x < d.width; x++) if (d.tiles[y * MAP_MAX + x] === tile) return { x, y };
  return null;
}

/** create_map, write_layout, write_spawns, finalize_map (one transaction each, as the test). */
export async function buildMapIxs(dm: TransactionSigner, d: MapDraft): Promise<{ map: Address; layout: Address; index: number; ixs: Instruction[] }> {
  const index = crypto.getRandomValues(new Uint32Array(1))[0]!;
  const map = await mapPda(dm.address, index);
  const layout = await layoutPda(map);
  const edit = { dm, map, layout };
  const tiles = chainTiles(d);
  const entrance = findTile(d, Tile.Entrance);
  const exit = findTile(d, Tile.Exit);
  if (!entrance || !exit) throw new Error("the map needs an entrance and an exit");
  const ixs = [
    buildInstruction("map", "create_map", { index, width: d.width, height: d.height, tier: d.tier }, { ...edit, systemProgram: SYSTEM_PROGRAM }),
    buildInstruction("map", "write_layout", { offset: 0, tiles: tiles.slice(0, (d.height - 1) * MAP_MAX + d.width) }, edit),
    buildInstruction("map", "write_spawns", { entrance, exit, spawns: d.spawns }, edit),
    buildInstruction("map", "finalize_map", { secretsRoot: new Uint8Array(32) }, edit),
  ];
  return { map, layout, index, ixs };
}

/** Maps of `dm` (memcmp on MapData.dm right after version). */
export function mapsOf(dm: Address) {
  return findAccounts<MapAcc>("map", "Map", [{ offset: 9, bytes: addrBytes(dm) }]);
}

export async function mapTiles(layout: Address): Promise<Uint8Array | null> {
  const l = await getAccount<{ core: { tiles: Uint8Array } }>("map", "MapLayout", layout);
  return l?.core.tiles ?? null;
}

function addrBytes(a: Address): Uint8Array {
  return addressEncoder.encode(a) as Uint8Array;
}

// ---------------------------------------------------------------------------------------------
// session

export type SessionAccounts = { session: Address; monsters: Address; escrow: Address };

export async function sessionAccounts(map: Address): Promise<SessionAccounts> {
  const session = await sessionPda(map);
  return { session, monsters: await monstersPda(session), escrow: await escrowPda(session) };
}

async function vrfAccounts() {
  return {
    programIdentity: await programIdentityPda(),
    oracleQueue: VRF.oracleQueue,
    vrfProgram: VRF.program,
    slotHashes: VRF.slotHashes,
    systemProgram: SYSTEM_PROGRAM,
  };
}

export async function openSessionIx(dm: TransactionSigner, map: Address, priceLamports: bigint, dmActor: Address): Promise<Instruction> {
  const acc = await sessionAccounts(map);
  return buildInstruction(
    "session",
    "open_session",
    { priceLamports, dmActor },
    {
      dm,
      map,
      layout: await layoutPda(map),
      ...acc,
      sessionAuthority: await sessionAuthorityPda(),
      config: await gameConfigPda(),
      bestiary: await bestiaryPda(),
      tileset: await tilesetPda(),
      mapProgram: PROGRAMS.map,
      systemProgram: SYSTEM_PROGRAM,
    },
  );
}

export async function joinSessionIx(player: TransactionSigner, session: Address, character: Address, contribution: bigint, actor: Address): Promise<Instruction> {
  return buildInstruction(
    "session",
    "join_session",
    { contribution, actor },
    {
      player,
      session,
      character,
      participation: await participationPda(session, character),
      stash: await stashPda(session, character),
      escrow: await escrowPda(session),
      sessionAuthority: await sessionAuthorityPda(),
      config: await gameConfigPda(),
      characterProgram: PROGRAMS.character,
      systemProgram: SYSTEM_PROGRAM,
      loadout: await loadoutPda(character),
    },
  );
}

export async function setReadyIx(player: TransactionSigner, session: Address, character: Address, ready: boolean): Promise<Instruction> {
  return buildInstruction("session", "set_ready", { ready }, { player, session, participation: await participationPda(session, character) });
}

export async function startSessionIx(dm: TransactionSigner, session: Address): Promise<Instruction> {
  return buildInstruction(
    "session",
    "start_session",
    {},
    { dm, session, monsters: await monstersPda(session), escrow: await escrowPda(session), ...(await vrfAccounts()) },
  );
}

/** A player's in-session keys: the session key signs, the SessionTokenV2 proves it may. */
export type Actor = { signer: TransactionSigner; token: Address | null };

export async function moveIx(actor: Actor, session: Address, character: Address, layout: Address, steps: Step[]): Promise<Instruction> {
  return buildInstruction("session", "move_unit", pathArgs(steps), {
    signer: actor.signer,
    session,
    participation: await participationPda(session, character),
    monsters: await monstersPda(session),
    layout,
    sessionToken: actor.token,
  });
}

/**
 * `target` by the skill's side: a monster index for the weapon, damage and debuff skills, a player
 * slot (the caster's own for a self-cast) for heals and buffs. `targetParticipation` is that ally's
 * Participation only for a heal or buff on another player, otherwise null.
 */
export async function actIx(
  actor: Actor,
  session: Address,
  character: Address,
  skillSlot: number,
  target: number,
  targetParticipation: Address | null,
): Promise<Instruction> {
  return buildInstruction(
    "session",
    "act",
    { skillSlot, target },
    {
      signer: actor.signer,
      session,
      participation: await participationPda(session, character),
      character,
      skillSet: await skillSetPda(character),
      monsters: await monstersPda(session),
      sessionToken: actor.token,
      ...(await vrfAccounts()),
      targetParticipation,
    },
  );
}

/** `character` null: a monster turn, ended by the DM's actor. */
export async function endTurnIx(actor: Actor, session: Address, character: Address | null): Promise<Instruction> {
  return buildInstruction("session", "end_turn", {}, {
    signer: actor.signer,
    session,
    monsters: await monstersPda(session),
    participation: character ? await participationPda(session, character) : null,
    sessionToken: actor.token,
  });
}

/**
 * start_turn for the current unit (demo.test.ts `begin`): permissionless, the sender only pays the
 * fee. `player` null: a monster turn. Sent alone: a Freeze / Stun skip ends the turn inside it.
 */
export async function startTurnIx(session: Address, player: { participation: Address; character: Address } | null): Promise<Instruction> {
  return buildInstruction("session", "start_turn", {}, {
    session,
    monsters: await monstersPda(session),
    participation: player?.participation ?? null,
    character: player?.character ?? null,
    sessionAuthority: await sessionAuthorityPda(),
    config: await gameConfigPda(),
    characterProgram: PROGRAMS.character,
  });
}

export async function monsterMoveIx(dmActor: Actor, session: Address, layout: Address, monster: number, steps: Step[]): Promise<Instruction> {
  return buildInstruction("session", "monster_move", { monster, ...pathArgs(steps) }, {
    signer: dmActor.signer,
    session,
    monsters: await monstersPda(session),
    layout,
    sessionToken: dmActor.token,
  });
}

export async function monsterActIx(dmActor: Actor, session: Address, monster: number, targetSlot: number, targetCharacter: Address): Promise<Instruction> {
  return buildInstruction(
    "session",
    "monster_act",
    { monster, action: 0, target: targetSlot },
    {
      signer: dmActor.signer,
      session,
      monsters: await monstersPda(session),
      participation: await participationPda(session, targetCharacter),
      character: targetCharacter,
      sessionToken: dmActor.token,
      ...(await vrfAccounts()),
    },
  );
}

/** claim_rewards: only the character's owner, from its main wallet or its session key (+ token). */
export async function claimRewardsIx(actor: Actor, session: Address, character: Address): Promise<Instruction> {
  return buildInstruction("session", "claim_rewards", {}, {
    signer: actor.signer,
    session,
    participation: await participationPda(session, character),
    stash: await stashPda(session, character),
    character,
    bag: await bagPda(character),
    sessionAuthority: await sessionAuthorityPda(),
    config: await gameConfigPda(),
    characterProgram: PROGRAMS.character,
    sessionToken: actor.token,
  });
}

export async function claimEscrowIx(dm: TransactionSigner, session: Address, map: Address): Promise<Instruction> {
  return buildInstruction("session", "claim_escrow", {}, {
    dm,
    session,
    escrow: await escrowPda(session),
    map,
    sessionAuthority: await sessionAuthorityPda(),
    config: await gameConfigPda(),
    mapProgram: PROGRAMS.map,
  });
}

// ---------------------------------------------------------------------------------------------
// mock VRF oracle (localnet, session built with `mock-vrf`)

let mockVrf: Promise<KeyPairSigner> | null = null;
/** `Keypair.fromSeed(sha256("deadroll:mock-vrf"))`, programs/session/src/vrf.rs MOCK_VRF_IDENTITY. */
export function mockVrfSigner(): Promise<KeyPairSigner> {
  mockVrf ??= sha256(new TextEncoder().encode("deadroll:mock-vrf")).then((seed) => createKeyPairSignerFromPrivateKeyBytes(seed));
  return mockVrf;
}

/** The ally account a pending heal or buff needs in the callback (`GameSession.pendingTarget`), or null. */
export const pendingTargetOf = (s: GameSessionAcc): Address | null => (s.pendingTarget === DEFAULT_KEY ? null : s.pendingTarget);

/**
 * consume_randomness for the pending action: the callback gets the pending participation's player,
 * and `targetParticipation` = `pendingTargetOf(session)` for a heal or buff on another ally.
 */
export async function consumeIx(
  session: Address,
  pendingParticipation: Address,
  character: Address,
  seed: Uint8Array,
  targetParticipation: Address | null,
): Promise<Instruction> {
  return buildInstruction("session", "consume_randomness", { randomness: seed }, {
    vrfProgramIdentity: await mockVrfSigner(),
    session,
    monsters: await monstersPda(session),
    participation: pendingParticipation,
    character,
    skillSet: await skillSetPda(character),
    sessionAuthority: await sessionAuthorityPda(),
    config: await gameConfigPda(),
    characterProgram: PROGRAMS.character,
    targetParticipation,
  });
}

/** Roll `index` of a VRF value, exactly `deadroll_combat::VrfRoll`: sha256(seed ‖ index u32 LE). */
export async function rollAt(seed: Uint8Array, index: number): Promise<bigint> {
  const buf = new Uint8Array(36);
  buf.set(seed);
  new DataView(buf.buffer).setUint32(32, index, true);
  const h = await sha256(buf);
  return new DataView(h.buffer, h.byteOffset).getBigUint64(0, true);
}

export type SeedKind = "random" | "crit" | "miss" | "resonant";

/**
 * Localnet oracle seeds. "random" is a plain VRF stand-in; the others search seeds the way
 * tests/demo.test.ts does (natural 20, natural 1, crit + first tint roll under 1%).
 */
export async function pickSeed(kind: SeedKind): Promise<Uint8Array> {
  for (let i = 0; i < 200_000; i++) {
    const seed = crypto.getRandomValues(new Uint8Array(32));
    if (kind === "random") return seed;
    const d20 = Number((await rollAt(seed, 0)) % 20n) + 1;
    if (kind === "crit" && d20 === 20) return seed;
    if (kind === "miss" && d20 === 1) return seed;
    if (kind === "resonant" && d20 === 20 && (await rollAt(seed, 1)) % 10_000n < 100n) return seed;
  }
  throw new Error(`no ${kind} seed found`);
}

// ---------------------------------------------------------------------------------------------
// the whole party in one read

export type PartyPlayer = {
  slot: number;
  character: Address;
  participation: Address;
  part: ParticipationAcc;
  char: CharacterAcc;
};
export type Party = {
  address: Address;
  session: GameSessionAcc;
  monsters: MonsterAcc[];
  players: PartyPlayer[];
  /** Per monster (same index as `monsters`), per skill: own turn starts until usable. */
  monsterCooldowns: Uint8Array[];
  /** Per monster (same index as `monsters`): buffs and debuffs, like `Participation.mods`. */
  monsterMods: TimedModAcc[][];
  escrow: { collected: bigint; claimed: boolean } | null;
  map: MapAcc | null;
  tiles: Uint8Array | null;
};

export async function readParty(address: Address, tiles: Uint8Array | null = null, prev: Party | null = null): Promise<Party | null> {
  // With the previous read of the same party, the player, participation, character, map and layout
  // addresses are already known, so everything comes in ONE getMultipleAccounts call. If the roster
  // or the map changed (a join), the guess is wrong and the second call fetches the right accounts.
  const known = prev && prev.address === address ? prev.session : null;
  const knownChars = known ? known.core.players.slice(0, known.core.playerCount) : [];
  const guess: Address[] = known
    ? [...(await Promise.all(knownChars.map((c) => participationPda(address, c)))), ...knownChars, known.core.map, ...(tiles ? [] : [known.layout])]
    : [];
  const head = [address, await monstersPda(address), await escrowPda(address)];
  const [rawSession, rawMonsters, rawEscrow, ...guessed] = await getAccounts([...head, ...guess]);
  if (!rawSession) return null;
  const session = coders.session.decodeAccount<GameSessionAcc>("GameSession", rawSession);
  const monsterState = rawMonsters
    ? coders.session.decodeAccount<{ core: { monsters: MonsterAcc[]; count: number }; cooldowns?: Uint8Array[]; mods?: TimedModAcc[][] }>("MonsterState", rawMonsters)
    : null;
  const monsters = monsterState?.core ?? { monsters: [], count: 0 };
  const escrow = rawEscrow ? coders.session.decodeAccount<{ collected: bigint; claimed: boolean }>("Escrow", rawEscrow) : null;
  const chars = session.core.players.slice(0, session.core.playerCount);
  const parts = await Promise.all(chars.map((c) => participationPda(address, c)));
  const wanted = [...parts, ...chars, session.core.map, ...(tiles ? [] : [session.layout])];
  const extra = wanted.length === guess.length && wanted.every((a, i) => a === guess[i]) ? guessed : await getAccounts(wanted);
  const players: PartyPlayer[] = chars.flatMap((character, slot) => {
    const p = extra[slot];
    const c = extra[chars.length + slot];
    if (!p || !c) return [];
    return [
      {
        slot,
        character,
        participation: parts[slot]!,
        part: coders.session.decodeAccount<ParticipationAcc>("Participation", p),
        char: coders.character.decodeAccount<CharacterAcc>("Character", c),
      },
    ];
  });
  const rawMap = extra[chars.length * 2];
  const map = rawMap ? coders.map.decodeAccount<MapAcc>("Map", rawMap) : null;
  if (!tiles) {
    const rawLayout = extra[chars.length * 2 + 1];
    tiles = rawLayout ? coders.map.decodeAccount<{ core: { tiles: Uint8Array } }>("MapLayout", rawLayout).core.tiles : null;
  }
  const monsterCooldowns = (monsterState?.cooldowns ?? []).slice(0, monsters.count);
  const monsterMods = (monsterState?.mods ?? []).slice(0, monsters.count);
  return { address, session, monsters: monsters.monsters.slice(0, monsters.count), players, monsterCooldowns, monsterMods, escrow, map, tiles };
}

/** Parties in the lobby or in play, any DM. */
export async function openSessions(): Promise<{ address: Address; data: GameSessionAcc }[]> {
  const all = await findAccounts<GameSessionAcc>("session", "GameSession");
  return all.filter((s) => status(s.data) === "lobby" || status(s.data) === "active");
}

/** Action points the player has now: granted lazily per turn_seq (demo.test.ts `ap`). */
export function playerAp(p: ParticipationAcc, s: GameSessionAcc): number {
  return p.apSeq === s.turnSeq ? p.core.actionPoints : p.maxAp;
}
