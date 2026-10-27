import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { EquipSource } from "@deadroll/sdk/character";
import { LAYER, MATERIAL, RAMP, TOMBSTONE, characterAppearance, isCurrentAppearance, pointLook, tileLook, tileVariants } from "./appearance";
import { EMPTY_ITEM, EQUIP_SLOTS, ITEM_BASES, ITEM_SPRITE_BASE, characterRequest, dressed, starterEquipment } from "./items";
import type { CharacterAcc } from "./game";
import { Aspect, Biome, Class, Race, TILE_COUNT, Tile } from "./types";

const slot = (name: string) => EQUIP_SLOTS.indexOf(name);
const base = (name: string) => ITEM_BASES.indexOf(name);

describe("characterAppearance", () => {
  it("holds the body as one layer, its hair and the shared front fist, and no items", () => {
    for (const [race, body, hair, material] of [
      [Race.Human, 10, 11, MATERIAL.human],
      [Race.ForestElf, 12, 13, MATERIAL.elf],
    ] as const) {
      const a = characterAppearance(race, Aspect.Fire);
      expect(a[LAYER.body]).toMatchObject({ spriteId: body, materialRamp: material });
      expect(a[LAYER.hair]).toMatchObject({ spriteId: hair, materialRamp: material });
      expect(a[LAYER.fistFront]).toMatchObject({ spriteId: 14, materialRamp: material });
      for (const l of [LAYER.mainhand, LAYER.armorBody, LAYER.offhand, LAYER.helmet, LAYER.pants, LAYER.boots, LAYER.backItem]) {
        expect(a[l]!.spriteId).toBe(0);
      }
    }
  });

  it("keeps the shield over the armor and the hair, the fist over the weapon", () => {
    expect(LAYER.armorBody).toBeLessThan(LAYER.offhand);
    expect(LAYER.hair).toBeLessThan(LAYER.offhand);
    expect(LAYER.mainhand).toBeLessThan(LAYER.fistFront);
    expect(LAYER.fistFront).toBeLessThan(LAYER.gloveFront);
  });
});

describe("dressed", () => {
  it("wears each class starter kit: one sprite per item base, 100 + ItemBase", () => {
    for (const [cls, weapon, armor] of [
      [Class.Warrior, "sword", "leatherArmor"],
      [Class.Mage, "staff", "robe"],
      [Class.Support, "wand", "lightLeatherArmor"],
    ] as const) {
      const eq = starterEquipment(cls);
      expect(eq[slot("mainHand")]).toBe(base(weapon));
      expect(eq[slot("body")]).toBe(base(armor));
      const a = dressed(characterAppearance(Race.ForestElf, Aspect.Cold), eq);
      expect(a[LAYER.mainhand]!.spriteId).toBe(ITEM_SPRITE_BASE + base(weapon));
      expect(a[LAYER.armorBody]!.spriteId).toBe(ITEM_SPRITE_BASE + base(armor));
      expect(a[LAYER.body]!.spriteId).toBe(12);
      expect(a.filter((l) => l.spriteId !== 0)).toHaveLength(6);
    }
  });

  it("draws only what is worn: unequipping clears the layer, equipping fills it", () => {
    const body = characterAppearance(Race.Human, Aspect.Fire);
    const eq = starterEquipment(Class.Warrior);
    eq[slot("body")] = EMPTY_ITEM;
    const noArmor = dressed(body, eq);
    expect(noArmor[LAYER.armorBody]!.spriteId).toBe(0);
    expect(noArmor[LAYER.mainhand]!.spriteId).toBe(ITEM_SPRITE_BASE + base("sword"));
    eq[slot("body")] = base("robe");
    expect(dressed(body, eq)[LAYER.armorBody]!.spriteId).toBe(ITEM_SPRITE_BASE + base("robe"));
    const naked = dressed(body, EQUIP_SLOTS.map(() => EMPTY_ITEM));
    expect(naked.filter((l) => l.spriteId !== 0)).toHaveLength(4);
  });
});

describe("characterRequest", () => {
  const char = (alive: boolean, worn: Record<string, string>): CharacterAcc => {
    const bases = Object.values(worn);
    return {
      core: { alive, appearance: characterAppearance(Race.Human, Aspect.Fire) },
      gear: {
        items: bases.map((b) => ({ base: base(b), level: 1 })),
        itemCount: bases.length,
        equipped: EQUIP_SLOTS.map((s) => {
          const i = Object.keys(worn).indexOf(s);
          return i < 0 ? { source: EquipSource.Empty, index: 0, asset: "" } : { source: EquipSource.Soulbound, index: i, asset: "" };
        }),
      },
    } as unknown as CharacterAcc;
  };

  it("dresses a living character in its gear and shows a tombstone once dead", () => {
    const r = characterRequest(char(true, { mainHand: "wand", body: "lightLeatherArmor" }));
    expect(r.layers[LAYER.mainhand]!.spriteId).toBe(ITEM_SPRITE_BASE + base("wand"));
    expect(r.layers[LAYER.armorBody]!.spriteId).toBe(ITEM_SPRITE_BASE + base("lightLeatherArmor"));
    expect(characterRequest(char(false, { mainHand: "wand" }))).toBe(TOMBSTONE);
  });
});

describe("isCurrentAppearance", () => {
  it("accepts the bodies and refuses appearances from before the one-piece bodies", () => {
    for (const race of [Race.Human, Race.ForestElf]) {
      expect(isCurrentAppearance(characterAppearance(race, Aspect.Cold))).toBe(true);
    }
    // Patch 7 and older: arm_back at slot 2, legs 5, torso 8, head 11, hair 12, arm_front 14.
    const old = Array.from({ length: 20 }, () => ({ spriteId: 0, materialRamp: 1, accentRamp: 13, elementRamp: 0 }));
    for (const [s, id] of [[2, 2], [5, 3], [8, 4], [11, 5], [12, 6], [14, 7]] as const) old[s] = { ...old[s]!, spriteId: id };
    expect(isCurrentAppearance(old)).toBe(false);
    expect(isCurrentAppearance(old.map((l, i) => (i === 2 ? { ...l, spriteId: 20 } : l)))).toBe(false);
    expect(isCurrentAppearance([])).toBe(false);
  });
});

describe("palette indices", () => {
  const pal = readFileSync(resolve(import.meta.dirname, "../../../assets/palette/palette_2.pal"), "utf8");
  const names = (kind: string) =>
    pal
      .split("\n")
      .filter((l) => l.startsWith(kind + " "))
      .map((l) => l.split(/\s+/)[1]!.replace(/_(\w)/g, (_, c: string) => c.toUpperCase()));

  it("RAMP and MATERIAL follow the ramp and material lines of palette_2.pal", () => {
    const ramps = names("ramp");
    for (const [name, i] of Object.entries(RAMP)) expect(ramps[i], name).toBe(name);
    const materials = names("material");
    for (const [name, i] of Object.entries(MATERIAL)) expect(materials[i], name).toBe(name);
  });
});

describe("tiles by biome", () => {
  /** `sprite <id> <name>` lines of a DSL file in assets/sprites. */
  const spritesOf = (file: string) =>
    new Map(
      [...readFileSync(resolve(import.meta.dirname, "../../../assets/sprites", file), "utf8").matchAll(/^sprite (\d+) (\w+)/gm)].map((m) => [Number(m[1]), m[2]!]),
    );
  const books = { [Biome.Caves]: spritesOf("tiles/caves/caves.dsl"), [Biome.Forest]: spritesOf("tiles/forest/forest.dsl"), [Biome.Crypt]: spritesOf("tiles/crypt/crypt.dsl") };
  const points = spritesOf("points/points.dsl");
  const cell = (tile: Tile, variant = 0, mask = 0) => ({ tile, variant, mask });

  it("every cell of every biome is a sprite of that biome's set", () => {
    for (const biome of [Biome.Caves, Biome.Forest, Biome.Crypt]) {
      for (let tile = 0; tile < TILE_COUNT; tile++) {
        const masks = tile === Tile.Wall ? 16 : 1;
        for (let variant = 0; variant < 8; variant++) {
          for (let mask = 0; mask < masks; mask++) {
            const look = tileLook(biome, cell(tile as Tile, variant, mask));
            expect(look && books[biome].has(look[0]), `biome ${biome} tile ${tile} v${variant} m${mask}`).toBe(true);
          }
        }
      }
    }
  });

  it("walls by the neighbor mask: the 16 shapes in order", () => {
    expect(Array.from({ length: 16 }, (_, m) => books[Biome.Forest].get(tileLook(Biome.Forest, cell(Tile.Wall, 0, m))![0]))).toEqual(
      Array.from({ length: 16 }, (_, m) => `forest_wall_${m}`),
    );
    // Caves: mask 10 (east and west) is the older straight wall, the rest follow the comments.
    expect(tileLook(Biome.Caves, cell(Tile.Wall, 0, 10))![0]).toBe(501);
    expect(books[Biome.Caves].get(tileLook(Biome.Caves, cell(Tile.Wall, 0, 11))![0])).toBe("cave_wall_11");
    expect(tileLook(Biome.Crypt, cell(Tile.Wall, 0, 15))![0]).toBe(815);
    // The variant does not change a wall.
    expect(tileLook(Biome.Crypt, cell(Tile.Wall, 5, 3))).toEqual(tileLook(Biome.Crypt, cell(Tile.Wall, 0, 3)));
  });

  it("the variant picks the picture; one the biome lacks draws variant 0", () => {
    expect(tileVariants(Biome.Caves, Tile.StoneFloor)).toBe(4);
    expect([0, 1, 2, 3].map((v) => tileLook(Biome.Caves, cell(Tile.StoneFloor, v))![0])).toEqual([500, 522, 523, 524]);
    expect(tileLook(Biome.Forest, cell(Tile.WoodenFloor, 2))![0]).toBe(562);
    expect(tileLook(Biome.Forest, cell(Tile.WoodenFloor, 7))).toEqual(tileLook(Biome.Forest, cell(Tile.WoodenFloor, 0)));
    expect([tileVariants(Biome.Crypt, Tile.Wall), tileVariants(Biome.Crypt, Tile.Door), tileVariants(Biome.Crypt, Tile.ShallowWater)]).toEqual([1, 1, 2]);
  });

  it("colors follow tools/art TILE_LOOKS", () => {
    expect(tileLook(Biome.Caves, cell(Tile.Dirt))).toEqual([528, MATERIAL.caveFloor, RAMP.bone, RAMP.murk]);
    expect(tileLook(Biome.Forest, cell(Tile.Dirt, 1))).toEqual([564, MATERIAL.forestGrass, RAMP.rust, RAMP.bone]);
    expect(tileLook(Biome.Forest, cell(Tile.ShallowWater))).toEqual([567, MATERIAL.murk, RAMP.moss, RAMP.cold]);
    expect(tileLook(Biome.Crypt, cell(Tile.Door))).toEqual([826, MATERIAL.cryptDoor, RAMP.steel, RAMP.cold]);
  });

  it("the entrance by biome, the portal exit everywhere; an unknown biome draws Caves", () => {
    expect([Biome.Caves, Biome.Forest, Biome.Crypt].map((b) => points.get(pointLook("entrance", b)[0]))).toEqual(["tile_entrance", "forest_entrance", "crypt_entrance"]);
    expect(points.get(pointLook("exit", Biome.Crypt)[0])).toBe("tile_exit");
    expect(tileLook(9, cell(Tile.StoneFloor, 1))).toEqual(tileLook(Biome.Caves, cell(Tile.StoneFloor, 1)));
  });
});
