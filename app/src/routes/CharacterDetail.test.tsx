import { cleanup, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Address } from "@solana/kit";
import { variantLabel, type CharacterAcc, type GearAcc, type SkillRecordAcc } from "../chain/game";
import { CharacterDetailView, EQUIPMENT_SLOTS, inventoryLock, inventoryView, skillAspects, usableInCombat, type PartyInfo } from "./CharacterDetail";

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
    gear: gear(),
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
  it("shows identity, stats, skills; the inventory once loaded", () => {
    view(character(), [skill("Frost Bolt"), skill("Cold Mind", { category: e("passive"), purpose: e("buff"), effect: e("empowerment") })]);
    expect(screen.getByRole("heading", { name: "Aria" })).toBeTruthy();
    expect(screen.getByText("Forest elf Mage · Cold predisposition")).toBeTruthy();
    expect(screen.getByText("Level 2 · 140 XP")).toBeTruthy();
    expect(screen.getByText("Intelligence").nextSibling?.textContent).toBe("12");
    expect(screen.getByText("30 (max)")).toBeTruthy();
    expect(screen.getByText("Frost Bolt")).toBeTruthy();
    expect(screen.getAllByText(/not usable in combat/)).toHaveLength(1);
    expect(screen.getByText("Empowerment")).toBeTruthy();
    expect(screen.getByText("Loading items…")).toBeTruthy();
    expect(EQUIPMENT_SLOTS).toHaveLength(11);
    expect(EQUIPMENT_SLOTS[0]).toEqual({ key: "mainHand", label: "Main hand" });
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

function gear(): GearAcc {
  const empty = { source: e("empty"), index: 0, asset: ADDR };
  return {
    items: [
      { base: e("staff"), level: 1 },
      { base: e("robe"), level: 1 },
      { base: e("sword"), level: 0 },
      { base: e("sword"), level: 0 },
    ],
    itemCount: 2,
    equipped: EQUIPMENT_SLOTS.map((s) => (s.key === "mainHand" ? { source: e("soulbound"), index: 0, asset: ADDR } : empty)),
  };
}

describe("inventory", () => {
  it("lists the soulbound items, what they give and where they are worn", () => {
    expect(inventoryView({ ...gear(), itemCount: 0 })).toEqual([]);
    expect(inventoryView(gear())).toEqual([
      { index: 0, base: "staff", label: "Staff", level: 1, bonus: "damage Blunt 3-6", fits: ["mainHand"], worn: "mainHand" },
      { index: 1, base: "robe", label: "Robe", level: 1, bonus: "+2 armor", fits: ["body"], worn: null },
    ]);
  });

  it("only the owner changes it, never in a party or on a tombstone", () => {
    const OTHER = "Othr111111111111111111111111111111111111111" as Address;
    expect(inventoryLock(character(), ADDR)).toBeNull();
    expect(inventoryLock(character(), OTHER)).toMatch(/Only the owner/);
    expect(inventoryLock(character(), null)).toMatch(/Only the owner/);
    expect(inventoryLock(character({ inSession: SESSION }), ADDR)).toMatch(/locked until it ends/);
    expect(inventoryLock(character({ alive: false }), ADDR)).toMatch(/tombstone/);
  });

  it("the panel wears and takes off through its callbacks", () => {
    const onEquip = vi.fn();
    const onUnequip = vi.fn();
    render(
      <MemoryRouter>
        <CharacterDetailView
          address={ADDR}
          c={character()}
          skills={[]}
          party={null}
          items={{ slots: EQUIPMENT_SLOTS, items: inventoryView(gear()), locked: null, busy: false, onEquip, onUnequip }}
        />
      </MemoryRouter>,
    );
    expect(screen.getByTestId("slot-mainHand").textContent).toContain("Staff");
    expect(screen.getByTestId("slot-body").textContent).toContain("empty");
    expect(screen.getByText("not worn")).toBeTruthy();
    screen.getByRole("button", { name: "Wear" }).click();
    expect(onEquip).toHaveBeenCalledWith(1, "body");
    screen.getByRole("button", { name: "Take off" }).click();
    expect(onUnequip).toHaveBeenCalledWith("mainHand");
  });

  it("locked: the buttons are off and say why; a tombstone has no items left", () => {
    render(
      <MemoryRouter>
        <CharacterDetailView
          address={ADDR}
          c={character()}
          skills={[]}
          party={null}
          items={{ slots: EQUIPMENT_SLOTS, items: inventoryView(gear()), locked: "In a party: the equipment is locked until it ends.", busy: false, onEquip: vi.fn(), onUnequip: vi.fn() }}
        />
      </MemoryRouter>,
    );
    expect(screen.getByText(/locked until it ends/)).toBeTruthy();
    expect((screen.getByRole("button", { name: "Take off" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Wear" }) as HTMLButtonElement).disabled).toBe(true);
    cleanup();
    render(
      <MemoryRouter>
        <CharacterDetailView
          address={ADDR}
          c={character({ alive: false })}
          skills={[]}
          party={null}
          items={{ slots: EQUIPMENT_SLOTS, items: [], locked: inventoryLock(character({ alive: false }), ADDR), busy: false, onEquip: vi.fn(), onUnequip: vi.fn() }}
        />
      </MemoryRouter>,
    );
    expect(screen.getByText(/vanished with it/)).toBeTruthy();
    expect(screen.getByText("No items.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Take off" })).toBeNull();
  });
});
