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
import { concat, fromBase64, sha256 } from "./bytes";
import { buildInstruction, coders, type RemainingAccount } from "./programs";
import {
  bagPda,
  bestiaryPda,
  characterPda,
  escrowPda,
  gameConfigPda,
  layoutPda,
  mapPda,
  monstersPda,
  participationPda,
  profilePda,
  programIdentityPda,
  sessionAuthorityPda,
  sessionPda,
  skillExtrasPda,
  skillSetPda,
  stashPda,
  tilesetPda,
  aspectsPda,
  traitsPda,
  resonancesPda,
  monsterSkillsPda,
} from "./pda";
import { pathArgs, type Step } from "./route";
import { PRESETS, SKILL_DESCRIPTION_LEN, SKILL_NAME_LEN, type NamedSkill } from "./skillcheck";
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
/** `MonsterState.defense`: bits by aspect / state. */
export type MonsterDefenseAcc = { resist: number; immuneAspects: number; immuneStates: number };
/** `MonsterState.auras` (patch 6): a monster's aura and its kin-only flag. */
export type MonsterAuraAcc = { aura: AuraAcc; kinOnly: boolean };
/** `MonsterState.burning_cells`: Burning of `power` on whoever steps in, through `untilRound`. */
export type BurningCellAcc = { x: number; y: number; untilRound: number; power: number; source: number };
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
  /** Unused since patch 5 (allies come with the party accounts); the default key. */
  pendingTarget: Address;
  /** Patch 5: each slot's Participation (set at join), reactive skill, auras (passive, reactive). */
  participations: Address[];
  reactives: ReactiveAcc[];
  auras: AuraAcc[][];
  pendingReactive: { kind: number; bearers: number; source: number; victim: number; hpBefore: number; hpAfter: number; hpMax: number };
  /** Patch 6: a roll for monster reactives only (`kind` 0 = none). */
  pendingMonsterReactive: { kind: number; bearers: number; source: number; victim: number; hpBefore: number; hpAfter: number; hpMax: number };
  /** Patch 6: players down in a callback whose death is not finalized yet, one bit per slot. While
   * not 0 no turn starts and no action is taken: `start_turn` (anyone) or a claim finalizes them. */
  pendingDeaths: number;
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
  /** The character's maximum HP with its gear, copied at set_ready (heals stop here). */
  hpMax: number;
  /** Buffs and debuffs on this player (one per kind, any slot: read them with support.ts activeMods). */
  mods: TimedModAcc[];
  /** What the character wears, `ItemBase` bytes by `EquipSlot` (0xFF = none), copied from `Character.gear` at set_ready. */
  equipment: Uint8Array;
  /** Skills taken onto the map, SkillSet indexes: 0-3 active, 4 reactive, 5 passive (0xFF = empty). Set by set_ready. */
  skills: Uint8Array;
  /** The character's class trait tier, copied from `Character.classTier` at set_ready. */
  classTier: number;
  /** A resonance's accuracy buff (Pale Flame): +bonus to the d20 of `aspect` attacks, `turns` 0 = none. */
  accuracy: { aspect: Enum; turns: number; bonus: number };
  /** Patch 5: the map skills of `skills`, slot by slot, as the rules see them (copied at set_ready). */
  mapSkills: MapSkillAcc[];
};
/** session MapSkillData: a skill taken onto the map (blocks, derived values, the patch 5 ext). */
export type MapSkillAcc = {
  blocks: SkillRecordAcc["blocks"];
  sharesBp: number[];
  powerBp: number;
  cooldown: number;
  ext: { range: Enum; timing: Enum; hpBelowPct: number };
  apCost: number;
};
/** session AuraData (docs/SKILL_BUILDER.md 3.5): `turns` 255 = permanent (a passive), 0 = none. */
export type AuraAcc = {
  kind: Enum;
  aspect: Enum;
  powerBp: number;
  radius: number;
  turns: number;
  hostile: boolean;
  passive: boolean;
  selfOnly: boolean;
};
/** session ReactiveData: a player's reactive skill on the map, `skillSlot` 0xFF = none. */
export type ReactiveAcc = { skillSlot: number; skill: MapSkillAcc; cooldownLeft: number };
/** programs/character SkillExtras: the patch 5 part of every skill, by SkillSet index (zero-copy, raw bytes). */
export type SkillExtraAcc = { range: number; timing: number; hpBelowPct: number; apCost: number; description: string };
/** programs/character `Gear`: soulbound starter items and what is worn, by `EquipSlot`. */
export type GearAcc = {
  items: { base: Enum; level: number }[];
  itemCount: number;
  equipped: { source: Enum; index: number; asset: Address }[];
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
    /** Race trait tier, 1 best .. 14. */
    predispositionTier: number;
    hpMax: number;
    alive: boolean;
    inSession: Address | null;
    appearance: { spriteId: number; materialRamp: number; accentRamp: number; elementRamp: number }[];
    stateVersion: number;
  };
  name: Uint8Array;
  unspentPoints: number;
  nftFrozen: boolean;
  gear: GearAcc;
  /** Class trait tier, 1 best .. 14 (docs/CHARACTER.md 6.3). */
  classTier: number;
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

/** Bytes of one SkillExtra record (programs/character skill_extras.rs). */
const SKILL_EXTRA_SIZE = 4 + SKILL_DESCRIPTION_LEN;
/** Offset of the first record: discriminator, version, bump, character. */
const SKILL_EXTRAS_START = 8 + 2 + 32;

/** The zero-copy `SkillExtras` account: one record per SkillSet index, raw bytes. */
export function decodeSkillExtras(raw: Uint8Array, count: number): SkillExtraAcc[] {
  const text = new TextDecoder();
  return Array.from({ length: count }, (_, i) => {
    const at = SKILL_EXTRAS_START + i * SKILL_EXTRA_SIZE;
    const r = raw.subarray(at, at + SKILL_EXTRA_SIZE);
    const d = r.subarray(4);
    const len = d.indexOf(0) === -1 ? d.length : d.indexOf(0);
    return { range: r[0]!, timing: r[1]!, hpBelowPct: r[2]!, apCost: r[3]!, description: text.decode(d.subarray(0, len)) };
  });
}

/** A character's `SkillExtras` (the first `count` skills), or null before it exists. */
export async function skillExtrasOf(character: Address, count: number): Promise<SkillExtraAcc[] | null> {
  const [raw] = await getAccounts([await skillExtrasPda(character)]);
  return raw ? decodeSkillExtras(raw, count) : null;
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
 * create_profile if missing, then create_character with a fresh Core NFT asset keypair. The class
 * starter items come with the character (`Character.gear`), worn.
 */
export async function createCharacterIxs(
  owner: TransactionSigner,
  c: NewCharacter,
): Promise<{ profileIx: Instruction | null; ix: Instruction; character: Address; asset: KeyPairSigner }> {
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
      appearance: characterAppearance(c.race, c.predisposition),
      skills: (c.skills ?? SKILL_PRESETS[c.class]).map((s) => ({ blocks: s.blocks, name: textBytes(s.name, SKILL_NAME_LEN), ext: s.ext })),
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
    skillExtras: await skillExtrasPda(character),
  });
  return { profileIx: prof ? null : createProfileIx(owner, profile), ix, character, asset };
}

/**
 * The descriptions of a new character's skills (set_skill_description, once each): sent after
 * create_character, `perTx` instructions to a transaction (six of 200 bytes do not fit one).
 */
export async function skillDescriptionIxs(owner: TransactionSigner, character: Address, skills: readonly NamedSkill[], perTx = 3): Promise<Instruction[][]> {
  const accounts = {
    owner,
    character,
    skillSet: await skillSetPda(character),
    skillExtras: await skillExtrasPda(character),
  };
  const ixs = skills
    .map((s, i) => [s.description, i] as const)
    .filter(([d]) => d.length > 0)
    .map(([d, i]) => buildInstruction("character", "set_skill_description", { skillIndex: i, description: textBytes(d, SKILL_DESCRIPTION_LEN) }, accounts));
  const out: Instruction[][] = [];
  for (let i = 0; i < ixs.length; i += perTx) out.push(ixs.slice(i, i + perTx));
  return out;
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
    // The builder rules read the registry bestiary and monster skills (roles, pools, slots).
    buildInstruction("map", "finalize_map", { secretsRoot: new Uint8Array(32) }, { ...edit, bestiary: await bestiaryPda(), monsterSkills: await monsterSkillsPda() }),
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
      aspects: await aspectsPda(),
      traits: await traitsPda(),
      resonances: await resonancesPda(),
      monsterSkills: await monsterSkillsPda(),
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
      systemProgram: SYSTEM_PROGRAM,
    },
  );
}

/**
 * Ready (true): takes `selection` onto the map (SkillSet indexes, 4 active + 1 reactive + 1 passive,
 * NO_SKILL = empty; skillcheck.ts checkMapSelection) and locks the character, which freezes its bag
 * and gear for the party. Unready (false) unlocks it; the selection is ignored.
 */
export async function setReadyIx(
  player: TransactionSigner,
  session: Address,
  character: Address,
  ready: boolean,
  selection: readonly number[],
): Promise<Instruction> {
  return buildInstruction("session", "set_ready", { ready, selection: Uint8Array.from(selection) }, {
    player,
    session,
    participation: await participationPda(session, character),
    character,
    skillSet: await skillSetPda(character),
    sessionAuthority: await sessionAuthorityPda(),
    config: await gameConfigPda(),
    characterProgram: PROGRAMS.character,
    skillExtras: await skillExtrasPda(character),
  });
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
    // Patch 6: stepping next to a monster can trigger its reactive skill (a VRF roll).
    ...(await vrfAccounts()),
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
  layout: Address,
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
      bag: await bagPda(character),
      monsters: await monstersPda(session),
      sessionToken: actor.token,
      ...(await vrfAccounts()),
      targetParticipation,
      layout,
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
 * The accounts that finalize the pending deaths (`GameSession.pending_deaths`): each queued slot's
 * Character, SkillSet and Bag, writable, in slot order. The programs check them and refuse
 * everything else while the queue is not empty; the app only supplies them.
 */
export async function deathAccountsOf(s: GameSessionAcc): Promise<RemainingAccount[]> {
  const out: RemainingAccount[] = [];
  for (let slot = 0; slot < s.core.playerCount; slot++) {
    if (!(s.pendingDeaths & (1 << slot))) continue;
    const character = s.core.players[slot]!;
    out.push({ address: character, writable: true }, { address: await skillSetPda(character), writable: true }, { address: await bagPda(character), writable: true });
  }
  return out;
}

/** ComputeBudget program: `SetComputeUnitLimit(units)`. */
const COMPUTE_BUDGET = "ComputeBudget111111111111111111111111111111" as Address;
/** Compute limit of a transaction that may finalize deaths (`character.mark_dead` each, about
 * 45k CU; four players at most). */
export const FINALIZE_CU = 600_000;
export function computeLimitIx(units: number): Instruction {
  const data = new Uint8Array(5);
  data[0] = 2;
  new DataView(data.buffer).setUint32(1, units, true);
  return { programAddress: COMPUTE_BUDGET, data };
}
/** `ix` with the compute limit in front when it finalizes deaths. */
export const withDeaths = (deaths: readonly RemainingAccount[], ix: Instruction): Instruction[] => (deaths.length ? [computeLimitIx(FINALIZE_CU), ix] : [ix]);

/**
 * start_turn for the current unit (demo.test.ts `begin`): permissionless, the sender only pays the
 * fee. `player` null: a monster turn. Sent alone: a Freeze / Stun skip ends the turn inside it.
 * `deaths` (patch 6, `deathAccountsOf`): it finalizes them first, also when the turn already started.
 */
export async function startTurnIx(
  session: Address,
  player: { participation: Address; character: Address } | null,
  payer: TransactionSigner,
  deaths: readonly RemainingAccount[] = [],
): Promise<Instruction> {
  return buildInstruction("session", "start_turn", {}, {
    session,
    monsters: await monstersPda(session),
    participation: player?.participation ?? null,
    character: player?.character ?? null,
    sessionAuthority: await sessionAuthorityPda(),
    config: await gameConfigPda(),
    characterProgram: PROGRAMS.character,
    // Burned if over-time damage kills the player.
    skillSet: player ? await skillSetPda(player.character) : null,
    bag: player ? await bagPda(player.character) : null,
    // Pays the VRF fee when over-time damage triggers a reactive skill (patch 5).
    payer,
    ...(await vrfAccounts()),
  }, deaths);
}

export async function monsterMoveIx(dmActor: Actor, session: Address, layout: Address, monster: number, steps: Step[]): Promise<Instruction> {
  return buildInstruction("session", "monster_move", { monster, ...pathArgs(steps) }, {
    signer: dmActor.signer,
    session,
    monsters: await monstersPda(session),
    layout,
    sessionToken: dmActor.token,
    // A monster stepping next to a player can trigger its reactive skill (a VRF roll).
    ...(await vrfAccounts()),
  });
}

/**
 * A monster's action: `action` 0 = its natural attack, `1 + slot` = the active skill in that monster
 * slot. `target`: a player slot (natural attack, damage, debuff), or a monster index (heal, buff).
 * `character`: the target player's character, or for a heal or buff on a monster the lowest living
 * slot's (the callback's participation).
 */
export async function monsterActIx(
  dmActor: Actor,
  session: Address,
  monster: number,
  action: number,
  target: number,
  character: Address,
  layout: Address,
): Promise<Instruction> {
  return buildInstruction(
    "session",
    "monster_act",
    { monster, action, target },
    {
      signer: dmActor.signer,
      session,
      monsters: await monstersPda(session),
      participation: await participationPda(session, character),
      character,
      skillSet: await skillSetPda(character),
      bag: await bagPda(character),
      sessionToken: dmActor.token,
      ...(await vrfAccounts()),
      layout,
    },
  );
}

/** claim_rewards: only the character's owner, from its main wallet or its session key (+ token).
 * `deaths`: the pending deaths it finalizes first (patch 6). */
export async function claimRewardsIx(actor: Actor, session: Address, character: Address, deaths: readonly RemainingAccount[] = []): Promise<Instruction> {
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
  }, deaths);
}

/** claim_escrow: the DM's main wallet. `deaths`: the pending deaths it finalizes first (patch 6). */
export async function claimEscrowIx(dm: TransactionSigner, session: Address, map: Address, deaths: readonly RemainingAccount[] = []): Promise<Instruction> {
  return buildInstruction("session", "claim_escrow", {}, {
    dm,
    session,
    escrow: await escrowPda(session),
    map,
    sessionAuthority: await sessionAuthorityPda(),
    config: await gameConfigPda(),
    mapProgram: PROGRAMS.map,
    characterProgram: PROGRAMS.character,
  }, deaths);
}

// ---------------------------------------------------------------------------------------------
// mock VRF oracle (localnet, session built with `mock-vrf`)

let mockVrf: Promise<KeyPairSigner> | null = null;
/** `Keypair.fromSeed(sha256("deadroll:mock-vrf"))`, programs/session/src/vrf.rs MOCK_VRF_IDENTITY. */
export function mockVrfSigner(): Promise<KeyPairSigner> {
  mockVrf ??= sha256(new TextEncoder().encode("deadroll:mock-vrf")).then((seed) => createKeyPairSignerFromPrivateKeyBytes(seed));
  return mockVrf;
}

/**
 * The party accounts every callback gets after its fixed list (programs/session party.rs): each
 * living player but the pending participation's, in slot order, its Participation (writable) and
 * its Character. A death in the callback only queues (patch 6), so no death accounts ride along.
 */
export function partyAccountsOf(s: GameSessionAcc, pendingParticipation: Address): RemainingAccount[] {
  const out: RemainingAccount[] = [];
  for (let slot = 0; slot < s.core.playerCount; slot++) {
    const alive = (s.aliveMask & (1 << slot)) !== 0;
    const participation = s.participations[slot]!;
    if (!alive || participation === pendingParticipation) continue;
    out.push({ address: participation, writable: true }, { address: s.core.players[slot]!, writable: false });
  }
  return out;
}

/** The body of a registry table (as far as it is written), or null. */
async function registryTable(pdaOf: () => Promise<Address>): Promise<Uint8Array | null> {
  const [raw] = await getAccounts([await pdaOf()]);
  if (!raw || raw.length < 16) return null;
  const len = new DataView(raw.buffer, raw.byteOffset).getUint32(12, true);
  return raw.slice(16, 16 + len);
}

/** Patch 6: the registry `bestiary` (48-byte records by id) and `monster_skills` (32-byte records)
 * bodies, read straight from the chain: the map builder's pools and the skill names' indices. */
export const registryBestiary = () => registryTable(bestiaryPda);
export const registryMonsterSkills = () => registryTable(monsterSkillsPda);

/**
 * consume_randomness for the pending action: the callback gets the pending participation's player,
 * the session's map layout (`GameSession.layout`: a resonance's knockback and burning cells, a
 * line's walls) and the other living players (`partyAccountsOf`: reactive skills, lines, buffs on
 * an ally).
 */
export async function consumeIx(
  session: Address,
  pendingParticipation: Address,
  character: Address,
  seed: Uint8Array,
  layout: Address,
  party: readonly RemainingAccount[],
): Promise<Instruction> {
  return buildInstruction("session", "consume_randomness", { randomness: seed }, {
    vrfProgramIdentity: await mockVrfSigner(),
    session,
    monsters: await monstersPda(session),
    participation: pendingParticipation,
    character,
    skillSet: await skillSetPda(character),
    bag: await bagPda(character),
    sessionAuthority: await sessionAuthorityPda(),
    config: await gameConfigPda(),
    characterProgram: PROGRAMS.character,
    // Since patch 5 an ally comes with the party accounts.
    targetParticipation: null,
    layout,
  }, party);
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

/** First four bytes are `n` little endian. Same constants as tests/demo.test.ts. */
function fixedSeed(n: number): Uint8Array {
  const seed = new Uint8Array(32);
  new DataView(seed.buffer).setUint32(0, n, true);
  return seed;
}

/**
 * Localnet oracle seeds. "random" is a plain VRF stand-in. The others are fixed: natural 20,
 * natural 1, and for the demo mage's Frost Burn a crit whose Fire base gets its Cold tint (roll 2,
 * a d100 under 5) while both staff (Blunt) tints fail (rolls 1 and 3): Pale Flame.
 */
export async function pickSeed(kind: SeedKind): Promise<Uint8Array> {
  if (kind === "crit") return fixedSeed(9);
  if (kind === "miss") return fixedSeed(17);
  if (kind === "resonant") return fixedSeed(128);
  return crypto.getRandomValues(new Uint8Array(32));
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
  /** Per monster (same index as `monsters`), per skill slot (`0..5` active, `5` reactive, `6`
   * passive): own turn starts until usable (`MonsterState.skill_cooldowns`, patch 6). */
  monsterCooldowns: Uint8Array[];
  /** Per monster: its bestiary record (`MonsterState.records`, copied at open_session). */
  monsterRecords: Uint8Array[];
  /** Per monster, per skill slot: the skill's 16-byte session copy (`skill_table`), null = empty. */
  monsterSkills: (Uint8Array | null)[][];
  /** Per monster: its passive skill's aura, then its reactive skill's. */
  monsterAuras: MonsterAuraAcc[][];
  /** Per monster (same index as `monsters`): buffs and debuffs, like `Participation.mods`. */
  monsterMods: TimedModAcc[][];
  /** Per monster: its registry defenses (`MonsterState.defense`, copied at open_session). */
  monsterDefense: MonsterDefenseAcc[];
  /** The resonance tables the session copied (aspects, traits, resonances bodies), or null for a
   * session opened before patch 4. */
  resonanceTables: Uint8Array | null;
  /** Cells a resonance set on fire. */
  burningCells: BurningCellAcc[];
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
    ? coders.session.decodeAccount<{
        core: { monsters: MonsterAcc[]; count: number };
        cooldowns?: Uint8Array[];
        records?: Uint8Array[];
        skillTable?: Uint8Array[];
        skillCount?: number;
        skills?: Uint8Array[];
        skillCooldowns?: Uint8Array[];
        auras?: MonsterAuraAcc[][];
        mods?: TimedModAcc[][];
        defense?: MonsterDefenseAcc[];
        resAspects?: Uint8Array;
        resTraits?: Uint8Array;
        resPairs?: Uint8Array;
        burningCells?: BurningCellAcc[];
      }>("MonsterState", rawMonsters)
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
  const monsterCooldowns = (monsterState?.skillCooldowns ?? []).slice(0, monsters.count).map((c) => Uint8Array.from(c));
  const monsterRecords = (monsterState?.records ?? []).slice(0, monsters.count).map((r) => Uint8Array.from(r));
  const table = (monsterState?.skillTable ?? []).slice(0, monsterState?.skillCount ?? 0);
  const monsterSkills = (monsterState?.skills ?? [])
    .slice(0, monsters.count)
    .map((slots) => Array.from(slots).map((t) => (t === 0xff || !table[t] ? null : Uint8Array.from(table[t]!))));
  const monsterAuras = (monsterState?.auras ?? []).slice(0, monsters.count);
  const monsterMods = (monsterState?.mods ?? []).slice(0, monsters.count);
  const monsterDefense = (monsterState?.defense ?? []).slice(0, monsters.count);
  const resonanceTables =
    monsterState?.resAspects && monsterState.resTraits && monsterState.resPairs
      ? concat(Uint8Array.from(monsterState.resAspects), Uint8Array.from(monsterState.resTraits), Uint8Array.from(monsterState.resPairs))
      : null;
  const burningCells = (monsterState?.burningCells ?? []).filter((c) => c.power > 0);
  return {
    address,
    session,
    monsters: monsters.monsters.slice(0, monsters.count),
    players,
    monsterCooldowns,
    monsterRecords,
    monsterSkills,
    monsterAuras,
    monsterMods,
    monsterDefense,
    resonanceTables,
    burningCells,
    escrow,
    map,
    tiles,
  };
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

/**
 * character `spend_stat_points`: the level-up points chosen per stat (Strength, Dexterity,
 * Intelligence, Endurance), all in one transaction. The owner's main wallet only.
 */
export function spendStatPointsIx(owner: TransactionSigner, character: Address, points: readonly number[]): Instruction {
  return buildInstruction("character", "spend_stat_points", { points: [...points] }, { owner, character });
}
