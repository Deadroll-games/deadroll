import { cleanup, fireEvent, render, screen, act } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MOCK_HITS } from "../../chain/mock";
import { Aspect, HitOutcome, ModKind } from "../../chain/types";
import { formatScaled } from "../../chain/names";
import { cascadeSteps, chanceView } from "./cascade";
import { DiceCascade } from "./DiceCascade";
import { HitBreakdown, itemLines, modLines } from "./HitBreakdown";
import Demo from "./Demo";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("cascadeSteps", () => {
  it("miss is only the attack d20 and a miss", () => {
    const steps = cascadeSteps(MOCK_HITS.miss);
    expect(steps.map((s) => s.kind)).toEqual(["d20", "damage"]);
    expect(steps[0]?.success).toBe(false);
    expect(steps[0]?.label).toBe("Attack d20");
    expect(steps[0]?.value).toBe("3");
    expect(steps[0]?.target).toBe("total 7 vs defense 14");
    expect(steps[1]?.label).toBe("Miss");
    expect(steps[1]?.value).toBe("0");
    expect(steps[1]?.success).toBeUndefined();
  });

  it("resonance hit walks tint, resonance, d100, both packets and chill", () => {
    const steps = cascadeSteps(MOCK_HITS.resonance);
    expect(steps.map((s) => s.kind)).toEqual([
      "d20",
      "crit",
      "tint",
      "tint",
      "resonance",
      "d100",
      "packet",
      "packet",
      "state",
      "state",
      "damage",
    ]);
    expect(steps[0]?.success).toBe(true);
    expect(steps[1]?.success).toBe(false);
    expect(steps[2]?.label).toBe("Resonance (Fire tint on Cold, tier 9)");
    expect(steps[2]?.value).toBe("rolled 90.22");
    expect(steps[2]?.target).toBe("needs 82.00+");
    expect(steps[2]?.flag).toBe("RESONANCE!");
    expect(steps[2]?.bar).toEqual({ shown: 9_022, needed: 8_200, success: true });
    expect(steps[2]?.title).toBe("on-chain raw roll 9.77%, shown as 99.99 - raw, passes when raw < 18.00%, carrier Race");
    expect(steps[2]?.success).toBe(true);
    expect(steps[3]?.success).toBe(false);
    expect(steps[4]?.label).toBe("Resonance Cold + Fire");
    expect(steps[4]?.value).toBe("tier 9");
    expect(steps[4]?.success).toBe(true);
    expect(steps[5]?.label).toBe("Damage d100");
    expect(steps[5]?.value).toBe("63");
    expect(steps.filter((s) => s.kind === "packet")).toHaveLength(2);
    expect(steps[6]?.value).toBe(formatScaled(71_685n));
    expect(steps[6]?.target).toBe(
      `rolled ${formatScaled(89_650n)} in ${formatScaled(55_000n)}-${formatScaled(110_000n)}`,
    );
    const chill = steps.find((s) => s.kind === "state" && s.label.startsWith("Chill"));
    expect(chill?.label).toBe("Chill chance added");
    expect(chill?.success).toBe(true);
    expect(chill?.value).toBe("rolled 87.95");
    expect(chill?.target).toBe("needs 65.00+");
    expect(steps[10]?.label).toBe("Damage");
    expect(steps[10]?.value).toBe("9");
  });

  it("crit roll succeeds", () => {
    const steps = cascadeSteps(MOCK_HITS.crit);
    const crit = steps.find((s) => s.kind === "crit");
    expect(crit?.success).toBe(true);
    expect(crit?.label).toBe("Crit");
    expect(crit?.value).toBe("rolled 97.88");
    expect(crit?.flag).toBe("CRIT!");
    expect(crit?.target).toBe("needs ?+");
    expect(cascadeSteps(MOCK_HITS.crit, 500).find((s) => s.kind === "crit")?.target).toBe("needs 95.00+");
    expect(steps.find((s) => s.kind === "state")?.label).toBe("Bleeding chance added");
  });

  it("shown roll agrees with the chain at the boundary (raw < chance passes)", () => {
    const atChance = chanceView(500, 500, false);
    expect(atChance.value).toBe("rolled 94.99");
    expect(atChance.target).toBe("needs 95.00+");
    expect(atChance.bar.shown).toBeLessThan(atChance.bar.needed!);
    const justUnder = chanceView(499, 500, true);
    expect(justUnder.value).toBe("rolled 95.00");
    expect(justUnder.bar.shown).toBe(justUnder.bar.needed);
  });

  it("immune replaces the defense target", () => {
    const steps = cascadeSteps({ ...MOCK_HITS.miss, outcome: HitOutcome.Immune });
    expect(steps[0]?.target).toBe("Immune");
    expect(steps[0]?.success).toBe(true);
    expect(steps[1]?.label).toBe("Damage");
  });
});

describe("DiceCascade", () => {
  it("instant mode calls onDone once and shows the final damage", () => {
    const onDone = vi.fn();
    render(
      <DiceCascade hit={MOCK_HITS.crit} attackerLabel="Aria" targetLabel="Wolf" instant onDone={onDone} />,
    );
    expect(onDone).toHaveBeenCalledTimes(1);
    expect(screen.getByText("Aria → Wolf")).toBeTruthy();
    expect(screen.getByText("16")).toBeTruthy();
    expect(screen.getByText("Attack d20")).toBeTruthy();
  });

  it("does not call onDone again on rerender", () => {
    const onDone = vi.fn();
    const props = {
      hit: MOCK_HITS.miss,
      attackerLabel: "Aria",
      targetLabel: "Wolf",
      instant: true,
      onDone,
    };
    const view = render(<DiceCascade {...props} />);
    view.rerender(<DiceCascade {...props} />);
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it("reveals steps over time and calls onDone after the last one", () => {
    vi.useFakeTimers();
    const onDone = vi.fn();
    render(
      <DiceCascade hit={MOCK_HITS.miss} attackerLabel="Aria" targetLabel="Wolf" stepMs={450} onDone={onDone} />,
    );
    const before = document.querySelectorAll(".dice-step").length;
    expect(before).toBeGreaterThan(0);
    expect(before).toBeLessThan(cascadeSteps(MOCK_HITS.miss).length);
    expect(onDone).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(450);
    });

    const after = document.querySelectorAll(".dice-step").length;
    expect(after).toBeGreaterThan(before);
    expect(after).toBe(cascadeSteps(MOCK_HITS.miss).length);
    const damage = document.querySelector('[data-kind="damage"] .dice-settle');
    expect(damage?.textContent).toBe("0");
    expect(onDone).toHaveBeenCalledTimes(1);

    act(() => {
      vi.advanceTimersByTime(5_000);
    });
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it("shows every step immediately when reduced motion is requested", () => {
    const previous = window.matchMedia;
    window.matchMedia = vi.fn().mockReturnValue({
      matches: true,
      media: "(prefers-reduced-motion: reduce)",
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
      onchange: null,
    }) as unknown as typeof window.matchMedia;
    try {
      const onDone = vi.fn();
      render(<DiceCascade hit={MOCK_HITS.miss} attackerLabel="Aria" targetLabel="Wolf" onDone={onDone} />);
      expect(document.querySelectorAll(".dice-step").length).toBe(2);
      expect(onDone).toHaveBeenCalledTimes(1);
    } finally {
      if (previous) window.matchMedia = previous;
      else Reflect.deleteProperty(window, "matchMedia");
    }
  });
});

describe("HitBreakdown", () => {
  it("renders one packet table per packet", () => {
    const resonance = render(<HitBreakdown hit={MOCK_HITS.resonance} />);
    expect(resonance.container.querySelectorAll('[data-testid="packet"]')).toHaveLength(2);
    expect(resonance.getByText("Hit breakdown")).toBeTruthy();
    expect(resonance.getByText("Cold + Fire, tier 9, Race, second tint none")).toBeTruthy();
    expect(resonance.getByText("Total damage: 9")).toBeTruthy();
    expect(resonance.getByText("Resonance (Fire tint on Cold, tier 9): needs 82.00+, rolled 90.22, RESONANCE!")).toBeTruthy();
    expect(resonance.getByText("Crit: needs ?+, rolled 95.87, fail")).toBeTruthy();
    expect(resonance.getByText("Rolls used: 9")).toBeTruthy();
    expect((resonance.getByText("Hit breakdown").closest("details") as HTMLDetailsElement).open).toBe(true);

    cleanup();
    const crit = render(<HitBreakdown hit={MOCK_HITS.crit} critChanceBp={500} />);
    expect(crit.container.querySelectorAll('[data-testid="packet"]')).toHaveLength(1);
    expect(crit.getByText("+50%")).toBeTruthy();
    expect(crit.getByText("Crit: needs 95.00+, rolled 97.88, CRIT!").getAttribute("title")).toBe(
      "on-chain raw roll 2.11%, shown as 99.99 - raw, passes when raw < 5.00%",
    );

    cleanup();
    const miss = render(<HitBreakdown hit={MOCK_HITS.miss} defaultOpen={false} />);
    expect(miss.container.querySelectorAll('[data-testid="packet"]')).toHaveLength(0);
    expect((miss.getByText("Hit breakdown").closest("details") as HTMLDetailsElement).open).toBe(false);
  });
});

describe("Demo", () => {
  it("switches the three mock hits, replays, and can show them instantly", () => {
    render(<Demo />);
    expect(screen.getByText("Aria → Wolf")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Resonance hit" })).toBeTruthy();

    fireEvent.click(screen.getByRole("checkbox", { name: "instant" }));
    expect(screen.getByText("Damage d100")).toBeTruthy();
    expect(screen.getByText("9")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Miss" }));
    expect(document.querySelector('[data-kind="damage"] .dice-label')?.textContent).toBe("Miss");
    expect(screen.queryByText("Damage d100")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Crit" }));
    expect(screen.getByText("16")).toBeTruthy();
    expect(document.querySelector('[data-kind="crit"] .dice-label')?.textContent).toBe("Crit");

    fireEvent.click(screen.getByRole("button", { name: "Replay" }));
    expect(screen.getByText("16")).toBeTruthy();
    expect(screen.getAllByTestId("packet")).toHaveLength(1);
  });
});

describe("HitBreakdown modifiers", () => {
  const m = (kind: ModKind, powerBp: number, aspect: Aspect = Aspect.Fire) => ({ kind, aspect, turns: 2, powerBp, source: 5 });

  it("a Vulnerability to an aspect the hit does not carry does nothing, and says so", () => {
    // The crit mock is a single Piercing packet.
    const lines = modLines(MOCK_HITS.crit, { attacker: [], target: [m(ModKind.Vulnerability, 2_500)] });
    expect(lines).toEqual([{ side: "Target", text: "Vulnerability +25% Fire damage taken: no Fire packet in this hit, does nothing", applied: false }]);
    render(<HitBreakdown hit={MOCK_HITS.crit} mods={{ attacker: [], target: [m(ModKind.Vulnerability, 2_500)] }} />);
    expect(screen.getByTestId("hit-mods").textContent).toContain("no Fire packet in this hit");
    expect(screen.getByTestId("packet").textContent).not.toContain("incl.");
  });

  it("annotates the rows each mod fed", () => {
    const mods = { attacker: [m(ModKind.Empowerment, 3_000), m(ModKind.Weakening, 1_000), m(ModKind.Haste, 2_500)], target: [m(ModKind.Vulnerability, 2_500), m(ModKind.Protection, 2_000), m(ModKind.Weakening, 1_500)] };
    const lines = modLines(MOCK_HITS.resonance, mods);
    expect(lines.map((l) => [l.side, l.applied])).toEqual([
      ["Attacker", true],
      ["Attacker", true],
      ["Attacker", false],
      ["Target", true],
      ["Target", true],
      ["Target", false],
    ]);
    expect(lines[3]!.text).toBe("Vulnerability +25% Fire damage taken: in Vulnerability of the Fire packet");
    render(<HitBreakdown hit={MOCK_HITS.resonance} mods={mods} />);
    const [cold, fire] = screen.getAllByTestId("packet");
    expect(cold!.textContent).toContain("incl. Empowerment 30%");
    expect(cold!.textContent).toContain("incl. Weakening 10%");
    expect(cold!.textContent).toContain("incl. Protection 20%");
    expect(cold!.textContent).not.toContain("incl. Vulnerability");
    expect(fire!.textContent).toContain("incl. Vulnerability 25%");
  });

  it("without mods there is no Modifiers section", () => {
    render(<HitBreakdown hit={MOCK_HITS.crit} />);
    expect(screen.queryByTestId("hit-mods")).toBeNull();
  });
});

describe("HitBreakdown items", () => {
  const none = { weapon: null, armor: 0, defense: 0, attack: 0 };

  it("says what the worn items put into the hit", () => {
    // The crit mock is a single Piercing packet with armor in it.
    const items = { attacker: { weapon: "sword", armor: 0, defense: 0, attack: 2 }, target: { weapon: "staff", armor: 10, defense: 1, attack: 0 } };
    expect(itemLines(MOCK_HITS.crit, items)).toEqual([
      { side: "Attacker", text: "Sword: the damage base (Blunt 4-8)", applied: true },
      { side: "Attacker", text: `+2 attack from items, in the total ${MOCK_HITS.crit.attackTotal}`, applied: true },
      { side: "Target", text: "+10 armor from items, in Armor of the physical packets", applied: MOCK_HITS.crit.packets.some((p) => p.armor > 0) },
      { side: "Target", text: `+1 defense from items, in the defense ${MOCK_HITS.crit.defense}`, applied: true },
    ]);
    render(<HitBreakdown hit={MOCK_HITS.crit} items={items} />);
    expect(screen.getByTestId("hit-items").textContent).toContain("Sword: the damage base");
  });

  it("bare hands, and nothing for a monster side", () => {
    expect(itemLines(MOCK_HITS.crit, { attacker: none, target: null })).toEqual([{ side: "Attacker", text: "bare hands: the damage base (Blunt 1-2)", applied: true }]);
    expect(itemLines(MOCK_HITS.crit, { attacker: null, target: none })).toEqual([{ side: "Target", text: "no armor from items", applied: false }]);
    render(<HitBreakdown hit={MOCK_HITS.crit} items={{ attacker: null, target: null }} />);
    expect(screen.getByTestId("hit-items").textContent).toContain("None");
    cleanup();
    render(<HitBreakdown hit={MOCK_HITS.crit} />);
    expect(screen.queryByTestId("hit-items")).toBeNull();
  });
});
