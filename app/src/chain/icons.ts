// Icons of bag and cache rewards (32x32 sprites 600..799, assets/sprites/resources). One sprite per
// kind of thing; the material, accent and element ramps color it (tools/art keeps the default
// colors per sprite, `ICON_LOOKS`). Resource ids 0..7 have no names yet: they borrow the first
// monster resources until the resource registry exists.
import { lookRequest, MATERIAL, RAMP, type Look } from "./appearance";
import type { Reward } from "./loot";
import type { RenderRequest } from "./types";

export const ICON = {
  pelt: 600, fang: 601, sinew: 602, claw: 603, fat: 604, log: 605, bark: 606, resin: 607, heartwood: 608,
  pollen: 609, stone: 610, wing: 611, scrap: 612, rags: 613, trinket: 614, bone: 615, vial: 616,
  ore: 650, ingot: 651, dualStone: 652, recipe: 680, blank: 681, reagent: 682, potion: 690, food: 691, torch: 692,
} as const;

/** Resource id -> icon (temporary: wolf pelt, fangs, sinews, bear claws, bear fat, scrap metal, scraps, trinkets). */
const RESOURCES: Look[] = [
  [ICON.pelt, MATERIAL.pelt, RAMP.leather, RAMP.cold],
  [ICON.fang, MATERIAL.bone, RAMP.gold, RAMP.cold],
  [ICON.sinew, MATERIAL.bone, RAMP.gold, RAMP.cold],
  [ICON.claw, MATERIAL.bone, RAMP.gold, RAMP.cold],
  [ICON.fat, MATERIAL.bone, RAMP.rust, RAMP.cold],
  [ICON.scrap, MATERIAL.steel, RAMP.rust, RAMP.cold],
  [ICON.rags, MATERIAL.cloth, RAMP.gold, RAMP.cold],
  [ICON.trinket, MATERIAL.bone, RAMP.gold, RAMP.cold],
];

/** Reward kind (`REWARD_KINDS`: resource, recipe, consumable, blank, reagent) -> the generic icon. */
const BY_KIND: Record<number, Look> = {
  1: [ICON.recipe, MATERIAL.bone, RAMP.capeRed, RAMP.cold],
  2: [ICON.potion, MATERIAL.leather, RAMP.steel, RAMP.blood],
  3: [ICON.blank, MATERIAL.stone, RAMP.gold, RAMP.glowSoul],
  4: [ICON.reagent, MATERIAL.tileDoor, RAMP.steel, RAMP.glowToxic],
};

/** The render request of a reward's icon, null when it has none. */
export function iconRequest(r: Reward): RenderRequest | null {
  const look = r.kind === 0 ? RESOURCES[r.id] : BY_KIND[r.kind];
  return look ? lookRequest(look) : null;
}
