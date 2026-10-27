// The city program on the site (docs/CITY_DESCRIPTION.md, agents/CONTRACTS.md 25): the city and its
// buildings, the Tavern boards and their quests, and the DM's publish transaction. Looking is free
// (account reads); posting a quest is the DM's main wallet. Nothing here touches React.
import { getAddressEncoder, getProgramDerivedAddress, type Address, type Instruction, type TransactionSigner } from "@solana/kit";
import {
  BuildingKind,
  BuildingStatus,
  findBoardPda,
  findCityPda,
  findQuestPda,
  getCloseQuestInstruction,
  getPostQuestInstruction,
  getQuestSize,
  type Building,
  type City,
  type Quest,
} from "@deadroll/sdk/city";
import { getEscrowSize, getGameSessionSize, getMonsterStateSize } from "@deadroll/sdk/session";
import { PROGRAMS, SYSTEM_PROGRAM } from "../config";
import { client } from "../wallet/client";
import { findAccounts, getAccounts, decodeAccount, sessionAccounts, type EscrowAcc, type GameSessionAcc, type MapAcc } from "./game";
import { gameConfigPda } from "./pda";

export { BuildingKind, BuildingStatus };
export type CityAcc = City;
export type BuildingAcc = Building;
export type QuestAcc = Quest;

/** `CITY_W_MAX` (crates/types): cells are row-major, 48 per row. */
export const CITY_W_MAX = 48;
/** A city cell byte is a site city tile: sprite id `2000 + byte` (assets/city/city.json). */
export const CITY_TILE_BASE = 2000;

const enc = getAddressEncoder();

export const cityPda = async (): Promise<Address> => (await findCityPda())[0];
/** `["building", id: u16 LE]`: the id comes from the city, so the IDL has no seeds for it. */
export const buildingPda = async (id: number): Promise<Address> =>
  (await getProgramDerivedAddress({ programAddress: PROGRAMS.city, seeds: ["building", new Uint8Array([id & 0xff, id >> 8])] }))[0];
export const boardPda = async (building: Address): Promise<Address> => (await findBoardPda({ building }))[0];
export const questPda = async (session: Address): Promise<Address> => (await findQuestPda({ session }))[0];

export type CityView = { city: CityAcc; buildings: { address: Address; data: BuildingAcc }[] };

/** The city and its buildings (two reads), or null before the city is seeded. */
export async function readCity(): Promise<CityView | null> {
  const [raw] = await getAccounts([await cityPda()]);
  if (!raw) return null;
  const city = decodeAccount("City", raw);
  const ids = Array.from({ length: city.core.buildingCount }, (_, i) => i);
  const addresses = await Promise.all(ids.map(buildingPda));
  const raws = await getAccounts(addresses);
  const buildings = addresses.flatMap((address, i) => (raws[i] ? [{ address, data: decodeAccount("Building", raws[i]!) }] : []));
  return { city, buildings };
}

/** A quest on the board with the live state of its party. */
export type QuestView = {
  address: Address;
  quest: QuestAcc;
  session: GameSessionAcc | null;
  escrow: EscrowAcc | null;
  map: MapAcc | null;
};

/** `Quest.board` sits after the discriminator and `version`. */
const QUEST_BOARD_OFFSET = 9;

/** The quests of a board (one getProgramAccounts) and their parties, escrows and maps (one read). */
export async function questsOn(board: Address): Promise<QuestView[]> {
  const quests = await findAccounts("Quest", [{ offset: QUEST_BOARD_OFFSET, bytes: Uint8Array.from(enc.encode(board)) }]);
  if (quests.length === 0) return [];
  const escrows = await Promise.all(quests.map((q) => sessionAccounts(q.data.core.map).then((a) => a.escrow)));
  const raws = await getAccounts(quests.flatMap((q, i) => [q.data.core.session, escrows[i]!, q.data.core.map]));
  return quests
    .map((q, i) => {
      const at = (k: number) => raws[i * 3 + k] ?? null;
      const safe = <T>(f: () => T): T | null => {
        try {
          return f();
        } catch {
          return null;
        }
      };
      return {
        address: q.address,
        quest: q.data,
        session: at(0) ? safe(() => decodeAccount("GameSession", at(0)!)) : null,
        escrow: at(1) ? safe(() => decodeAccount("Escrow", at(1)!)) : null,
        map: at(2) ? safe(() => decodeAccount("Map", at(2)!)) : null,
      };
    })
    .sort((a, b) => Number(b.quest.core.postedAt - a.quest.core.postedAt));
}

/** city `post_quest`: after `open_session` in the same transaction (the DM's main wallet). */
export async function postQuestIx(dm: TransactionSigner, building: Address, map: Address): Promise<Instruction> {
  const { session } = await sessionAccounts(map);
  return getPostQuestInstruction({
    dm,
    config: await gameConfigPda(),
    building,
    board: await boardPda(building),
    map,
    session,
    quest: await questPda(session),
    systemProgram: SYSTEM_PROGRAM,
  });
}

/** city `close_quest`: the DM takes the quest down; its rent comes back. */
export async function closeQuestIx(dm: TransactionSigner, board: Address, session: Address): Promise<Instruction> {
  return getCloseQuestInstruction({ dm, config: await gameConfigPda(), board, quest: await questPda(session), session });
}

/** Rent of what publishing creates: the party (session, monsters, escrow) and its quest. */
export async function publishRent(): Promise<bigint> {
  const rent = (size: number) => client.rpc.getMinimumBalanceForRentExemption(BigInt(size)).send();
  const all = await Promise.all([getGameSessionSize(), getMonsterStateSize(), getEscrowSize(), getQuestSize()].map(rent));
  return all.reduce((a, b) => a + BigInt(b), 0n);
}

/** What the board shows for a party. */
export function questState(q: QuestView): "lobby" | "active" | "ended" | "gone" {
  if (!q.session) return "gone";
  const status = q.session.core.status;
  return status === 0 ? "lobby" : status === 1 ? "active" : "ended";
}
