// Site art (app/assets, exported by tools/art/export_site.sh at one pixel per pixel): the city,
// interiors, menu icons, the parchment frame, loading screens and the bestiary. Never game state:
// sprites of the game itself come from the chain.
const FILES = import.meta.glob("../../assets/**/*.png", { eager: true, query: "?url", import: "default" }) as Record<string, string>;

const BY_NAME = new Map<string, string>();
const BY_ID = new Map<number, string>();
for (const [path, url] of Object.entries(FILES)) {
  const file = path.slice(path.lastIndexOf("/") + 1, -".png".length);
  const folder = path.slice("../../assets/".length, path.lastIndexOf("/"));
  BY_NAME.set(`${folder}/${file}`, url);
  const id = /^(\d+)_/.exec(file);
  if (id) BY_ID.set(Number(id[1]), url);
}

/** `folder/name` without the extension, e.g. `city/tavern`, `screens/parchment`. */
export function artUrl(name: string): string | undefined {
  return BY_NAME.get(name);
}

/** A site sprite by its id (`2000_city_street_0.png` is 2000). */
export function spriteUrl(id: number): string | undefined {
  return BY_ID.get(id);
}

/** Menu icons (assets/site_sprites/ui/ui.dsl). */
export const ICONS = {
  bag: 2200,
  map: 2201,
  skills: 2202,
  settings: 2203,
  characters: 2204,
  city: 2205,
} as const;

/** The planks nailed over the door of a closed building. */
export const BOARDS_SPRITE = 2009;

/** Loading screen of a map biome (`Biome`: Caves, Forest, Crypt); an unknown one draws Caves. */
export function loadingScreen(biome: number): string | undefined {
  const name = ["caves", "forest", "crypt"][biome] ?? "caves";
  return artUrl(`screens/loading_${name}`);
}
