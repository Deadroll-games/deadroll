// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PartyKeyPanel, type PartyKeyPanelProps } from "./PartyKeyPanel";

afterEach(() => {
  cleanup();
});

const props = (over: Partial<PartyKeyPanelProps> = {}): PartyKeyPanelProps => ({
  keyAddress: "Key1111111111111111111111111111111111111111",
  bound: true,
  live: true,
  balance: 41_234_568n,
  low: false,
  lowLamports: 2_000_000n,
  secondsLeft: 3600,
  ended: false,
  busy: false,
  topUpLamports: 8_765_432n,
  replaceLamports: 10_000_000n,
  ...over,
});

describe("PartyKeyPanel: SOL moves to the key only after the exact amount is confirmed", () => {
  it("Top up shows the exact amount; Cancel signs nothing, Confirm tops up", () => {
    const onTopUp = vi.fn();
    render(<PartyKeyPanel {...props({ onTopUp })} />);
    fireEvent.click(screen.getByRole("button", { name: /Top up/ }));
    expect(onTopUp).not.toHaveBeenCalled();
    const text = screen.getByRole("alertdialog").textContent ?? "";
    expect(text).toContain("Top up takes 0.008765432 SOL from your wallet");
    expect(text).not.toContain("deposit for its token");
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(onTopUp).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /Top up/ }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm Top up" }));
    expect(onTopUp).toHaveBeenCalledTimes(1);
  });

  it("Replace the key shows the new key's prepayment and its token deposit; Confirm replaces", () => {
    const onReplace = vi.fn();
    render(<PartyKeyPanel {...props({ live: false, onReplace })} />);
    fireEvent.click(screen.getByRole("button", { name: "Replace the key" }));
    expect(onReplace).not.toHaveBeenCalled();
    const text = screen.getByRole("alertdialog").textContent ?? "";
    expect(text).toContain("Replace the key takes 0.01 SOL from your wallet");
    expect(text).toContain("deposit for its token");
    fireEvent.click(screen.getByRole("button", { name: "Confirm Replace the key" }));
    expect(onReplace).toHaveBeenCalledTimes(1);
  });

  it("Leave moves SOL back to the wallet and asks nothing", () => {
    const onLeave = vi.fn();
    render(<PartyKeyPanel {...props({ ended: true, onLeave })} />);
    fireEvent.click(screen.getByRole("button", { name: /Leave/ }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(onLeave).toHaveBeenCalledTimes(1);
  });
});
