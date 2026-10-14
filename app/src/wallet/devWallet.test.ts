import { afterEach, describe, expect, it } from "vitest";
import { devModeRequested } from "./devWallet";

describe("dev wallet gate", () => {
  afterEach(() => sessionStorage.clear());

  it("?dev=1 turns dev mode on where allowed, ?dev=0 off", () => {
    expect(devModeRequested("?dev=1", true)).toBe(true);
    expect(devModeRequested("", true)).toBe(true);
    expect(devModeRequested("?dev=0", true)).toBe(false);
  });

  it("is never on where not allowed (devnet build or a non-local RPC)", () => {
    expect(devModeRequested("?dev=1", false)).toBe(false);
    expect(devModeRequested("", false)).toBe(false);
  });
});
