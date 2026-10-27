// The city as the site draws it: from the city program's accounts, or in mock mode from
// assets/city/city.json (the same data the seed script puts on chain).
import type { Address } from "@solana/kit";
import cityJson from "../../../../assets/city/city.json";
import { BuildingKind, BuildingStatus, CITY_W_MAX, readCity, type CityView } from "../../chain/city";

export type CityBuilding = {
  id: number;
  address: Address;
  kind: number;
  prefabId: number;
  x: number;
  y: number;
  door: { x: number; y: number };
  target: Address;
  interiorId: number;
  status: number;
};

export type CityModel = {
  width: number;
  height: number;
  /** Row-major cell bytes, `CITY_W_MAX` per row. */
  cells: ArrayLike<number>;
  objects: { sprite: number; x: number; y: number }[];
  spawn: { x: number; y: number };
  buildings: CityBuilding[];
};

export function fromChain(v: CityView): CityModel {
  const c = v.city.core;
  return {
    width: c.width,
    height: c.height,
    cells: c.cells,
    objects: c.objects.slice(0, c.objectCount).map((o) => ({ sprite: o.sprite, x: o.x, y: o.y })),
    spawn: c.spawn,
    buildings: v.buildings.map(({ address, data: { core: b } }) => ({
      id: b.id,
      address,
      kind: b.kind,
      prefabId: b.prefabId,
      x: b.x,
      y: b.y,
      door: b.door,
      target: b.target,
      interiorId: b.interiorId,
      status: b.status,
    })),
  };
}

/** The seeded city without a validator (mock mode). */
export function mockCity(): CityModel {
  const rows = cityJson.rows;
  const legend = cityJson.legend as Record<string, number>;
  const cells = new Uint8Array(CITY_W_MAX * rows.length);
  rows.forEach((row, y) => [...row].forEach((ch, x) => (cells[y * CITY_W_MAX + x] = legend[ch] ?? 0)));
  return {
    width: rows[0]!.length,
    height: rows.length,
    cells,
    objects: cityJson.objects,
    spawn: cityJson.spawn,
    buildings: cityJson.buildings.map((b, id) => ({
      id,
      address: `MockBuilding${id}1111111111111111111111111111` as Address,
      kind: BuildingKind[b.kind as keyof typeof BuildingKind] as number,
      prefabId: b.prefab,
      x: b.x,
      y: b.y,
      door: b.door,
      target: `MockBoard${id}11111111111111111111111111111111` as Address,
      interiorId: b.interior,
      status: BuildingStatus[b.status as keyof typeof BuildingStatus] as number,
    })),
  };
}

export async function loadCity(mock: boolean): Promise<CityModel | null> {
  if (mock) return mockCity();
  const v = await readCity();
  return v ? fromChain(v) : null;
}

export const isOpen = (b: CityBuilding) => b.status === BuildingStatus.Active;
export const isTavern = (b: CityBuilding) => b.kind === BuildingKind.Tavern;
