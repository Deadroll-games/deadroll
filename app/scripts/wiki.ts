// The wiki and the bestiary, built from the repo when the site is built (no RPC): the enums of
// crates/types with their doc comments, the hit order as resolve_hit runs it, the BalanceConfig
// numbers (app/src/chain/balance.json, kept fresh by crates/sim/tests/sim.rs) with the doc comments
// of their fields, the registry JSON and the monster sprites. The short texts live next to the data
// in assets/wiki/*.json; a missing one fails the build, so new content reaches the site on its own
// once it has a line there.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { Plugin } from "vite";

export type Named = { name: string; doc: string; about: string | null };
export type FieldDoc = { field: string; doc: string; value: unknown };
export type WikiData = {
  enums: Record<string, { doc: string; variants: Named[] }>;
  aspects: { name: string; family: string; native: string; trait: string; condition: string; about: string }[];
  states: { name: string; turns: number | null; about: string }[];
  traits: { trait: string; aspect: string; epithet: string; hit: unknown; template: unknown[] }[];
  resonances: { base: string; tint: string; name: string | null; condition: string; ops: unknown[]; about: string | null }[];
  hitSteps: { steps: string; text: string }[];
  numbers: { group: string; fields: FieldDoc[] }[];
  monsterSkills: (Record<string, unknown> & { id: number; name: string; about: string })[];
  bestiary: (Record<string, unknown> & { id: number; name: string; about: string })[];
  sprites: { id: number; slug: string; name: string; art: string }[];
};

/** Enums the wiki shows, by the file that declares them. */
const ENUMS = {
  "crates/types/src/enums.rs": ["Aspect", "Family", "StateKind", "Source", "Form", "TimeTag", "RangeTag", "Race", "Class", "Biome"],
  "crates/types/src/skill.rs": ["Category", "Purpose", "Target", "Effect", "Trigger"],
};
/** Tag enums described in assets/wiki/tags.json. */
const TAGS = ["Source", "Form", "TimeTag", "RangeTag", "Target", "Category", "Purpose", "Effect", "Trigger"];
/** Rust structs whose fields are the balance.json groups. */
const NUMBER_GROUPS: { group: string; json: string; file: string; struct: string }[] = [
  { group: "Hit math", json: "combat", file: "crates/combat/src/balance.rs", struct: "Balance" },
  { group: "Heroes: action points, initiative, reach", json: "players", file: "crates/rules/src/config.rs", struct: "PlayerTuning" },
  { group: "Skills: shares, power, cooldowns, action points", json: "skills", file: "crates/skillcheck/src/derive.rs", struct: "SkillTuning" },
];
/** Monster sprites that are not monsters. */
const NOT_MONSTERS = new Set(["tombstone"]);

const read = (root: string, rel: string) => readFileSync(join(root, rel), "utf8");
const json = (root: string, rel: string) => JSON.parse(read(root, rel));

/** `u8_enum!(/// doc ... Name { A, B, C })` declarations: name -> doc and variants. */
export function parseEnums(src: string): Record<string, { doc: string; variants: string[] }> {
  const out: Record<string, { doc: string; variants: string[] }> = {};
  const re = /u8_enum!\(\s*((?:\/\/\/[^\n]*\n\s*)*)(\w+)\s*\{([^}]*)\}\s*\)/g;
  for (const m of src.matchAll(re)) {
    const doc = docText(m[1] ?? "");
    const variants = (m[3] ?? "")
      .split(",")
      .map((v) => v.replace(/\/\/[^\n]*/g, "").trim())
      .filter(Boolean);
    out[m[2]!] = { doc, variants };
  }
  return out;
}

/** The `///` doc comment of each field of `pub struct name`. */
export function parseStructDocs(src: string, name: string): Record<string, string> {
  const start = src.search(new RegExp(`pub struct ${name}\\s*\\{`));
  if (start < 0) throw new Error(`no struct ${name}`);
  const body = src.slice(src.indexOf("{", start) + 1, src.indexOf("\n}", start));
  const out: Record<string, string> = {};
  let doc: string[] = [];
  for (const line of body.split("\n")) {
    const t = line.trim();
    if (t.startsWith("///")) doc.push(t.slice(3).trim());
    else if (t.startsWith("pub ")) {
      const field = /^pub (\w+):/.exec(t)?.[1];
      if (field) out[field] = doc.join(" ");
      doc = [];
    } else if (t && !t.startsWith("#[") && !t.startsWith("//")) doc = [];
  }
  return out;
}

/** The step comments of `resolve_hit_into`, in the order the code runs them. */
export function parseHitSteps(src: string): { steps: string; text: string }[] {
  const start = src.indexOf("pub fn resolve_hit_into(");
  if (start < 0) throw new Error("no resolve_hit_into in crates/combat/src/hit.rs");
  const end = src.indexOf("\n}\n", start);
  const body = src.slice(start, end);
  const out: { steps: string; text: string }[] = [];
  for (const m of body.matchAll(/\/\/ Steps? ([\d-]+): ([^\n]*(?:\n\s*\/\/ (?!Step)[^\n]*)*)/g)) {
    out.push({ steps: m[1]!, text: m[2]!.replace(/\n\s*\/\/ /g, " ").trim() });
  }
  return out;
}

/** `sprite <id> <slug>` headers of the monster sprites. */
export function parseSprites(src: string): { id: number; slug: string }[] {
  return [...src.matchAll(/^sprite (\d+) (\w+)/gm)].map((m) => ({ id: Number(m[1]), slug: m[2]! }));
}

function docText(block: string): string {
  return block
    .split("\n")
    .map((l) => l.trim().replace(/^\/\/\/\s?/, ""))
    .filter(Boolean)
    .join(" ");
}

const title = (slug: string) => slug.split("_").map((w) => w[0]!.toUpperCase() + w.slice(1)).join(" ");

export class WikiError extends Error {}

/** Everything the wiki and the bestiary show. Throws a `WikiError` listing every missing text. */
export function buildWiki(root: string, art = join(root, "app/assets/bestiary"), texts: Record<string, Record<string, unknown>> = {}): WikiData {
  const missing: string[] = [];
  /** `texts` replaces a file of assets/wiki (tests). */
  const textsOf = (file: string) => texts[file] ?? (json(root, `assets/wiki/${file}`) as Record<string, unknown>);
  const about = (file: string, key: string, required: boolean): string | null => {
    const texts = textsOf(file);
    const v = texts[key];
    if (typeof v === "string" && v.trim()) return v;
    if (required) missing.push(`assets/wiki/${file}: "${key}"`);
    return null;
  };

  const enums: WikiData["enums"] = {};
  const tags = textsOf("tags.json") as Record<string, Record<string, string>>;
  for (const [file, names] of Object.entries(ENUMS)) {
    const parsed = parseEnums(read(root, file));
    for (const name of names) {
      const e = parsed[name];
      if (!e) throw new WikiError(`${file}: no u8_enum ${name}`);
      enums[name] = { doc: e.doc, variants: e.variants.map((v) => ({ name: v, doc: "", about: TAGS.includes(name) ? (tags[name]?.[v] ?? null) : null })) };
    }
  }

  const balance = json(root, "app/src/chain/balance.json");
  const aspectRecords = json(root, "assets/registry/aspects.json").records as { aspect: string; family: string; native: string; trait: string; condition: string }[];
  const aspects = enums.Aspect!.variants.map((v) => {
    const r = aspectRecords.find((a) => a.aspect === v.name);
    if (!r) missing.push(`assets/registry/aspects.json: no record for ${v.name}`);
    return { name: v.name, family: r?.family ?? "", native: r?.native ?? "", trait: r?.trait ?? "", condition: r?.condition ?? "", about: about("aspects.json", v.name, true) ?? "" };
  });
  const turns = balance.combat.state_turns as number[];
  const states = enums.StateKind!.variants.map((v, i) => ({ name: v.name, turns: turns[i] ?? null, about: about("states.json", v.name, true) ?? "" }));

  const resonances = (json(root, "assets/registry/resonances.json").records as { base: string; tint: string; name: string | null; condition: string; ops: unknown[] }[])
    .filter((r) => r.base !== r.tint)
    .map((r) => ({ base: r.base, tint: r.tint, name: r.name, condition: r.condition, ops: r.ops, about: r.name ? about("resonances.json", r.name, true) : null }));
  const traits = json(root, "assets/registry/traits.json").records;

  const steps = parseHitSteps(read(root, "crates/combat/src/hit.rs"));
  const numbers = NUMBER_GROUPS.map((g) => {
    const docs = parseStructDocs(read(root, g.file), g.struct);
    const values = balance[g.json] as Record<string, unknown>;
    return { group: g.group, fields: Object.entries(values).map(([field, value]) => ({ field, value, doc: docs[field] ?? "" })) };
  });

  const monsterSkills = (json(root, "assets/registry/monster_skills.json").records as { id: number; name: string }[]).map((s) => ({
    ...s,
    about: about("monster_skills.json", s.name, true) ?? "",
  }));
  const bestiary = (json(root, "assets/registry/bestiary.json").records as { id: number; name: string }[]).map((m) => ({
    ...m,
    about: about("monsters.json", m.name, true) ?? "",
  }));

  const dir = join(root, "assets/sprites/monsters");
  const sprites = readdirSync(dir)
    .filter((f) => f.endsWith(".dsl"))
    .flatMap((f) => parseSprites(readFileSync(join(dir, f), "utf8")))
    .filter((s) => !NOT_MONSTERS.has(s.slug))
    .sort((a, b) => a.id - b.id)
    .map((s) => {
      const file = `${s.id}_${s.slug}`;
      if (!existsSync(join(art, `${file}.png`))) missing.push(`app/assets/bestiary/${file}.png (run tools/art/export_site.sh)`);
      return { id: s.id, slug: s.slug, name: title(s.slug), art: `bestiary/${file}` };
    });

  if (missing.length) throw new WikiError(`wiki: missing text or art for new content:\n  ${missing.join("\n  ")}`);
  return { enums, aspects, states, traits, resonances, hitSteps: steps, numbers, monsterSkills, bestiary, sprites };
}

const VIRTUAL = "virtual:wiki";

/** `import wiki from "virtual:wiki"`: the data above, built when the bundle (or the dev server) starts. */
export function wikiPlugin(root: string): Plugin {
  return {
    name: "deadroll-wiki",
    resolveId: (id) => (id === VIRTUAL ? `\0${VIRTUAL}` : null),
    load(id) {
      if (id !== `\0${VIRTUAL}`) return null;
      return `export default ${JSON.stringify(buildWiki(root))};`;
    },
  };
}
