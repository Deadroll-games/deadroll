// What the site draws for a building's `prefab_id` and an `interior_id` (assets/city/city.json).
// A building whose prefab has no picture here is not drawn (the city program's tests add such ones).
import { artUrl } from "../art";

/** One cell of the city and of interiors, in pixels (site art is drawn at one pixel per pixel). */
export const CELL = 64;

export type Prefab = {
  /** `city/<name>` in app/assets. */
  art: string;
  /** Size in cells; the picture's bottom row stands on the building's bottom row. */
  w: number;
  h: number;
  /** Where the planks of a closed building go: the top-left pixel of a 64x64 overlay. */
  boards: { x: number; y: number };
};

export const PREFABS: Record<number, Prefab> = {
  1: { art: "tavern", w: 4, h: 4, boards: { x: 96, y: 186 } },
  2: { art: "artisans_guild", w: 4, h: 4, boards: { x: 96, y: 186 } },
  3: { art: "storage", w: 4, h: 3, boards: { x: 96, y: 128 } },
};

export const prefabUrl = (p: Prefab) => artUrl(`city/${p.art}`);

/** A piece of furniture, or a point that opens a window when clicked. */
export type InteriorItem = { sprite: number; x: number; y: number; action?: "board" | "dm" | "exit"; label?: string };

export type Interior = {
  name: string;
  w: number;
  h: number;
  /** Floor sprites by cell, row-major; `wall` marks the wall rows on top. */
  wallRows: number;
  items: InteriorItem[];
};

const FLOOR = [2010, 2011];
const WALL = 2012;
export const floorSprite = (x: number, y: number) => FLOOR[(x * 7 + y * 3) % 5 === 0 ? 1 : 0]!;
export const wallSprite = () => WALL;

/** The Tavern: the guild board on the wall, the DM's table, the counter and the hall. */
const TAVERN: Interior = {
  name: "Tavern",
  w: 11,
  h: 7,
  wallRows: 1,
  items: [
    { sprite: 2026, x: 2, y: 1, action: "board", label: "Guild board: the quests" },
    { sprite: 2020, x: 6, y: 1 },
    { sprite: 2020, x: 7, y: 1 },
    { sprite: 2020, x: 8, y: 1 },
    { sprite: 2023, x: 9, y: 1 },
    { sprite: 2023, x: 10, y: 1 },
    { sprite: 2021, x: 2, y: 4 },
    { sprite: 2022, x: 1, y: 4 },
    { sprite: 2022, x: 3, y: 4 },
    { sprite: 2021, x: 8, y: 4, action: "dm", label: "DM's table: build a map, post a quest" },
    { sprite: 2022, x: 7, y: 4 },
    { sprite: 2022, x: 9, y: 4 },
    { sprite: 2021, x: 5, y: 3 },
    { sprite: 2022, x: 5, y: 4 },
    { sprite: 2010, x: 5, y: 6, action: "exit", label: "Back to the city" },
  ],
};

/** The Artisans' Guild and the Storage are closed for now; their halls are ready for later. */
const GUILD: Interior = {
  name: "Artisans' Guild",
  w: 9,
  h: 6,
  wallRows: 1,
  items: [
    { sprite: 2024, x: 2, y: 1 },
    { sprite: 2025, x: 3, y: 2 },
    { sprite: 2210, x: 5, y: 1 },
    { sprite: 2211, x: 6, y: 1 },
    { sprite: 2212, x: 7, y: 1 },
    { sprite: 2010, x: 4, y: 5, action: "exit", label: "Back to the city" },
  ],
};

const STORAGE: Interior = {
  name: "Storage",
  w: 8,
  h: 6,
  wallRows: 1,
  items: [
    { sprite: 2023, x: 1, y: 1 },
    { sprite: 2023, x: 2, y: 1 },
    { sprite: 2213, x: 5, y: 1 },
    { sprite: 2010, x: 4, y: 5, action: "exit", label: "Back to the city" },
  ],
};

export const INTERIORS: Record<number, Interior> = { 1: TAVERN, 2: GUILD, 3: STORAGE };
