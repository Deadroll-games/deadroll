import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Category } from "../../chain/skillcheck";
import { Aspect, Class, Race, Tile } from "../../chain/types";
import { CharacterForm } from "./CharacterForm";
import { formatSol, formatSolExact, pointsLeft, shortAddress } from "./format";
import { MapPalette } from "./MapPalette";
import { PartyLobby } from "./PartyLobby";
import { PrepayConfirm } from "./PrepayConfirm";
import type { CharacterDraft, LobbyPlayer, MapPaletteProps, PartyLobbyProps } from "./types";

afterEach(() => {
  cleanup();
});

const ALL_ASPECTS = [Aspect.Fire, Aspect.Cold, Aspect.Lightning, Aspect.Blood, Aspect.Blunt, Aspect.Piercing];

const DRAFT: CharacterDraft = {
  name: "Bran",
  race: Race.Human,
  class: Class.Warrior,
  predisposition: Aspect.Piercing,
  stats: [5, 5, 5, 5],
};

describe("format helpers", () => {
  it("counts unspent stat points", () => {
    expect(pointsLeft([5, 5, 5, 5], 5, 6)).toBe(6);
    expect(pointsLeft([7, 6, 5, 5], 5, 6)).toBe(3);
    expect(pointsLeft([8, 6, 6, 6], 5, 6)).toBe(0);
  });

  it("formats lamports as SOL", () => {
    expect(formatSol(1_500_000_000n)).toBe("1.5");
    // A confirmation shows the amount to the lamport.
    expect(formatSolExact(8_765_432n)).toBe("0.008765432");
    expect(formatSolExact(10_000_000n)).toBe("0.01");
    expect(formatSolExact(2_000_000_000n)).toBe("2");
    expect(formatSolExact(0n)).toBe("0");
    expect(formatSol(0n)).toBe("0");
    expect(formatSol(123_456_789n)).toBe("0.1234");
    expect(formatSol(2_000_000_000n)).toBe("2");
  });

  it("shortens an address", () => {
    expect(shortAddress("Ch4rWarr1or1111111111111111111111111111111")).toBe("Ch4r..1111");
  });
});

function CharacterHarness({ onSubmit }: { onSubmit: (draft: CharacterDraft) => void }) {
  const [value, setValue] = useState(DRAFT);
  return (
    <CharacterForm
      value={value}
      onChange={setValue}
      onSubmit={onSubmit}
      statPoints={6}
      statMin={5}
      allowedPredispositions={ALL_ASPECTS}
    />
  );
}

describe("CharacterForm", () => {
  it("keeps submit disabled until the points are spent, then submits the draft", () => {
    const onSubmit = vi.fn();
    render(<CharacterHarness onSubmit={onSubmit} />);
    const submit = screen.getByRole("button", { name: "Create character" });
    expect(submit).toHaveProperty("disabled", true);

    const increase = screen.getByRole("button", { name: "increase Strength" });
    for (let i = 0; i < 6; i++) fireEvent.click(increase);

    expect(submit).toHaveProperty("disabled", false);
    fireEvent.click(submit);
    expect(onSubmit).toHaveBeenCalledWith({ ...DRAFT, stats: [11, 5, 5, 5] });
  });

  it("increments a stat through onChange", () => {
    const onChange = vi.fn();
    render(
      <CharacterForm
        value={DRAFT}
        onChange={onChange}
        onSubmit={() => {}}
        statPoints={6}
        statMin={5}
        allowedPredispositions={ALL_ASPECTS}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "increase Strength" }));
    expect(onChange).toHaveBeenCalledWith({ ...DRAFT, stats: [6, 5, 5, 5] });
  });
});

function paletteProps(overrides: Partial<MapPaletteProps> = {}): MapPaletteProps {
  return {
    tool: { kind: "erase" },
    onToolChange: () => {},
    bestiary: [{ id: 0, name: "Wolf" }],
    presets: [{ name: "demo_16x16", width: 16, height: 16 }],
    onLoadPreset: () => {},
    spawns: [],
    onRemoveSpawn: () => {},
    errors: [],
    onValidate: () => {},
    canFinalize: false,
    onFinalize: () => {},
    ...overrides,
  };
}

describe("MapPalette", () => {
  it("selects a tile tool", () => {
    const onToolChange = vi.fn();
    render(<MapPalette {...paletteProps({ onToolChange })} />);
    fireEvent.click(screen.getByRole("button", { name: "Stone floor" }));
    expect(onToolChange).toHaveBeenCalledWith({ kind: "tile", tile: Tile.StoneFloor });
  });

  it("patch 13: selects the entrance and the exit tools", () => {
    const onToolChange = vi.fn();
    render(<MapPalette {...paletteProps({ onToolChange })} />);
    fireEvent.click(screen.getByRole("button", { name: "Entrance" }));
    expect(onToolChange).toHaveBeenCalledWith({ kind: "entrance" });
    fireEvent.click(screen.getByRole("button", { name: "Exit" }));
    expect(onToolChange).toHaveBeenCalledWith({ kind: "exit" });
  });

  it("patch 11: selects the chest tool and shows the loot panel", () => {
    const onToolChange = vi.fn();
    render(<MapPalette {...paletteProps({ onToolChange, loot: <p>loot panel</p> })} />);
    fireEvent.click(screen.getByRole("button", { name: "Chest" }));
    expect(onToolChange).toHaveBeenCalledWith({ kind: "chest" });
    expect(screen.getByText("loot panel")).toBeTruthy();
  });

  it("disables Finalize until the map is valid", () => {
    const { rerender } = render(<MapPalette {...paletteProps({ canFinalize: false })} />);
    expect(screen.getByRole("button", { name: "Finalize" })).toHaveProperty("disabled", true);
    rerender(<MapPalette {...paletteProps({ canFinalize: true })} />);
    expect(screen.getByRole("button", { name: "Finalize" })).toHaveProperty("disabled", false);
  });
});

function lobbyProps(overrides: Partial<PartyLobbyProps> = {}): PartyLobbyProps {
  return {
    mapName: "demo_16x16",
    dm: "Dm11111111111111111111111111111111111111dm",
    priceLamports: 100_000_000n,
    collected: 0n,
    players: [],
    maxPlayers: 4,
    role: "dm",
    myCharacters: [],
    onJoin: () => {},
    onReady: () => {},
    onStart: () => {},
    ...overrides,
  };
}

const BRAN: LobbyPlayer = {
  slot: 0,
  player: "Ch4rWarr1or1111111111111111111111111111111",
  characterName: "Bran",
  race: Race.Human,
  class: Class.Warrior,
  ready: false,
  paid: 100_000_000n,
};

const ARIA: LobbyPlayer = {
  slot: 1,
  player: "Ch4rE1fMage111111111111111111111111111111",
  characterName: "Aria",
  race: Race.ForestElf,
  class: Class.Mage,
  ready: true,
  paid: 100_000_000n,
};

describe("PartyLobby", () => {
  it("keeps Start disabled until every player is ready", () => {
    const players = [BRAN, ARIA];
    const { rerender } = render(<PartyLobby {...lobbyProps({ players })} />);
    expect(screen.getByRole("button", { name: "Start session" })).toHaveProperty("disabled", true);
    rerender(<PartyLobby {...lobbyProps({ players: [{ ...BRAN, ready: true }, ARIA] })} />);
    expect(screen.getByRole("button", { name: "Start session" })).toHaveProperty("disabled", false);
  });

  it("Ready takes a full map selection: 4 active + 1 reactive + 1 passive, swappable", () => {
    const onReady = vi.fn();
    const mySkills = [
      { index: 0, name: "Thrust", category: Category.Active },
      { index: 1, name: "Bash", category: Category.Active },
      { index: 2, name: "Cleave", category: Category.Active },
      { index: 3, name: "Javelin", category: Category.Active },
      { index: 4, name: "Riposte", category: Category.Reactive },
      { index: 5, name: "Iron Skin", category: Category.Passive },
      { index: 6, name: "Sap", category: Category.Active },
    ];
    render(<PartyLobby {...lobbyProps({ role: "player", mySlot: 0, players: [BRAN], mySkills, onReady })} />);
    // The first four actives start checked; the fifth waits for a free active slot.
    const box = (name: string) => screen.getByRole("checkbox", { name }) as HTMLInputElement;
    expect(box("Sap").checked).toBe(false);
    expect(box("Sap").disabled).toBe(true);
    fireEvent.click(box("Bash"));
    expect(screen.getByRole("button", { name: "Ready" })).toHaveProperty("disabled", true);
    expect(screen.getByText(/Take 4 active/)).toBeTruthy();
    fireEvent.click(box("Sap"));
    fireEvent.click(screen.getByRole("button", { name: "Ready" }));
    expect(onReady).toHaveBeenCalledWith(true, [0, 2, 3, 6, 4, 5]);
  });

  it("Ready with a new session key asks to confirm the prepayment; nothing happens without Confirm", () => {
    const onReady = vi.fn();
    const mySkills = [
      { index: 0, name: "Thrust", category: Category.Active },
      { index: 1, name: "Bash", category: Category.Active },
      { index: 2, name: "Cleave", category: Category.Active },
      { index: 3, name: "Javelin", category: Category.Active },
      { index: 4, name: "Riposte", category: Category.Reactive },
      { index: 5, name: "Iron Skin", category: Category.Passive },
    ];
    render(<PartyLobby {...lobbyProps({ role: "player", mySlot: 0, players: [BRAN], mySkills, onReady, readyPrepayLamports: 10_000_000n })} />);
    fireEvent.click(screen.getByRole("button", { name: "Ready" }));
    expect(onReady).not.toHaveBeenCalled();
    expect(screen.getByRole("alertdialog").textContent).toContain("Ready takes 0.01 SOL from your wallet");
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onReady).not.toHaveBeenCalled();
    expect(screen.queryByRole("alertdialog")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Ready" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm Ready" }));
    expect(onReady).toHaveBeenCalledWith(true, [0, 1, 2, 3, 4, 5]);
  });

  it("Ready that reuses the party's key and Not ready ask nothing", () => {
    const onReady = vi.fn();
    render(<PartyLobby {...lobbyProps({ role: "player", mySlot: 0, players: [{ ...BRAN, ready: true }], mySkills: [], onReady, readyPrepayLamports: 10_000_000n })} />);
    fireEvent.click(screen.getByRole("button", { name: "Not ready" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(onReady).toHaveBeenCalledTimes(1);
    cleanup();
    const mySkills = [
      ...[0, 1, 2, 3].map((index) => ({ index, name: `A${index}`, category: Category.Active })),
      { index: 4, name: "R", category: Category.Reactive },
      { index: 5, name: "P", category: Category.Passive },
    ];
    render(<PartyLobby {...lobbyProps({ role: "player", mySlot: 0, players: [BRAN], mySkills, onReady, readyPrepayLamports: 0n })} />);
    fireEvent.click(screen.getByRole("button", { name: "Ready" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(onReady).toHaveBeenLastCalledWith(true, [0, 1, 2, 3, 4, 5]);
  });

  it("Not ready needs no selection", () => {
    const onReady = vi.fn();
    render(<PartyLobby {...lobbyProps({ role: "player", mySlot: 0, players: [{ ...BRAN, ready: true }], mySkills: [], onReady })} />);
    expect(screen.queryByRole("checkbox")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Not ready" }));
    expect(onReady).toHaveBeenCalledWith(false, [0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
  });

  it("joins with the selected character", () => {
    const onJoin = vi.fn();
    render(
      <PartyLobby
        {...lobbyProps({
          role: "guest",
          onJoin,
          myCharacters: [
            { address: BRAN.player, name: "Bran" },
            { address: ARIA.player, name: "Aria" },
          ],
        })}
      />,
    );
    fireEvent.change(screen.getByLabelText("Character"), { target: { value: ARIA.player } });
    fireEvent.click(screen.getByRole("button", { name: "Join and pay 0.1 SOL" }));
    // Patch 11: the share and the rent of the party accounts are confirmed first.
    expect(onJoin).not.toHaveBeenCalled();
    expect(screen.getByRole("alertdialog").textContent).toContain("rent of your party accounts");
    fireEvent.click(screen.getByRole("button", { name: "Confirm Join" }));
    expect(onJoin).toHaveBeenCalledWith(ARIA.player);
  });
});

describe("PrepayConfirm", () => {
  it("states the prepayment of a session key by default", () => {
    render(<PrepayConfirm action="Ready" lamports={5_000_000n} onConfirm={() => {}} onCancel={() => {}} />);
    expect(screen.getByRole("alertdialog").textContent).toContain("prepayment of this party's session key");
  });

  it("patch 11: states what else the SOL pays for, then confirms", () => {
    const onConfirm = vi.fn();
    render(<PrepayConfirm action="Finalize" lamports={12_000_000n} detail="as rent for the map's two accounts" onConfirm={onConfirm} onCancel={() => {}} />);
    const text = screen.getByRole("alertdialog").textContent ?? "";
    expect(text).toContain("as rent for the map's two accounts");
    expect(text).not.toContain("session key");
    fireEvent.click(screen.getByRole("button", { name: "Confirm Finalize" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });
});
