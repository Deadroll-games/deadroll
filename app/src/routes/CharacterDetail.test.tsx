import { cleanup, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Address } from "@solana/kit";
import { variantLabel, type CharacterAcc, type SkillRecordAcc } from "../chain/game";
import { CharacterDetailView, EQUIPMENT_SLOTS, skillAspects, usableInCombat, type PartyInfo } from "./CharacterDetail";

vi.mock("../raster/RasterizerProvider", () => ({ useRenderImage: () => ({ image: null, error: null }) }));

afterEach(cleanup);

const ADDR = "Char111111111111111111111111111111111111111" as Address;
const SESSION = "Sess111111111111111111111111111111111111111" as Address;
const text = (s: string) => {
  const out = new Uint8Array(24);
  out.set(new TextEncoder().encode(s));
  return out;
};
const e = (k: string) => ({ [k]: {} });

function skill(name: string, over: Partial<SkillRecordAcc["blocks"]> = {}, sharesBp = [10_000, 0, 0]): SkillRecordAcc {
  return {
    blocks: {
      category: e("active"),
      purpose: e("damage"),
      source: e("magical"),
      form: e("projectile"),
      target: e("single"),
      aspects: [e("cold"), e("fire"), e("fire")],
      aspectCount: 1,
      effect: e("none"),
      trigger: e("none"),
      auraRadius: 0,
      ...over,
    },
    sharesBp,
    powerBp: 10_000,
    fusion: 0,
    cooldown: 1,
    name: text(name),
  };
}

function character(over: Partial<CharacterAcc["core"]> = {}, frozen = false): CharacterAcc {
  return {
    core: {
      owner: ADDR,
      index: 0,
      nft: null,
      race: e("forestElf"),
      class: e("mage"),
      level: 2,
      xp: 140,
      stats: [5, 8, 12, 5],
      predisposition: e("cold"),
      hpMax: 30,
      alive: true,
      inSession: null,
      appearance: [],
      stateVersion: 0,
      ...over,
    },
    name: new TextEncoder().encode("Aria"),
    unspentPoints: 0,
    nftFrozen: frozen,
  };
}

const view = (c: CharacterAcc, skills: SkillRecordAcc[], party: PartyInfo = null) =>
  render(
    <MemoryRouter>
      <CharacterDetailView address={ADDR} c={c} skills={skills} party={party} />
    </MemoryRouter>,
  );

describe("helpers", () => {
  it("labels enum variants", () => {
    expect(variantLabel(e("enemyAdjacent"))).toBe("Enemy adjacent");
    expect(variantLabel(e("active"))).toBe("Active");
  });

  it("lists used aspects with their shares", () => {
    const s = skill("Frost Burn", { aspectCount: 2 }, [6_500, 3_500, 0]);
    expect(skillAspects(s)).toBe("Cold 65% · Fire 35%");
  });

  it("every Active skill with its effect is usable in combat", () => {
    expect(usableInCombat(skill("a"))).toBe(true);
    expect(usableInCombat(skill("q", { purpose: e("buff"), effect: e("haste") }))).toBe(true);
    expect(usableInCombat(skill("s", { purpose: e("debuff"), effect: e("weakening") }))).toBe(true);
    expect(usableInCombat(skill("b", { category: e("passive"), purpose: e("buff") }))).toBe(false);
  });
});

describe("CharacterDetailView", () => {
  it("shows identity, stats, skills and empty equipment", () => {
    view(character(), [skill("Frost Bolt"), skill("Cold Mind", { category: e("passive"), purpose: e("buff"), effect: e("empowerment") })]);
    expect(screen.getByRole("heading", { name: "Aria" })).toBeTruthy();
    expect(screen.getByText("Forest elf Mage · Cold predisposition")).toBeTruthy();
    expect(screen.getByText("Level 2 · 140 XP")).toBeTruthy();
    expect(screen.getByText("Intelligence").nextSibling?.textContent).toBe("12");
    expect(screen.getByText("30 (max)")).toBeTruthy();
    expect(screen.getByText("Frost Bolt")).toBeTruthy();
    expect(screen.getAllByText(/not usable in combat/)).toHaveLength(1);
    expect(screen.getByText("Empowerment")).toBeTruthy();
    expect(screen.getAllByRole("listitem")).toHaveLength(EQUIPMENT_SLOTS.length);
    expect(screen.getByText("Items coming soon.")).toBeTruthy();
    expect(screen.getByText("Not in a party.")).toBeTruthy();
  });

  it("links the current party with live HP", () => {
    view(character({ inSession: SESSION }), [], { session: SESSION, status: "active", slot: 1, hp: 17 });
    expect(screen.getByText("17 / 30")).toBeTruthy();
    expect(screen.getByText("Alive · in a party")).toBeTruthy();
    expect(screen.getByRole("link", { name: "open party" }).getAttribute("href")).toBe(`/party?s=${SESSION}`);
  });

  it("shows a tombstone for a dead character", () => {
    view(character({ alive: false }), [], null);
    expect(screen.getByText("Tombstone · freeze pending")).toBeTruthy();
    cleanup();
    view(character({ alive: false }, true), [], null);
    expect(screen.getByText("Tombstone · NFT frozen")).toBeTruthy();
    expect(screen.getByText("0 / 30")).toBeTruthy();
  });
});
