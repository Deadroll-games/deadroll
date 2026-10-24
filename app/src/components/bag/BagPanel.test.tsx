import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { emptyBag } from "../../chain/bagLayout";
import { rewardLabel } from "../../chain/loot";
import { BagPanel } from "./BagPanel";
import type { BagPanelProps } from "./types";

afterEach(() => cleanup());

function props(over: Partial<BagPanelProps> = {}): BagPanelProps {
  const bag = emptyBag();
  bag.resources[0] = { kind: 0, id: 1, qty: 3 };
  return {
    bag,
    cache: [{ kind: 0, id: 2, qty: 4 }],
    editable: true,
    locks: new Set(),
    picked: null,
    error: null,
    label: rewardLabel,
    onPick: vi.fn(),
    onTakeAll: vi.fn(),
    onPlace: vi.fn(),
    onMove: vi.fn(),
    onSplit: vi.fn(),
    onDiscard: vi.fn(),
    onToggleLock: vi.fn(),
    onReset: vi.fn(),
    onDone: vi.fn(),
    ...over,
  };
}

describe("BagPanel (patch 11)", () => {
  it("shows 40 + 5 slots and the cache; Shift + click takes all, a click picks", () => {
    const p = props();
    render(<BagPanel {...p} />);
    expect(screen.getAllByRole("button", { name: /^resources slot/ })).toHaveLength(40);
    expect(screen.getAllByRole("button", { name: /^consumables slot/ })).toHaveLength(5);
    const cached = screen.getByRole("button", { name: "resource #2 x4" });
    fireEvent.click(cached, { shiftKey: true });
    expect(p.onTakeAll).toHaveBeenCalledWith(0);
    fireEvent.click(cached);
    expect(p.onPick).toHaveBeenCalledWith(0);
  });

  it("a picked cache reward replaces the clicked slot", () => {
    const p = props({ picked: 0 });
    render(<BagPanel {...p} />);
    fireEvent.click(screen.getByRole("button", { name: "resources slot 1: resource #1 x3" }));
    expect(p.onPlace).toHaveBeenCalledWith(0, { section: "resources", index: 0 });
  });

  it("throwing a slot away asks first; a locked slot cannot be thrown away", () => {
    const p = props();
    const { rerender } = render(<BagPanel {...p} />);
    fireEvent.click(screen.getByRole("button", { name: "resources slot 1: resource #1 x3" }));
    fireEvent.click(screen.getByRole("button", { name: "Throw away" }));
    expect(p.onDiscard).not.toHaveBeenCalled();
    fireEvent.click(screen.getAllByRole("button", { name: "Throw away" }).at(-1)!);
    expect(p.onDiscard).toHaveBeenCalledWith({ section: "resources", index: 0 });
    rerender(<BagPanel {...p} locks={new Set(["r:0"])} />);
    fireEvent.click(screen.getByRole("button", { name: "resources slot 1: resource #1 x3 (locked)" }));
    expect(screen.getByRole("button", { name: "Throw away" })).toHaveProperty("disabled", true);
  });

  it("Done is off while the layout would be refused; a bag without a cache is read only", () => {
    const { rerender } = render(<BagPanel {...props({ error: "refused" })} />);
    expect(screen.getByRole("button", { name: "Done" })).toHaveProperty("disabled", true);
    rerender(<BagPanel {...props({ editable: false, cache: [] })} />);
    expect(screen.queryByRole("button", { name: "Done" })).toBeNull();
  });
});
