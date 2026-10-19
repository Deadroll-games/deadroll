// Sprite layers for characters, monsters and tiles from the sprite books (assets/sprites, packed by
// `art pack`). Ramp and material indices are positions in assets/palette/palette_2.pal (the first
// 20 ramps and 13 materials are those of palette 1); the character presets mirror
// tools/art/src/preview.rs `characters()`.
import { Aspect, Class, LAYERS_MAX, Race, Tile, type LayerRef, type RenderRequest } from "./types";

/** palette_2.pal `ramp` lines, in order (append only). */
export const RAMP = {
  gray: 0, steel: 1, skin: 2, skinElf: 3, hair: 4, hairElf: 5, robe: 6, green: 7, leather: 8, gold: 9,
  cold: 10, fire: 11, dark: 12, eyes: 13, capeRed: 14, fur: 15, stone: 16, floor: 17, water: 18, wood: 19,
} as const;
/** palette_2.pal `material` lines, in order (append only). */
export const MATERIAL = {
  steel: 0, human: 1, elf: 2, robe: 3, leather: 4, cape: 5, wolf: 6, goblin: 7, stone: 8,
  tileWall: 9, tileFloor: 10, tileWater: 11, tileDoor: 12,
} as const;

export const SPRITE = { tombstone: 403 } as const;

type Entry = [layer: number, spriteId: number, material: number, accent: number];

const EMPTY: LayerRef = { spriteId: 0, materialRamp: 0, accentRamp: 0, elementRamp: 0 };

function layers(entries: Entry[], element: number): LayerRef[] {
  const out = Array.from({ length: LAYERS_MAX }, () => ({ ...EMPTY }));
  for (const [i, spriteId, material, accent] of entries) out[i] = { spriteId, materialRamp: material, accentRamp: accent, elementRamp: element };
  return out;
}

function body(race: Race): Entry[] {
  const [m, ids] = race === Race.ForestElf ? [MATERIAL.elf, [20, 21, 22, 23, 24, 25]] : [MATERIAL.human, [2, 3, 4, 5, 6, 7]];
  // arm_back, legs, torso, head, hair, arm_front at their layer slots.
  return [2, 5, 8, 11, 12, 14].map((slot, i) => [slot, ids[i]!, m, RAMP.eyes] as Entry);
}

function kit(cls: Class): Entry[] {
  const G = RAMP.gold;
  switch (cls) {
    case Class.Mage:
      return [[1, 205, MATERIAL.leather, G], [6, 202, MATERIAL.robe, G], [7, 203, MATERIAL.leather, G], [9, 201, MATERIAL.robe, G], [13, 200, MATERIAL.robe, G], [16, 204, MATERIAL.leather, G]];
    case Class.Support:
      return [[1, 304, MATERIAL.leather, G], [7, 302, MATERIAL.leather, G], [9, 301, MATERIAL.leather, G], [10, 305, MATERIAL.leather, G], [13, 300, MATERIAL.leather, G], [16, 303, MATERIAL.leather, G]];
    default:
      return [[1, 100, MATERIAL.cape, G], [4, 101, MATERIAL.steel, G], [6, 102, MATERIAL.leather, G], [7, 103, MATERIAL.leather, G], [9, 104, MATERIAL.steel, G], [13, 105, MATERIAL.steel, G], [16, 106, MATERIAL.steel, G]];
  }
}

const ELEMENT_RAMP: Record<number, number> = { [Aspect.Fire]: RAMP.fire, [Aspect.Cold]: RAMP.cold, [Aspect.Lightning]: RAMP.gold, [Aspect.Blood]: RAMP.capeRed, [Aspect.Blunt]: RAMP.steel, [Aspect.Piercing]: RAMP.steel };

/** `Character.appearance` for a new character: shadow, body by race, kit by class. */
export function characterAppearance(race: Race, cls: Class, predisposition: Aspect): LayerRef[] {
  const shadow: Entry = [0, 1, MATERIAL.stone, RAMP.gold];
  return layers([shadow, ...body(race), ...kit(cls)], ELEMENT_RAMP[predisposition] ?? RAMP.cold);
}

/** A request for the rasterizer. The palette version is filled in at render time from the palette
 * GameConfig points at (`Rasterizer.render`). */
export function renderRequest(appearance: LayerRef[], dead = false): RenderRequest {
  return { paletteVersion: 0, layers: appearance, flags: dead ? 1 : 0 };
}

/** A tombstone in place of a dead character on the grid. */
export const TOMBSTONE: RenderRequest = renderRequest(layers([[0, SPRITE.tombstone, MATERIAL.stone, RAMP.gray]], RAMP.cold));

const MONSTER_SPRITES: Record<number, [number, number]> = {
  0: [400, MATERIAL.wolf],
  1: [401, MATERIAL.goblin],
  2: [402, MATERIAL.goblin],
  3: [401, MATERIAL.goblin],
  4: [401, MATERIAL.goblin],
};

export function monsterRequest(bestiaryId: number, dead: boolean): RenderRequest {
  const [id, mat] = MONSTER_SPRITES[bestiaryId] ?? [401, MATERIAL.goblin];
  return renderRequest(layers([[0, id, mat, RAMP.leather]], RAMP.cold), dead);
}

const TILE_SPRITES: Record<number, [number, number]> = {
  [Tile.Floor]: [500, MATERIAL.tileFloor],
  [Tile.Wall]: [501, MATERIAL.tileWall],
  [Tile.Water]: [502, MATERIAL.tileWater],
  [Tile.Door]: [503, MATERIAL.tileDoor],
  [Tile.Entrance]: [504, MATERIAL.tileFloor],
  [Tile.Exit]: [505, MATERIAL.tileFloor],
  [Tile.Trap]: [506, MATERIAL.tileFloor],
};

export function tileRequest(tile: Tile): RenderRequest {
  const [id, mat] = TILE_SPRITES[tile] ?? [500, MATERIAL.tileFloor];
  return renderRequest(layers([[0, id, mat, RAMP.stone]], RAMP.cold));
}
