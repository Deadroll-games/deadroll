import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { MONSTER_DEFENSE, playerDefense, unitStats } from "../../chain/stats";
import { Aspect, Class, ModKind } from "../../chain/types";
import { StatsPanel } from "./StatsPanel";

afterEach(cleanup);

const mod = (kind: ModKind, powerBp: number, aspect: Aspect = Aspect.Fire) => ({ kind, aspect, turns: 3, powerBp, source: 5 });

describe("StatsPanel", () => {
  it("renders nothing without units", () => {
    const { container } = render(<StatsPanel units={[]} />);
    expect(container.innerHTML).toBe("");
  });

  it("shows base -> final with the mod that moved it", () => {
    const mira = unitStats({ hp: 18, hpMax: 24, apBase: 3, apNow: 4, profile: playerDefense(Class.Support, [5, 7, 13, 5], [2, 0xff, 0xff, 5, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]), mods: [mod(ModKind.Haste, 3_400), mod(ModKind.Protection, 2_000)] });
    const wolf = unitStats({ hp: 8, hpMax: 15, apBase: 6, apNow: null, profile: MONSTER_DEFENSE[0]!, mods: [mod(ModKind.Vulnerability, 2_500), mod(ModKind.Weakening, 1_800)] });
    render(
      <StatsPanel
        units={[
          { unit: 1, label: "Mira", kind: "player", current: true, stats: mira },
          { unit: 4, label: "Wolf 0", kind: "monster", stats: wolf },
        ]}
      />,
    );
    const [p, w] = screen.getAllByTestId("stats-row");
    expect(p!.textContent).toContain("HP 18/24");
    expect(p!.textContent).toContain("AP 3 -> 4 (Haste)");
    expect(p!.textContent).toContain("now 4");
    expect(p!.textContent).toContain("Def 11 · Armor 5");
    expect(p!.textContent).toContain("Resist all aspects 0% -> 20% (Protection)");
    expect(w!.textContent).toContain("AP 6");
    expect(w!.textContent).toContain("Resist Cold 50%");
    expect(w!.textContent).toContain("Vuln Fire 0% -> 25% (Vulnerability)");
    expect(w!.textContent).toContain("Weaken 18% (Weakening)");
    expect(w!.textContent).toContain("Vulnerability +25% Fire damage taken (2)");
    expect(w!.textContent).not.toContain("Vuln Cold");
  });
});
