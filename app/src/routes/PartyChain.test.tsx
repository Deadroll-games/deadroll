import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Address } from "@solana/kit";
import { SessionStatus } from "@deadroll/sdk/session";
import type { Party } from "../chain/game";
import { Claims } from "./PartyChain";

vi.mock("../raster/RasterizerProvider", () => ({ useRenderImage: () => ({ image: null, error: null }) }));

afterEach(cleanup);

const ME = "Me11111111111111111111111111111111111111111" as Address;
const OTHER = "0ther111111111111111111111111111111111111111" as Address;
const DEFAULT = "11111111111111111111111111111111" as Address;
const name = (s: string) => {
  const out = new Uint8Array(24);
  out.set(new TextEncoder().encode(s));
  return out;
};
const player = (slot: number, owner: Address, title: string) => ({
  slot,
  character: `Char${slot}111111111111111111111111111111111111111` as Address,
  char: { name: name(title), core: { xp: 0 } },
  // Patch 11: a cache of 40 xp and one reward (kind 0, id 3, qty 2).
  part: { player: owner, core: { actor: DEFAULT }, cacheXp: 40, cache: Uint8Array.from({ length: 160 }, (_, i) => [0, 3, 0, 2, 0][i] ?? 0) },
});

/** `endedAt`: seconds ago the party ended (the map gives the DM 10 h). */
function party(settledMask = 0, endedAgo = 60, escrowClaimed = false): Party {
  return {
    address: "Sess111111111111111111111111111111111111111" as Address,
    session: { core: { status: SessionStatus.Victory, map: DEFAULT }, settledMask, aliveMask: 0b11, settleTimeoutSecs: 36_000, endedAt: BigInt(Math.floor(Date.now() / 1000) - endedAgo) },
    players: [player(0, ME, "Bran"), player(1, OTHER, "Mira")],
    escrow: { collected: 100_000_000n, claimed: escrowClaimed },
  } as unknown as Party;
}

function show(p: Party, isDm = false) {
  render(
    <Claims
      party={p}
      isDm={isDm}
      me={ME}
      actorFor={() => null}
      payer={null}
      send={vi.fn() as never}
      ingest={() => {}}
      busy={null}
    />,
  );
}

describe("Claims (patch 11)", () => {
  it("shows each cache; before the deadline only the DM hands out the rewards", () => {
    show(party());
    expect(screen.getByText(/Bran/).closest("li")!.textContent).toContain("cache 40 xp");
    expect(screen.getByText(/Bran/).closest("li")!.textContent).toContain("resource #3 x2");
    expect(screen.queryByRole("button", { name: "Claim it myself" })).toBeNull();
    expect(screen.queryByRole("button", { name: /Hand out the rewards/ })).toBeNull();
    cleanup();
    show(party(), true);
    expect(screen.getByRole("button", { name: /^Hand out the rewards and take 0.1/ })).toBeTruthy();
  });

  it("after the deadline each player claims their own; the DM no longer can", () => {
    show(party(0, 11 * 3600), true);
    const buttons = screen.getAllByRole("button", { name: "Claim it myself" });
    expect(buttons).toHaveLength(1);
    expect(buttons[0]!.closest("li")!.textContent).toContain("Bran");
    expect(screen.queryByRole("button", { name: /Hand out the rewards/ })).toBeNull();
  });

  it("shows settled once done", () => {
    show(party(0b11, 60, true));
    expect(screen.getByText(/Bran/).closest("li")!.textContent).toContain("settled");
    expect(screen.getByText(/handed out the rewards/)).toBeTruthy();
  });
});
