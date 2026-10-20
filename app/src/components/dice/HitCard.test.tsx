import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MOCK_HITS } from "../../chain/mock";
import { cascadeSteps } from "./cascade";
import { HIT_STEP_MS, HitCard } from "./HitCard";

const hit = MOCK_HITS.resonance;
const steps = cascadeSteps(hit);
const text = () => document.querySelector(".hit-card-step")?.textContent ?? "";

const play = () => {
  for (let i = 0; i < steps.length; i++) act(() => void vi.advanceTimersByTime(HIT_STEP_MS));
};

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("HitCard", () => {
  it("shows one roll at a time, replacing the last one, and ends on the damage", () => {
    render(<HitCard rollKey="t1" header="A → B" hit={hit} />);
    expect(document.querySelectorAll(".hit-card-step")).toHaveLength(1);
    expect(text()).toContain(steps[0]!.label);
    act(() => void vi.advanceTimersByTime(HIT_STEP_MS));
    expect(document.querySelectorAll(".hit-card-step")).toHaveLength(1);
    expect(text()).toContain(steps[1]!.label);
    play();
    expect(text()).toContain(steps[steps.length - 1]!.label);
    expect(document.querySelector(".hit-card-step")?.getAttribute("data-kind")).toBe("damage");
  });

  it("does not restart for a roll that already played (rerender or remount)", () => {
    const { unmount, rerender } = render(<HitCard rollKey="t2" header="A → B" hit={hit} />);
    play();
    rerender(<HitCard rollKey="t2" header="A → B" hit={hit} />);
    expect(document.querySelector(".hit-card-step")?.getAttribute("data-kind")).toBe("damage");
    unmount();
    render(<HitCard rollKey="t2" header="A → B" hit={hit} />);
    expect(document.querySelector(".hit-card-step")?.getAttribute("data-kind")).toBe("damage");
  });

  it("plays again for a new roll key", () => {
    render(<HitCard rollKey="t3" header="A → B" hit={hit} />);
    play();
    cleanup();
    render(<HitCard rollKey="t4" header="A → B" hit={hit} />);
    expect(text()).toContain(steps[0]!.label);
  });

  it("Details lists every roll of the hit", () => {
    render(<HitCard rollKey="t5" header="A → B" hit={hit} />);
    expect(screen.queryByText("Hit breakdown")).toBeNull();
    fireEvent.click(screen.getByText("Details"));
    expect(screen.getByText("Hit breakdown")).toBeTruthy();
    expect(screen.getAllByTestId("packet")).toHaveLength(2);
  });
});
