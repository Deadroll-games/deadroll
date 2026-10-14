import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HeroSheetPanel } from "./HeroSheetPanel";
import type { HeroSheetPanelProps, HeroSheetView } from "./types";

afterEach(cleanup);

const SHEET: HeroSheetView = {
  accuracy: 3,
  evasion: 11,
  armor: 30,
  critBp: 0,
  hpMax: 46,
  actionPoints: 4,
  initiative: 2,
  martialStrikeBp: 2_000,
  martialProjectileBp: 1_400,
  magicalBp: 1_000,
  martialStateBp: 3_000,
  magicalStateBp: 1_500,
  tempoStateBp: 2_100,
  selfBuffTurns: 1,
};

function props(over: Partial<HeroSheetPanelProps> = {}): HeroSheetPanelProps {
  return {
    stats: [10, 7, 5, 8],
    plan: [0, 0, 0, 0],
    unspent: 0,
    sheet: SHEET,
    planned: SHEET,
    locked: null,
    busy: false,
    onPlan: vi.fn(),
    onReset: vi.fn(),
    onSave: vi.fn(),
    ...over,
  };
}

const row = (label: string) => screen.getByText(label).closest("tr")!.textContent;

describe("HeroSheetPanel", () => {
  it("shows every derived stat with where it comes from", () => {
    render(<HeroSheetPanel {...props()} />);
    expect(row("Accuracy")).toContain("+3");
    expect(row("Evasion")).toContain("11");
    expect(row("Crit chance")).toContain("0%");
    expect(row("HP")).toContain("46");
    expect(row("Action points")).toContain("4");
    expect(row("Initiative")).toContain("2");
    expect(row("Martial state chance")).toContain("+30%");
    // No points: no plan buttons.
    expect(screen.queryByRole("button", { name: "More Strength" })).toBeNull();
  });

  it("plans points with + and -, shows the new derived numbers, saves them all at once", () => {
    const onPlan = vi.fn();
    const onSave = vi.fn();
    const planned = { ...SHEET, hpMax: 50, martialStrikeBp: 2_400 };
    render(<HeroSheetPanel {...props({ unspent: 2, plan: [2, 0, 0, 0], planned, onPlan, onSave })} />);
    expect(screen.getByTestId("stat-Strength").textContent).toContain("12");
    expect(screen.getByText("0 of 2 points left to place. Save writes the whole choice in one transaction.")).toBeTruthy();
    // Every point placed: + is off, - is on.
    expect((screen.getByRole("button", { name: "More Dexterity" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Less Strength" }));
    expect(onPlan).toHaveBeenCalledWith(0, -1);
    expect(row("HP")).toContain("46 → 50");
    expect(row("Martial strike damage")).toContain("+20% → +24%");
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onSave).toHaveBeenCalledOnce();
  });

  it("nothing planned: Save and Reset are off", () => {
    render(<HeroSheetPanel {...props({ unspent: 2 })} />);
    expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Reset" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("locked: says why, no buttons", () => {
    render(<HeroSheetPanel {...props({ unspent: 2, locked: "In a party: spend them after it ends." })} />);
    expect(screen.getByText("2 unspent points. In a party: spend them after it ends.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
    expect(screen.queryByRole("button", { name: "More Strength" })).toBeNull();
  });

  it("waits for the combat module", () => {
    render(<HeroSheetPanel {...props({ sheet: null, planned: null })} />);
    expect(screen.getByText("Loading the combat module…")).toBeTruthy();
  });
});
