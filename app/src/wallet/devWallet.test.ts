import { afterEach, describe, expect, it } from "vitest";
import { devModeRequested, exportDevSeed, importDevSeed, loadDevSigner } from "./devWallet";

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

describe("patch C: the dev key in another tab", () => {
  it("exports the tab's seed and takes one pasted from another tab", async () => {
    sessionStorage.clear();
    const a = await loadDevSigner();
    const hex = exportDevSeed()!;
    expect(hex).toMatch(/^[0-9a-f]{64}$/);
    sessionStorage.clear();
    expect(importDevSeed("not a key")).toBe(false);
    expect(importDevSeed(hex)).toBe(true);
    expect((await loadDevSigner()).address).toBe(a.address);
  });
});
