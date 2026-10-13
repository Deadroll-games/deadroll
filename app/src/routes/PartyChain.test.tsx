import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Address } from "@solana/kit";
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
  part: { player: owner, core: { actor: DEFAULT } },
});

function party(settledMask = 0): Party {
  return {
    address: "Sess111111111111111111111111111111111111111" as Address,
    session: { core: { status: { victory: {} }, map: DEFAULT }, xpPool: 120, settledMask, aliveMask: 0b11 },
    players: [player(0, ME, "Bran"), player(1, OTHER, "Mira")],
    escrow: null,
  } as unknown as Party;
}

function show(p: Party) {
  render(
    <Claims
      party={p}
      isDm={false}
      me={ME}
      actorFor={() => null}
      payer={null}
      send={vi.fn() as never}
      ingest={() => {}}
      busy={null}
    />,
  );
}

describe("Claims", () => {
  it("shows the claim button only on the viewer's own character", () => {
    show(party());
    const buttons = screen.getAllByRole("button", { name: "Claim rewards" });
    expect(buttons).toHaveLength(1);
    expect(buttons[0]!.closest("li")!.textContent).toContain("Bran");
    const other = screen.getByText(/Mira/).closest("li")!;
    expect(other.querySelector("button")).toBeNull();
    expect(other.textContent).toContain("not claimed yet");
  });

  it("shows claimed instead of a button once settled", () => {
    show(party(0b01));
    expect(screen.queryByRole("button", { name: "Claim rewards" })).toBeNull();
    expect(screen.getByText(/Bran/).closest("li")!.textContent).toContain("claimed");
  });
});
