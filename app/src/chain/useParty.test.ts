import { describe, expect, it } from "vitest";
import { fallbackPollMs, partyReadError, POLL_FALLBACK_MS, POLL_LIVE_MS } from "./useParty";

describe("fallbackPollMs", () => {
  it("polls slowly while the stream is live and every 3 s while it is down", () => {
    expect(fallbackPollMs("live", false)).toBe(POLL_LIVE_MS);
    for (const st of ["connecting", "reconnecting", "stopped"] as const) expect(fallbackPollMs(st, false)).toBe(POLL_FALLBACK_MS);
  });
  it("never polls faster than 2-3 s and pauses in a hidden tab", () => {
    expect(POLL_FALLBACK_MS).toBeGreaterThanOrEqual(2_000);
    expect(fallbackPollMs("live", true)).toBeNull();
  });
});

describe("partyReadError", () => {
  it("explains a party opened before the session upgrade", () => {
    expect(partyReadError(new Error("borsh: read past end (100+16 > 108)"))).toMatch(/before the session program upgrade/);
    expect(partyReadError(new Error("not a GameSession account"))).toBe("not a GameSession account");
  });
});
