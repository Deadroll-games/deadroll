// The city of assets/city/city.json as the city program stores it (docs/CITY_DESCRIPTION.md 3).
import { readFileSync } from "node:fs";
import { BuildingKind, BuildingStatus } from "@deadroll/sdk/city";

/** `CITY_W_MAX`, `CITY_H_MAX`, `CITY_OBJECTS_MAX` (crates/types): cells are row-major, 48 per row. */
export const CITY_W_MAX = 48;
export const CITY_H_MAX = 32;
export const CITY_OBJECTS_MAX = 32;

type Cell = { x: number; y: number };
export type CityBuilding = {
  name: string;
  kind: BuildingKind;
  prefab: number;
  x: number;
  y: number;
  door: Cell;
  interior: number;
  status: BuildingStatus;
};
export type CityPlan = {
  width: number;
  height: number;
  /** `CITY_W_MAX * height` bytes: the rows in use (the rest of each row stays 0). */
  cells: Uint8Array;
  spawn: Cell;
  objects: { sprite: number; x: number; y: number }[];
  buildings: CityBuilding[];
};

const enumOf = <T extends Record<string, string | number>>(e: T, name: string, what: string): T[keyof T] => {
  const v = e[name as keyof T];
  if (typeof v !== "number") throw new Error(`city.json: unknown ${what} ${name}`);
  return v as T[keyof T];
};

export function loadCity(path = new URL("../../../assets/city/city.json", import.meta.url)): CityPlan {
  const raw = JSON.parse(readFileSync(path, "utf8"));
  const rows: string[] = raw.rows;
  const height = rows.length;
  const width = rows[0].length;
  if (width > CITY_W_MAX || height > CITY_H_MAX) throw new Error(`city.json: ${width}x${height} is over ${CITY_W_MAX}x${CITY_H_MAX}`);
  const cells = new Uint8Array(CITY_W_MAX * height);
  rows.forEach((row, y) => {
    if (row.length !== width) throw new Error(`city.json: row ${y} is ${row.length} cells, not ${width}`);
    [...row].forEach((ch, x) => {
      const byte = raw.legend[ch];
      if (typeof byte !== "number") throw new Error(`city.json: no legend for '${ch}' at ${x},${y}`);
      cells[y * CITY_W_MAX + x] = byte;
    });
  });
  const inside = (c: Cell) => c.x >= 0 && c.y >= 0 && c.x < width && c.y < height;
  const objects = raw.objects as CityPlan["objects"];
  if (objects.length > CITY_OBJECTS_MAX || !objects.every(inside) || !inside(raw.spawn)) throw new Error("city.json: objects or spawn outside the city");
  const buildings: CityBuilding[] = raw.buildings.map((b: any) => ({
    name: b.name,
    kind: enumOf(BuildingKind, b.kind, "kind"),
    prefab: b.prefab,
    x: b.x,
    y: b.y,
    door: b.door,
    interior: b.interior,
    status: enumOf(BuildingStatus, b.status, "status"),
  }));
  return { width, height, cells, spawn: raw.spawn, objects, buildings };
}
