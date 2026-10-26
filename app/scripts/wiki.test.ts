// The wiki is built from the repo at site build time; new content without its text fails the build.
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { buildWiki, parseEnums, parseHitSteps, parseSprites, parseStructDocs, WikiError } from "./wiki";

const ROOT = resolve(import.meta.dirname, "../..");

describe("wiki generator", () => {
  it("builds from the repo: every aspect, state, resonance, monster and monster skill has its text", () => {
    const w = buildWiki(ROOT);
    expect(w.aspects.map((a) => a.name)).toEqual(["Fire", "Cold", "Lightning", "Blood", "Blunt", "Piercing"]);
    expect(w.states.every((s) => s.about.length > 0)).toBe(true);
    expect(w.resonances.filter((r) => r.name).every((r) => r.about)).toBe(true);
    expect(w.bestiary.length).toBeGreaterThan(0);
    // Every monster sprite but the tombstone, with its exported picture.
    expect(w.sprites.some((s) => s.slug === "tombstone")).toBe(false);
    expect(w.sprites.find((s) => s.id === 400)).toEqual({ id: 400, slug: "wolf", name: "Wolf", art: "bestiary/400_wolf" });
    expect(w.hitSteps[0]?.steps).toBe("3");
  });

  it("fails on a new aspect without a text", () => {
    expect(() => buildWiki(ROOT, undefined, { "aspects.json": { Fire: "x" } })).toThrow(WikiError);
    expect(() => buildWiki(ROOT, undefined, { "aspects.json": { Fire: "x" } })).toThrow(/aspects.json: "Cold"/);
  });

  it("fails on a monster skill, a monster, a state or a resonance without a text", () => {
    for (const [file, key] of [
      ["monster_skills.json", "Lunge"],
      ["monsters.json", "wolf"],
      ["states.json", "Burning"],
      ["resonances.json", "Pale Flame"],
    ] as const) {
      expect(() => buildWiki(ROOT, undefined, { [file]: {} })).toThrow(new RegExp(`${file}: "${key}"`));
    }
  });

  it("fails on a monster sprite without its picture", () => {
    expect(() => buildWiki(ROOT, resolve(ROOT, "app/assets/ui"))).toThrow(/bestiary\/400_wolf.png/);
  });

  it("parses the Rust sources", () => {
    expect(parseEnums("u8_enum!(\n    /// Doc.\n    Pet { Cat, Dog }\n);\nu8_enum!(One { A });")).toEqual({ Pet: { doc: "Doc.", variants: ["Cat", "Dog"] }, One: { doc: "", variants: ["A"] } });
    expect(parseStructDocs("pub struct T {\n    /// The a.\n    pub a: u8,\n    pub b: u8,\n}\n", "T")).toEqual({ a: "The a.", b: "" });
    expect(parseHitSteps("pub fn resolve_hit_into() {\n    // Step 3: one.\n    x();\n    // Steps 4-5: two,\n    // more.\n}\n")).toEqual([
      { steps: "3", text: "one." },
      { steps: "4-5", text: "two, more." },
    ]);
    expect(parseSprites("# c\nsprite 400 wolf\nsize 96 96\n")).toEqual([{ id: 400, slug: "wolf" }]);
  });
});
