// The demo script as instructions and reads, mirroring tests/demo.test.ts (class Demo) call for
// call: same instructions, same account lists, same seeds. Screens call these; nothing here
// touches React. Instructions and accounts come from the generated sdk (@deadroll/sdk).
import {
  AccountRole,
  createKeyPairSignerFromPrivateKeyBytes,
  generateKeyPairSigner,
  getAddressEncoder,
  isSome,
  type Address,
  type Base64EncodedDataResponse,
  type Instruction,
  type KeyPairSigner,
  type Option,
  type ReadonlyUint8Array,
  type TransactionSigner,
} from "@solana/kit";
import {
  BAG_DISCRIMINATOR,
  CHARACTER_DISCRIMINATOR,
  PLAYER_PROFILE_DISCRIMINATOR,
  SKILL_EXTRAS_DISCRIMINATOR,
  SKILL_SET_DISCRIMINATOR,
  getBagDecoder,
  getCharacterDecoder,
  getCreateCharacterInstruction,
  getCreateProfileInstruction,
  getFreezeTombstoneInstruction,
  getPlayerProfileDecoder,
  getSetSkillDescriptionInstruction,
  getSkillExtrasDecoder,
  getSkillSetDecoder,
  getSpendStatPointsInstruction,
  type Character,
  type Gear,
  type SkillRecord,
  type SkillSet,
} from "@deadroll/sdk/character";
import {
  MAP_DISCRIMINATOR,
  MAP_LAYOUT_DISCRIMINATOR,
  getCreateMapInstruction,
  getFinalizeMapInstruction,
  getMapDecoder,
  getMapLayoutDecoder,
  getMapLayoutSize,
  getMapSize,
  getWritePointsInstruction,
  getWriteLayoutInstruction,
  getWriteSpawnsInstruction,
  MapStatus,
  PointKind,
  type Map as MapAccount,
} from "@deadroll/sdk/map";
import {
  ESCROW_DISCRIMINATOR,
  GAME_SESSION_DISCRIMINATOR,
  MONSTER_STATE_DISCRIMINATOR,
  PARTICIPATION_DISCRIMINATOR,
  REWARD_CACHE_DISCRIMINATOR,
  SessionStatus,
  getActInstruction,
  getClaimAfterTimeoutInstruction,
  getCloseCacheInstruction,
  getLeavePartyInstruction,
  getConsumeRandomnessInstruction,
  getEndTurnInstruction,
  getEscrowDecoder,
  getGameSessionDecoder,
  getJoinSessionInstruction,
  getMonsterActInstruction,
  getMonsterStateDecoder,
  getMovePathInstruction,
  getOpenChestInstruction,
  getOpenSessionInstruction,
  getParticipationDecoder,
  getRewardCacheDecoder,
  getParticipationSize,
  getRewardCacheSize,
  getSetReadyInstruction,
  getSetSessionKeyInstruction,
  getSettlePartyInstruction,
  getStartSessionInstruction,
  getStartTurnInstruction,
  type ActiveState,
  type AuraData,
  type BagSlotArgs,
  type BurningCellData,
  type GameSession,
  type MapSkillData,
  type Monster,
  type MonsterAuraData,
  type MonsterDefenseData,
  type Participation,
  type ReactiveData,
  type ResistModData,
  type TimedModData,
} from "@deadroll/sdk/session";
import {
  BUILDING_DISCRIMINATOR,
  CITY_DISCRIMINATOR,
  QUEST_DISCRIMINATOR,
  TAVERN_BOARD_DISCRIMINATOR,
  getBuildingDecoder,
  getCityDecoder,
  getQuestDecoder,
  getTavernBoardDecoder,
} from "@deadroll/sdk/city";
import { COMPUTE_BUDGET_PROGRAM } from "@deadroll/sdk";
import { MPL_CORE_PROGRAM, PROGRAMS, SYSTEM_PROGRAM, VRF } from "../config";
import { client } from "../wallet/client";
import { characterAppearance } from "./appearance";
import { concat, fromBase64, sha256 } from "./bytes";
import { packBySize } from "./send";
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
  rewardCachePda,
  tilesetPda,
  aspectsPda,
  traitsPda,
  resonancesPda,
  monsterSkillsPda,
  rewardsPda,
  triplesPda,
} from "./pda";
import type { GridPoint } from "../components/grid/types";
import type { Chest } from "./loot";
import type { Step } from "./route";
import { PRESETS, SKILL_DESCRIPTION_LEN, SKILL_NAME_LEN, type NamedSkill } from "./skillcheck";
import { Aspect, Class, MAP_CELLS, MAP_MAX, Race, Tile, cellByte, type Cell, type Spawn } from "./types";

// ---------------------------------------------------------------------------------------------
// decoded account shapes: the generated sdk types (numbers for enums, `Option` for Rust options)

export type ActiveStateAcc = ActiveState;
/** A buff or debuff slot (session TimedModData): active while `turns > 0`. One per kind: find by
 * `kind`. A Resistance (patch 9) carries its aspect weights, merged in by [`withResist`]. */
export type TimedModAcc = TimedModData & { weightsPct?: ArrayLike<number> };
/** session ResistModData (patch 9): the Resistance slot and its aspect weights. */
export type ResistModAcc = ResistModData;

/** A unit's stored mods and its Resistance slot (`Participation.resist_mod`,
 * `MonsterState.resist_mods`, the `resistMod` of the events) as one list. */
export function withResist(mods: readonly TimedModAcc[] | undefined, resist: ResistModAcc | undefined): TimedModAcc[] {
  const out = [...(mods ?? [])];
  if (resist && resist.data.turns > 0) out.push({ ...resist.data, weightsPct: Array.from(resist.weightsPct) });
  return out;
}
export type MonsterAcc = Monster;
/** `MonsterState.defense`: bits by aspect / state. */
export type MonsterDefenseAcc = MonsterDefenseData;
/** `MonsterState.auras` (patch 6): a monster's aura and its kin-only flag. */
export type MonsterAuraAcc = MonsterAuraData;
/** `MonsterState.burning_cells`: Burning of `power` on whoever steps in, through `untilRound`. */
export type BurningCellAcc = BurningCellData;
export type GameSessionAcc = GameSession;
export type ParticipationAcc = Participation;
/** session MapSkillData: a skill taken onto the map (blocks, derived values, the patch 5 ext). */
export type MapSkillAcc = MapSkillData;
/** session AuraData (docs/SKILL_BUILDER.md 3.5): `turns` 255 = permanent (a passive), 0 = none. A
 * Resistance aura carries its aspect weights (`GameSession.aura_weights`, merged in by `readParty`). */
export type AuraAcc = AuraData & { weightsPct?: ArrayLike<number> };
/** session ReactiveData: a player's reactive skill on the map, `skillSlot` 0xFF = none. */
export type ReactiveAcc = ReactiveData;
/** programs/character SkillExtras: the patch 5 part of every skill, by SkillSet index. */
export type SkillExtraAcc = { range: number; timing: number; hpBelowPct: number; apCost: number; description: string };
/** programs/character `Gear`: soulbound starter items and what is worn, by `EquipSlot`. */
export type GearAcc = Gear;
export type CharacterAcc = Character;
/** programs/character SkillRecord: crates/skillcheck SkillBlocks + derived values. */
export type SkillRecordAcc = SkillRecord;
export type SkillSetAcc = SkillSet;
export type MapAcc = MapAccount;
export type EscrowAcc = ReturnType<ReturnType<typeof getEscrowDecoder>["decode"]>;

/** A generated enum object (`{ Human: 0, ForestElf: 1, 0: "Human", 1: "ForestElf" }`). */
export type EnumObject = Readonly<Record<string | number, string | number>>;
const lowerFirst = (s: string): string => s.charAt(0).toLowerCase() + s.slice(1);
/** Variant names of an enum in on-chain order, lower camel ("mainHand"). */
export function enumKeys(e: EnumObject): string[] {
  return Object.entries(e)
    .filter((kv): kv is [string, number] => typeof kv[1] === "number")
    .sort((a, b) => a[1] - b[1])
    .map(([k]) => lowerFirst(k));
}
/** The variant name of `value`, lower camel ("enemyAdjacent"), or "" when out of range. */
export const variantKey = (e: EnumObject, value: number): string => lowerFirst(String(e[value] ?? ""));
/** Enum variant as a label: EnemyAdjacent -> "Enemy adjacent". */
export function variantLabel(e: EnumObject, value: number): string {
  const words = variantKey(e, value).replace(/([a-z0-9])([A-Z])/g, "$1 $2").toLowerCase();
  return words.charAt(0).toUpperCase() + words.slice(1);
}
/** A Rust `Option` field as the value or null. */
export const optional = <T>(o: Option<T>): T | null => (isSome(o) ? o.value : null);

export const MONSTER_BASE = 4;
export const BASE_ATTACK = 0xff;
export type SessionStatusKey = "lobby" | "active" | "victory" | "wiped" | "fled" | "abandoned";
export const status = (s: GameSessionAcc) => variantKey(SessionStatus, s.core.status) as SessionStatusKey;
export const mapStatus = (m: MapAcc) => variantKey(MapStatus, m.core.status) as "draft" | "finalized" | "inSession" | "finished";
/** Modifier variants in on-chain order (`ModKindData`); `expired` bit i is `1 << i` (deadroll_combat::support::mod_bit). */
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
export function bytesText(b: ReadonlyUint8Array): string {
  const end = b.indexOf(0);
  return new TextDecoder().decode(end < 0 ? b : b.subarray(0, end));
}

// ---------------------------------------------------------------------------------------------
// reads

/** The accounts the app reads: program, discriminator and generated decoder. */
const ACCOUNTS = {
  PlayerProfile: { program: "character", disc: PLAYER_PROFILE_DISCRIMINATOR, decoder: getPlayerProfileDecoder() },
  Character: { program: "character", disc: CHARACTER_DISCRIMINATOR, decoder: getCharacterDecoder() },
  SkillSet: { program: "character", disc: SKILL_SET_DISCRIMINATOR, decoder: getSkillSetDecoder() },
  SkillExtras: { program: "character", disc: SKILL_EXTRAS_DISCRIMINATOR, decoder: getSkillExtrasDecoder() },
  Map: { program: "map", disc: MAP_DISCRIMINATOR, decoder: getMapDecoder() },
  MapLayout: { program: "map", disc: MAP_LAYOUT_DISCRIMINATOR, decoder: getMapLayoutDecoder() },
  GameSession: { program: "session", disc: GAME_SESSION_DISCRIMINATOR, decoder: getGameSessionDecoder() },
  Participation: { program: "session", disc: PARTICIPATION_DISCRIMINATOR, decoder: getParticipationDecoder() },
  MonsterState: { program: "session", disc: MONSTER_STATE_DISCRIMINATOR, decoder: getMonsterStateDecoder() },
  Escrow: { program: "session", disc: ESCROW_DISCRIMINATOR, decoder: getEscrowDecoder() },
  // Patch 11: the bag in slots and the reward cache after a party.
  Bag: { program: "character", disc: BAG_DISCRIMINATOR, decoder: getBagDecoder() },
  RewardCache: { program: "session", disc: REWARD_CACHE_DISCRIMINATOR, decoder: getRewardCacheDecoder() },
  // The city and the Tavern board (agents/CONTRACTS.md 25).
  City: { program: "city", disc: CITY_DISCRIMINATOR, decoder: getCityDecoder() },
  Building: { program: "city", disc: BUILDING_DISCRIMINATOR, decoder: getBuildingDecoder() },
  TavernBoard: { program: "city", disc: TAVERN_BOARD_DISCRIMINATOR, decoder: getTavernBoardDecoder() },
  Quest: { program: "city", disc: QUEST_DISCRIMINATOR, decoder: getQuestDecoder() },
} as const;
export type AccountName = keyof typeof ACCOUNTS;
export type AccountOf<K extends AccountName> = ReturnType<(typeof ACCOUNTS)[K]["decoder"]["decode"]>;

/** `Map` accounts created before `Map.biome` (version 1) are one byte shorter. */
const MAP_V1_SIZE = getMapSize() - 1;

/** Decodes an account by name; throws on another account's discriminator. A `Map` from before
 * `biome` gets its missing byte as zero: it reads as `Biome.Caves`, like `Map::decode_padded`. */
export function decodeAccount<K extends AccountName>(name: K, raw: ReadonlyUint8Array): AccountOf<K> {
  const { disc, decoder } = ACCOUNTS[name];
  if (raw.length < 8 || disc.some((b, i) => raw[i] !== b)) throw new Error(`not a ${name} account`);
  if (name === "Map" && raw.length >= MAP_V1_SIZE && raw.length < getMapSize()) {
    const padded = new Uint8Array(getMapSize());
    padded.set(raw);
    return decoder.decode(padded) as AccountOf<K>;
  }
  return decoder.decode(raw) as AccountOf<K>;
}

export async function getAccounts(addresses: Address[]): Promise<(Uint8Array | null)[]> {
  if (addresses.length === 0) return [];
  const { value } = await client.rpc.getMultipleAccounts(addresses, { encoding: "base64", commitment: "confirmed" }).send();
  return value.map((v) => (v ? fromBase64((v.data as Base64EncodedDataResponse)[0]) : null));
}

export async function getAccount<K extends AccountName>(name: K, address: Address): Promise<AccountOf<K> | null> {
  const [raw] = await getAccounts([address]);
  return raw ? decodeAccount(name, raw) : null;
}

/** The `SkillExtras` account: one record per SkillSet index, the first `count`. */
export function decodeSkillExtras(raw: ReadonlyUint8Array, count: number): SkillExtraAcc[] {
  return decodeAccount("SkillExtras", raw)
    .extras.slice(0, count)
    .map((r) => ({ range: r.range, timing: r.timing, hpBelowPct: r.hpBelowPct, apCost: r.apCost, description: bytesText(r.description.slice(0, SKILL_DESCRIPTION_LEN)) }));
}

/** A character's `SkillExtras` (the first `count` skills), or null before it exists. */
export async function skillExtrasOf(character: Address, count: number): Promise<SkillExtraAcc[] | null> {
  const [raw] = await getAccounts([await skillExtrasPda(character)]);
  return raw ? decodeSkillExtras(raw, count) : null;
}

const addressEncoder = getAddressEncoder();
/** Accounts of `name` (by discriminator) that match the given memcmp. */
export async function findAccounts<K extends AccountName>(
  name: K,
  memcmp: { offset: number; bytes: Uint8Array }[] = [],
): Promise<{ address: Address; data: AccountOf<K> }[]> {
  const { program, disc } = ACCOUNTS[name];
  const filters = [
    { memcmp: { offset: 0n, bytes: base64(disc), encoding: "base64" as const } },
    ...memcmp.map((m) => ({ memcmp: { offset: BigInt(m.offset), bytes: base64(m.bytes), encoding: "base64" as const } })),
  ];
  const res = await client.rpc
    .getProgramAccounts(PROGRAMS[program], { encoding: "base64", commitment: "confirmed", filters } as never)
    .send();
  const list = res as unknown as { pubkey: Address; account: { data: Base64EncodedDataResponse } }[];
  const out: { address: Address; data: AccountOf<K> }[] = [];
  for (const { pubkey, account } of list) {
    try {
      out.push({ address: pubkey, data: decodeAccount(name, fromBase64(account.data[0])) });
    } catch {
      // older layout: skip
    }
  }
  return out;
}

function base64(bytes: ReadonlyUint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
}

/** An account after the instruction's own list (`ctx.remaining_accounts`). */
export type RemainingAccount = { address: Address; writable: boolean };

/** `ix` with `remaining` appended to its accounts. */
export function withRemaining(ix: Instruction, remaining: readonly RemainingAccount[]): Instruction {
  if (remaining.length === 0) return ix;
  const extra = remaining.map((r) => ({ address: r.address, role: r.writable ? AccountRole.WRITABLE : AccountRole.READONLY }));
  return { ...ix, accounts: [...(ix.accounts ?? []), ...extra] };
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
  return getCreateProfileInstruction({ owner, profile, systemProgram: SYSTEM_PROGRAM });
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
  const prof = await getAccount("PlayerProfile", profile);
  const index = prof?.characterCount ?? 0;
  const character = await characterPda(owner.address, index);
  const asset = await generateKeyPairSigner();
  const ix = getCreateCharacterInstruction({
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
    name: textBytes(c.name, 32),
    race: c.race,
    class: c.class,
    predisposition: c.predisposition,
    stats: c.stats,
    appearance: characterAppearance(c.race, c.predisposition),
    skills: (c.skills ?? SKILL_PRESETS[c.class]).map((s) => ({ blocks: s.blocks, name: textBytes(s.name, SKILL_NAME_LEN), ext: s.ext })),
  });
  return { profileIx: prof ? null : createProfileIx(owner, profile), ix, character, asset };
}

/**
 * The descriptions of a new character's skills (set_skill_description, once each), after
 * create_character; six of 200 bytes do not fit one transaction, `characterSteps` packs them.
 */
export async function skillDescriptionIxs(owner: TransactionSigner, character: Address, skills: readonly NamedSkill[]): Promise<Instruction[]> {
  const accounts = {
    owner,
    character,
    skillSet: await skillSetPda(character),
    skillExtras: await skillExtrasPda(character),
  };
  return skills
    .map((s, i) => [s.description, i] as const)
    .filter(([d]) => d.length > 0)
    .map(([d, i]) => getSetSkillDescriptionInstruction({ ...accounts, skillIndex: i, description: textBytes(d, SKILL_DESCRIPTION_LEN) }));
}

/**
 * Creating a character as one user action: create_profile if missing, create_character, then the
 * skill descriptions, packed in order into as few transactions as fit (one wallet confirmation
 * for all of them, `sendBatch`).
 */
export async function characterSteps(owner: TransactionSigner, c: NewCharacter): Promise<{ steps: Instruction[][]; character: Address; asset: KeyPairSigner }> {
  const made = await createCharacterIxs(owner, c);
  const descriptions = await skillDescriptionIxs(owner, made.character, c.skills ?? SKILL_PRESETS[c.class]);
  const groups = [[...(made.profileIx ? [made.profileIx] : []), made.ix], ...descriptions.map((ix) => [ix])];
  return { steps: packBySize(owner, groups), character: made.character, asset: made.asset };
}

/** All characters of `owner`, by profile count (indices 0..count). */
export async function myCharacters(owner: Address): Promise<{ address: Address; data: CharacterAcc }[]> {
  const prof = await getAccount("PlayerProfile", await profilePda(owner));
  const count = prof?.characterCount ?? 0;
  const addrs = await Promise.all(Array.from({ length: count }, (_, i) => characterPda(owner, i)));
  const raws = await getAccounts(addrs);
  return addrs.flatMap((address, i) => {
    const raw = raws[i];
    return raw ? [{ address, data: decodeAccount("Character", raw) }] : [];
  });
}

export function freezeTombstoneIx(character: Address, asset: Address, payer: TransactionSigner): Instruction {
  return getFreezeTombstoneInstruction({ character, asset, payer, mplCoreProgram: MPL_CORE_PROGRAM, systemProgram: SYSTEM_PROGRAM });
}

// ---------------------------------------------------------------------------------------------
// maps

/** Patch 11: `minPlayers` sets the reward budget, `chests` and `bossTable` are reward table ids.
 * Patch 13: `tiles` are cell bytes (`cellByte`), the entrance and the exit are points. */
export type MapDraft = {
  width: number;
  height: number;
  tier: number;
  minPlayers: number;
  /** The DM's chest radius and time to settle (`create_map` settings). */
  chestRadius: number;
  settleTimeoutSecs: number;
  /** The art set (`Biome`), `create_map` settings. */
  biome: number;
  tiles: Uint8Array;
  entrance: Cell | null;
  exit: Cell | null;
  spawns: Spawn[];
  chests: Chest[];
  bossTable: number;
};

/** Cells exactly as the chain stores them: stride 32, cells outside the map Void (0). */
export function chainTiles(d: MapDraft): Uint8Array {
  const out = new Uint8Array(MAP_CELLS);
  for (let y = 0; y < d.height; y++) for (let x = 0; x < d.width; x++) out[y * MAP_MAX + x] = d.tiles[y * MAP_MAX + x] ?? cellByte(Tile.Wall);
  return out;
}

/** `MapLayout.points`: the entrance, the exit, then the chests (a chest's index in a party is its place
 * among the chests). */
export function mapPoints(d: Pick<MapDraft, "entrance" | "exit" | "chests">) {
  if (!d.entrance || !d.exit) throw new Error("the map needs an entrance and an exit");
  const at = (kind: PointKind, c: Cell, data = 0) => ({ kind, x: c.x, y: c.y, data });
  return [at(PointKind.Entrance, d.entrance), at(PointKind.Exit, d.exit), ...d.chests.map((c) => at(PointKind.Chest, c, c.table))];
}

/** The entrance and the exit of a draft, for the grid. */
export function draftPoints(d: Pick<MapDraft, "entrance" | "exit">): GridPoint[] {
  return [
    ...(d.entrance ? [{ ...d.entrance, kind: "entrance" as const }] : []),
    ...(d.exit ? [{ ...d.exit, kind: "exit" as const }] : []),
  ];
}

/** create_map, write_layout, write_spawns, write_points, finalize_map (one transaction each, as the test). */
export async function buildMapIxs(dm: TransactionSigner, d: MapDraft): Promise<{ map: Address; layout: Address; index: number; ixs: Instruction[] }> {
  const index = crypto.getRandomValues(new Uint32Array(1))[0]!;
  const map = await mapPda(dm.address, index);
  const layout = await layoutPda(map);
  const edit = { dm, map, layout };
  const tiles = chainTiles(d);
  const points = mapPoints(d);
  const ixs = [
    getCreateMapInstruction({ ...edit, systemProgram: SYSTEM_PROGRAM, index, width: d.width, height: d.height, tier: d.tier, minPlayers: d.minPlayers, chestRadius: d.chestRadius, settleTimeoutSecs: d.settleTimeoutSecs, biome: d.biome }),
    getWriteLayoutInstruction({ ...edit, offset: 0, cells: tiles.slice(0, (d.height - 1) * MAP_MAX + d.width) }),
    getWriteSpawnsInstruction({ ...edit, spawns: d.spawns }),
    getWritePointsInstruction({ ...edit, points, bossTable: d.bossTable }),
    // The builder rules read the registry bestiary, monster skills (roles, pools, slots), reward
    // tables and the tileset (patch 13).
    getFinalizeMapInstruction({
      ...edit,
      bestiary: await bestiaryPda(),
      monsterSkills: await monsterSkillsPda(),
      rewards: await rewardsPda(),
      tileset: await tilesetPda(),
      secretsRoot: new Uint8Array(32),
    }),
  ];
  return { map, layout, index, ixs };
}

/** Patch 11: rent of what `join_session` creates: the participation, and the character's reward
 * cache unless it already exists (a later party rebinds it). */
export async function joinRent(character: Address): Promise<bigint> {
  const rent = (size: number) => client.rpc.getMinimumBalanceForRentExemption(BigInt(size)).send();
  const [cache] = await getAccounts([await rewardCachePda(character)]);
  const [p, c] = await Promise.all([rent(getParticipationSize()), cache ? Promise.resolve(0n) : rent(getRewardCacheSize())]);
  return BigInt(p) + BigInt(c);
}

/** Rent of the two accounts `create_map` makes (Map and MapLayout), in lamports. */
export async function mapRent(): Promise<bigint> {
  const rent = (size: number) => client.rpc.getMinimumBalanceForRentExemption(BigInt(size)).send();
  const [map, layout] = await Promise.all([rent(getMapSize()), rent(getMapLayoutSize())]);
  return BigInt(map) + BigInt(layout);
}

/** Maps of `dm` (memcmp on MapData.dm right after version). */
export function mapsOf(dm: Address) {
  return findAccounts("Map", [{ offset: 9, bytes: addrBytes(dm) }]);
}

export async function mapTiles(layout: Address): Promise<Uint8Array | null> {
  const l = await getAccount("MapLayout", layout);
  return l ? Uint8Array.from(l.core.cells) : null;
}

/** Patch 13: the entrance and the exit of a layout, for the grid. */
export function layoutPoints(core: { points: { kind: PointKind; x: number; y: number }[]; pointCount: number }): GridPoint[] {
  return core.points.slice(0, core.pointCount).flatMap((p): GridPoint[] =>
    p.kind === PointKind.Entrance ? [{ x: p.x, y: p.y, kind: "entrance" }] : p.kind === PointKind.Exit ? [{ x: p.x, y: p.y, kind: "exit" }] : [],
  );
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
  return getOpenSessionInstruction({
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
    rewards: await rewardsPda(),
    triples: await triplesPda(),
    priceLamports,
    dmActor,
  });
}

/** `cacheSessions` (patch 11): the `GameSession` of every `Open` entry of the character's reward
 * caches, in entry order ([`openCacheSessions`]). */
export async function joinSessionIx(
  player: TransactionSigner,
  session: Address,
  character: Address,
  contribution: bigint,
  actor: Address,
  cacheSessions: readonly Address[] = [],
): Promise<Instruction> {
  const ix = getJoinSessionInstruction({
    player,
    session,
    character,
    participation: await participationPda(session, character),
    rewardCache: await rewardCachePda(character),
    escrow: await escrowPda(session),
    systemProgram: SYSTEM_PROGRAM,
    contribution,
    actor,
  });
  return withRemaining(ix, cacheSessions.map((address) => ({ address, writable: false })));
}

/** `CacheStatus` (crates/types): a reward cache entry. */
export const CACHE_STATUS = { Empty: 0, Open: 1, Claimed: 2 } as const;

/** Patch 11: the character's reward cache entries (none when it has no cache account yet). */
export async function rewardCacheEntries(character: Address) {
  const cache = await getAccount("RewardCache", await rewardCachePda(character));
  return cache ? cache.entries.filter((e) => e.status !== CACHE_STATUS.Empty) : [];
}

/** Patch 11: what `join_session` takes after its accounts: the parties of the `Open` entries. */
export async function openCacheSessions(character: Address): Promise<Address[]> {
  return (await rewardCacheEntries(character)).filter((e) => e.status === CACHE_STATUS.Open).map((e) => e.session);
}

/** Patch 11, leave_party: the owner's main wallet frees a living character from its ended party
 * (before joining another, changing gear or pressing "Done"). */
export async function leavePartyIx(owner: TransactionSigner, session: Address, character: Address): Promise<Instruction> {
  return getLeavePartyInstruction({
    owner,
    session,
    character,
    sessionAuthority: await sessionAuthorityPda(),
    config: await gameConfigPda(),
    characterProgram: PROGRAMS.character,
  });
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
  return getSetReadyInstruction({
    player,
    session,
    participation: await participationPda(session, character),
    character,
    skillSet: await skillSetPda(character),
    sessionAuthority: await sessionAuthorityPda(),
    config: await gameConfigPda(),
    characterProgram: PROGRAMS.character,
    skillExtras: await skillExtrasPda(character),
    ready,
    selection: Uint8Array.from(selection),
  });
}

export async function startSessionIx(dm: TransactionSigner, session: Address): Promise<Instruction> {
  return getStartSessionInstruction({ dm, session, monsters: await monstersPda(session), escrow: await escrowPda(session), ...(await vrfAccounts()) });
}

/**
 * Binds `sessionKey` to the party (patch 10), main wallet only: for the player's `character`, or for
 * the DM when `character` is null. The old key stops passing.
 */
export async function setSessionKeyIx(wallet: TransactionSigner, session: Address, character: Address | null, sessionKey: Address): Promise<Instruction> {
  return getSetSessionKeyInstruction({
    signer: wallet,
    session,
    participation: character ? await participationPda(session, character) : undefined,
    sessionKey,
  });
}

/** A player's in-session keys: the session key signs, the SessionTokenV2 proves it may. */
export type Actor = { signer: TransactionSigner; token: Address | null };

/** An optional account: undefined is Anchor's None (the program id). */
const opt = <T>(v: T | null): T | undefined => v ?? undefined;

/** Who moves: a player (its slot and character) or a monster of the DM (its index). */
export type Mover = { kind: "player"; slot: number; character: Address } | { kind: "monster"; index: number };

/**
 * Patch 13: `move_path` for a player (its main wallet or session key) or a monster (the DM's): the
 * cells of `steps` in order. Stepping next to an enemy can trigger its reactive skill (a VRF roll),
 * so the VRF accounts always come along.
 */
export async function movePathIx(actor: Actor, session: Address, layout: Address, mover: Mover, steps: Step[]): Promise<Instruction> {
  return getMovePathInstruction({
    signer: actor.signer,
    session,
    participation: mover.kind === "player" ? await participationPda(session, mover.character) : undefined,
    monsters: await monstersPda(session),
    layout,
    sessionToken: opt(actor.token),
    ...(await vrfAccounts()),
    unit: mover.kind === "player" ? mover.slot : MONSTER_BASE + mover.index,
    path: steps.map((s) => s.to),
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
  return getActInstruction({
    signer: actor.signer,
    session,
    participation: await participationPda(session, character),
    character,
    skillSet: await skillSetPda(character),
    bag: await bagPda(character),
    monsters: await monstersPda(session),
    sessionToken: opt(actor.token),
    ...(await vrfAccounts()),
    targetParticipation: opt(targetParticipation),
    layout,
    skillSlot,
    target,
  });
}

/** `character` null: a monster turn, ended by the DM's actor. */
export async function endTurnIx(actor: Actor, session: Address, character: Address | null): Promise<Instruction> {
  return getEndTurnInstruction({
    signer: actor.signer,
    session,
    monsters: await monstersPda(session),
    participation: character ? await participationPda(session, character) : undefined,
    sessionToken: opt(actor.token),
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

/** Compute limit of a transaction that may finalize deaths (`character.mark_dead` each, about
 * 45k CU; four players at most). */
export const FINALIZE_CU = 600_000;
/** ComputeBudget program: `SetComputeUnitLimit(units)`. */
export function computeLimitIx(units: number): Instruction {
  const data = new Uint8Array(5);
  data[0] = 2;
  new DataView(data.buffer).setUint32(1, units, true);
  return { programAddress: COMPUTE_BUDGET_PROGRAM, data };
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
  const ix = getStartTurnInstruction({
    session,
    monsters: await monstersPda(session),
    participation: player?.participation,
    character: player?.character,
    sessionAuthority: await sessionAuthorityPda(),
    config: await gameConfigPda(),
    characterProgram: PROGRAMS.character,
    // Burned if over-time damage kills the player.
    skillSet: player ? await skillSetPda(player.character) : undefined,
    bag: player ? await bagPda(player.character) : undefined,
    // Pays the VRF fee when over-time damage triggers a reactive skill (patch 5).
    payer,
    ...(await vrfAccounts()),
  });
  return withRemaining(ix, deaths);
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
  return getMonsterActInstruction({
    signer: dmActor.signer,
    session,
    monsters: await monstersPda(session),
    participation: await participationPda(session, character),
    character,
    skillSet: await skillSetPda(character),
    bag: await bagPda(character),
    sessionToken: opt(dmActor.token),
    ...(await vrfAccounts()),
    layout,
    monster,
    action,
    target,
  });
}

/** Patch 11: each player's `(Participation, RewardCache, Character)`, writable, in slot order:
 * what `settle_party` takes after the pending deaths. */
export async function settleAccountsOf(s: GameSessionAcc, session: Address): Promise<RemainingAccount[]> {
  const out: RemainingAccount[] = [];
  for (let slot = 0; slot < s.core.playerCount; slot++) out.push(...(await playerSettleAccounts(session, s.core.players[slot]!)));
  return out;
}

async function playerSettleAccounts(session: Address, character: Address): Promise<RemainingAccount[]> {
  return [
    { address: await participationPda(session, character), writable: true },
    { address: await rewardCachePda(character), writable: true },
    { address: character, writable: true },
  ];
}

async function endAccounts(session: Address, map: Address) {
  return {
    session,
    escrow: await escrowPda(session),
    monsters: await monstersPda(session),
    map,
    sessionAuthority: await sessionAuthorityPda(),
    config: await gameConfigPda(),
    mapProgram: PROGRAMS.map,
    characterProgram: PROGRAMS.character,
  };
}

/** Patch 11, settle_party: the DM's main wallet (never the session key) settles every player and
 * takes the escrow. Remaining: the pending deaths, then every player's accounts. */
export async function settlePartyIx(dm: TransactionSigner, session: Address, s: GameSessionAcc): Promise<Instruction> {
  const ix = getSettlePartyInstruction({ dm, ...(await endAccounts(session, s.core.map)) });
  return withRemaining(ix, [...(await deathAccountsOf(s)), ...(await settleAccountsOf(s, session))]);
}

/** Patch 11, claim_after_timeout: after the DM's deadline, the player's main wallet settles its own
 * character and takes its contribution back. Remaining: its accounts, then the pending deaths. */
export async function claimAfterTimeoutIx(player: TransactionSigner, session: Address, s: GameSessionAcc, character: Address): Promise<Instruction> {
  const ix = getClaimAfterTimeoutInstruction({ player, ...(await endAccounts(session, s.core.map)) });
  return withRemaining(ix, [...(await playerSettleAccounts(session, character)), ...(await deathAccountsOf(s))]);
}

/** `BagLayout` (crates/types): 40 resource slots, 5 consumable slots. */
export type BagLayout = { resources: BagSlotIn[]; consumables: BagSlotIn[] };
/** A bag slot with its `RewardKind` as a number (crates/types order). */
type BagSlotIn = { kind: number; id: number; qty: number };
const slotArgs = (r: BagSlotIn): BagSlotArgs => ({ kind: r.kind as BagSlotArgs["kind"], id: r.id, qty: r.qty });

/** Patch 11, close_cache: "Done" in the bag screen for party `session`, the owner's main wallet; `layout` is the final
 * bag (resources and consumables), checked on chain against the bag and the cache. */
export async function closeCacheIx(owner: TransactionSigner, character: Address, session: Address, layout: BagLayout): Promise<Instruction> {
  return getCloseCacheInstruction({
    session,
    owner,
    rewardCache: await rewardCachePda(character),
    character,
    bag: await bagPda(character),
    sessionAuthority: await sessionAuthorityPda(),
    config: await gameConfigPda(),
    characterProgram: PROGRAMS.character,
    // Codama flattens the one struct argument into its fields.
    resources: layout.resources.map(slotArgs),
    consumables: layout.consumables.map(slotArgs),
  });
}

/** Patch 11, open_chest: player's main wallet or session key (+ token), on its turn, next to the
 * chest; the VRF callback rolls the rewards. */
export async function openChestIx(actor: Actor, session: Address, character: Address, layout: Address, chest: number): Promise<Instruction> {
  return getOpenChestInstruction({
    signer: actor.signer,
    session,
    participation: await participationPda(session, character),
    character,
    skillSet: await skillSetPda(character),
    bag: await bagPda(character),
    monsters: await monstersPda(session),
    sessionToken: opt(actor.token),
    ...(await vrfAccounts()),
    layout,
    chest,
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
/** Patch 11: the registry `rewards` body (108-byte reward table records by id). */
export const registryRewards = () => registryTable(rewardsPda);
/** Patch 13: the registry `tileset` body (8-byte records by tile type). */
export const registryTileset = () => registryTable(tilesetPda);

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
  const ix = getConsumeRandomnessInstruction({
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
    targetParticipation: undefined,
    layout,
    randomness: seed,
  });
  return withRemaining(ix, party);
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
  /** The resonance tables the session copied (aspects, traits, resonances, triples bodies). */
  resonanceTables: Uint8Array | null;
  /** Cells a resonance set on fire. */
  burningCells: BurningCellAcc[];
  escrow: { collected: bigint; claimed: boolean } | null;
  map: MapAcc | null;
  tiles: Uint8Array | null;
  /** Patch 13: the entrance and the exit (read with the cells, once). */
  points: GridPoint[];
};

/** The points read with a layout's cells: a caller passes the cells back, the points come along. */
const LAYOUT_POINTS = new WeakMap<Uint8Array, GridPoint[]>();

export async function readParty(address: Address, tiles: Uint8Array | null = null, prev: Party | null = null): Promise<Party | null> {
  let points = (tiles && LAYOUT_POINTS.get(tiles)) ?? [];
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
  const decoded = decodeAccount("GameSession", rawSession);
  // Patch 9: the aura weights travel with their auras.
  const session = { ...decoded, auras: decoded.auras.map((p, i) => p.map((a, j): AuraAcc => ({ ...a, weightsPct: decoded.auraWeights[i]?.[j] }))) };
  const monsterState = rawMonsters ? decodeAccount("MonsterState", rawMonsters) : null;
  const count = monsterState?.core.count ?? 0;
  const escrow = rawEscrow ? decodeAccount("Escrow", rawEscrow) : null;
  const chars = session.core.players.slice(0, session.core.playerCount);
  const parts = await Promise.all(chars.map((c) => participationPda(address, c)));
  const wanted = [...parts, ...chars, session.core.map, ...(tiles ? [] : [session.layout])];
  const extra = wanted.length === guess.length && wanted.every((a, i) => a === guess[i]) ? guessed : await getAccounts(wanted);
  const players: PartyPlayer[] = chars.flatMap((character, slot) => {
    const p = extra[slot];
    const c = extra[chars.length + slot];
    if (!p || !c) return [];
    const part = decodeAccount("Participation", p);
    return [{ slot, character, participation: parts[slot]!, part: { ...part, mods: withResist(part.mods, part.resistMod) }, char: decodeAccount("Character", c) }];
  });
  const rawMap = extra[chars.length * 2];
  const map = rawMap ? decodeAccount("Map", rawMap) : null;
  if (!tiles) {
    const rawLayout = extra[chars.length * 2 + 1];
    const layout = rawLayout ? decodeAccount("MapLayout", rawLayout).core : null;
    tiles = layout ? Uint8Array.from(layout.cells) : null;
    points = layout ? layoutPoints(layout) : [];
    if (tiles) LAYOUT_POINTS.set(tiles, points);
  }
  const monsterCooldowns = (monsterState?.skillCooldowns ?? []).slice(0, count).map((c) => Uint8Array.from(c));
  const monsterRecords = (monsterState?.records ?? []).slice(0, count).map((r) => Uint8Array.from(r));
  const table = (monsterState?.skillTable ?? []).slice(0, monsterState?.skillCount ?? 0);
  const monsterSkills = (monsterState?.skills ?? [])
    .slice(0, count)
    .map((slots) => Array.from(slots).map((t) => (t === 0xff || !table[t] ? null : Uint8Array.from(table[t]!))));
  const monsterAuras = (monsterState?.auras ?? [])
    .slice(0, count)
    .map((p, i) => p.map((a, j) => ({ ...a, aura: { ...a.aura, weightsPct: monsterState?.auraWeights[i]?.[j] } as AuraAcc })));
  const monsterMods = (monsterState?.mods ?? []).slice(0, count).map((m, i) => withResist(m, monsterState?.resistMods[i]));
  const monsterDefense = (monsterState?.defense ?? []).slice(0, count);
  const resonanceTables = monsterState
    ? concat(
        Uint8Array.from(monsterState.resAspects),
        Uint8Array.from(monsterState.resTraits),
        Uint8Array.from(monsterState.resPairs),
        Uint8Array.from(monsterState.resTriples),
      )
    : null;
  const burningCells = (monsterState?.burningCells ?? []).filter((c) => c.power > 0);
  return {
    address,
    session,
    monsters: (monsterState?.core.monsters ?? []).slice(0, count),
    players,
    monsterCooldowns,
    monsterRecords,
    monsterSkills,
    monsterAuras,
    monsterMods,
    monsterDefense,
    resonanceTables,
    burningCells,
    escrow: escrow && { collected: escrow.collected, claimed: escrow.claimed },
    map,
    tiles,
    points,
  };
}

/** Parties in the lobby or in play, any DM. */
export async function openSessions(): Promise<{ address: Address; data: GameSessionAcc }[]> {
  const all = await findAccounts("GameSession");
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
  return getSpendStatPointsInstruction({ owner, character, points: [...points] });
}
