// Sprite layers for characters, monsters and tiles from the sprite books (assets/sprites, packed by
// `art pack`). Ramp and material indices are positions in assets/palette/palette_2.pal (the first
// 20 ramps and 13 materials are those of palette 1). A character is drawn as its body, its hair and
// the items it wears (chain/items.ts `characterRequest`); `Character.appearance` holds the body part.
import { Aspect, LAYERS_MAX, Race, Tile, type LayerRef, type RenderRequest } from "./types";
import type { GridPointKind } from "../components/grid/types";

/** palette_2.pal `ramp` lines, in order (append only). */
export const RAMP = {
  gray: 0, steel: 1, skin: 2, skinElf: 3, hair: 4, hairElf: 5, robe: 6, green: 7, leather: 8, gold: 9,
  cold: 10, fire: 11, dark: 12, eyes: 13, capeRed: 14, fur: 15, stone: 16, floor: 17, water: 18, wood: 19,
  cloth: 20, moss: 21, bone: 22, blood: 23, curse: 24, dirt: 25, furDark: 26, gobSkin: 27, rag: 28, furBrown: 29,
  bark: 30, skinPale: 31, troll: 32, bruise: 33, rust: 34, glowRed: 35, glowToxic: 36, glowSoul: 37,
  rot: 38, ooze: 39, blackiron: 40, ash: 41, ogre: 42, furPale: 43, wyrm: 44, hydra: 45, storm: 46,
  caveRock: 47, murk: 48,
} as const;
/** palette_2.pal `material` lines, in order (append only). */
export const MATERIAL = {
  steel: 0, human: 1, elf: 2, robe: 3, leather: 4, cape: 5, wolf: 6, goblin: 7, stone: 8,
  tileWall: 9, tileFloor: 10, tileWater: 11, tileDoor: 12, cloth: 13, moss: 14, bone: 15, chest: 16,
  wolfDark: 17, goblinDark: 18, bear: 19, treant: 20, fairy: 21, troll: 22, demon: 23, rusted: 24,
  caveWall: 50, caveFloor: 51, caveDoor: 52, murk: 53, planks: 54,
} as const;

export const SPRITE = { tombstone: 403 } as const;

/** Character layer slots: the index in `RenderRequest.layers` (crates/render/FORMAT.md 1.4). */
export const LAYER = {
  shadow: 0, backItem: 1, body: 2, gloveBack: 3, pants: 4, boots: 5, armorBody: 6, necklace: 7,
  hair: 8, offhand: 9, helmet: 10, mainhand: 11, fistFront: 12, gloveFront: 13, overlayElement: 14,
} as const;

type Entry = [layer: number, spriteId: number, material: number, accent: number];

const EMPTY: LayerRef = { spriteId: 0, materialRamp: 0, accentRamp: 0, elementRamp: 0 };

function layers(entries: Entry[], element: number): LayerRef[] {
  const out = Array.from({ length: LAYERS_MAX }, () => ({ ...EMPTY }));
  for (const [i, spriteId, material, accent] of entries) out[i] = { spriteId, materialRamp: material, accentRamp: accent, elementRamp: element };
  return out;
}

/** Body sprites: the whole body of a race, its hair, and the front fist every race shares. */
const BODY = { human: 10, humanHair: 11, elf: 12, elfHair: 13, fistFront: 14 } as const;

/**
 * Whether an appearance uses the current graphics (one-piece bodies). Characters created before
 * patch 7c store old body part sprites and layer slots: the site shows a label, not a portrait.
 */
export function isCurrentAppearance(appearance: readonly LayerRef[]): boolean {
  const id = appearance[LAYER.body]?.spriteId;
  return id === BODY.human || id === BODY.elf;
}

function body(race: Race): Entry[] {
  const elf = race === Race.ForestElf;
  const m = elf ? MATERIAL.elf : MATERIAL.human;
  return [
    [LAYER.body, elf ? BODY.elf : BODY.human, m, RAMP.eyes],
    [LAYER.hair, elf ? BODY.elfHair : BODY.humanHair, m, RAMP.eyes],
    [LAYER.fistFront, BODY.fistFront, m, RAMP.eyes],
  ];
}

export const ELEMENT_RAMP: Record<number, number> = { [Aspect.Fire]: RAMP.fire, [Aspect.Cold]: RAMP.cold, [Aspect.Lightning]: RAMP.gold, [Aspect.Blood]: RAMP.capeRed, [Aspect.Blunt]: RAMP.steel, [Aspect.Piercing]: RAMP.steel };

/**
 * `Character.appearance` for a new character: the shadow, the body of the race, its hair and the
 * front fist. No items: what the character wears comes from its gear when it is drawn.
 */
export function characterAppearance(race: Race, predisposition: Aspect): LayerRef[] {
  const shadow: Entry = [LAYER.shadow, 1, MATERIAL.stone, RAMP.gold];
  return layers([shadow, ...body(race)], ELEMENT_RAMP[predisposition] ?? RAMP.cold);
}

/** A request for the rasterizer. The palette version is filled in at render time from the palette
 * GameConfig points at (`Rasterizer.render`). */
export function renderRequest(appearance: LayerRef[], dead = false): RenderRequest {
  return { paletteVersion: 0, layers: appearance, flags: dead ? 1 : 0 };
}

/** A tombstone in place of a dead character on the grid. */
export const TOMBSTONE: RenderRequest = renderRequest(layers([[0, SPRITE.tombstone, MATERIAL.stone, RAMP.gray]], RAMP.cold));

/** Bestiary id -> sprite, material, accent (eyes, marks) and element (horns, claws, teeth) ramps.
 * tools/art keeps the same colors per sprite name (`MONSTER_LOOKS` in tools/art/src/preview.rs). */
const MONSTER_SPRITES: Record<number, [sprite: number, material: number, accent: number, element: number]> = {
  0: [400, MATERIAL.wolfDark, RAMP.glowRed, RAMP.fur],
  1: [401, MATERIAL.goblinDark, RAMP.glowRed, RAMP.steel],
  2: [402, MATERIAL.goblinDark, RAMP.gold, RAMP.steel],
  3: [404, MATERIAL.goblinDark, RAMP.glowRed, RAMP.steel],
  4: [405, MATERIAL.goblinDark, RAMP.glowToxic, RAMP.bone],
};

export function monsterRequest(bestiaryId: number, dead: boolean): RenderRequest {
  const [id, mat, accent, element] = MONSTER_SPRITES[bestiaryId] ?? MONSTER_SPRITES[1]!;
  return renderRequest(layers([[0, id, mat, accent]], element), dead);
}

type Look = [sprite: number, material: number, accent: number, element: number];

/**
 * The Caves and Ruins tile set, variant 0 of each tile type (assets/sprites/tiles/caves; tools/art
 * keeps the same colors, `TILE_LOOKS`). The map has no biome yet and the variant does not change
 * the picture; walls use one shape (east and west neighbors) until autotiling exists.
 */
const TILE_SPRITES: Record<number, Look> = {
  [Tile.Void]: [537, MATERIAL.caveWall, RAMP.steel, RAMP.cold],
  [Tile.StoneFloor]: [500, MATERIAL.caveFloor, RAMP.bone, RAMP.cold],
  [Tile.WoodenFloor]: [525, MATERIAL.planks, RAMP.steel, RAMP.cold],
  [Tile.Dirt]: [528, MATERIAL.caveFloor, RAMP.bone, RAMP.murk],
  [Tile.Wall]: [501, MATERIAL.caveWall, RAMP.steel, RAMP.cold],
  [Tile.Door]: [503, MATERIAL.caveDoor, RAMP.steel, RAMP.cold],
  [Tile.ShallowWater]: [502, MATERIAL.murk, RAMP.murk, RAMP.cold],
  [Tile.DeepWater]: [532, MATERIAL.murk, RAMP.murk, RAMP.cold],
  [Tile.Stairs]: [533, MATERIAL.caveFloor, RAMP.bone, RAMP.cold],
  [Tile.Rubble]: [534, MATERIAL.caveFloor, RAMP.bone, RAMP.cold],
  [Tile.Chasm]: [536, MATERIAL.caveFloor, RAMP.bone, RAMP.cold],
};

const lookRequest = ([sprite, material, accent, element]: Look) => renderRequest(layers([[0, sprite, material, accent]], element));

/** The render request of a tile type, null for one drawn as a flat color. */
export function tileRequest(tile: Tile): RenderRequest | null {
  const look = TILE_SPRITES[tile];
  return look ? lookRequest(look) : null;
}

/** The entrance and the exit points over their cell: the Caves mine adit and the stone portal. */
const POINT_SPRITES: Record<GridPointKind, Look> = {
  entrance: [504, MATERIAL.caveDoor, RAMP.steel, RAMP.fire],
  exit: [505, MATERIAL.caveWall, RAMP.steel, RAMP.glowSoul],
};

export function pointRequest(kind: GridPointKind): RenderRequest {
  return lookRequest(POINT_SPRITES[kind]);
}

/** A visible chest over its cell, closed or opened (patch 11). */
export function chestRequest(opened: boolean): RenderRequest {
  return lookRequest([opened ? 581 : 580, MATERIAL.chest, RAMP.gold, RAMP.cold]);
}
