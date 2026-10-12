import { describe, expect, it } from "vitest";
import { CONFIRM_POLL_MS, logsRetryMs } from "./send";

describe("send timing", () => {
  it("polls signature status no faster than every 1.5 s", () => {
    expect(CONFIRM_POLL_MS).toBeGreaterThanOrEqual(1_500);
  });
  it("backs off the log fetch", () => {
    expect([0, 1, 2].map(logsRetryMs)).toEqual([500, 1_000, 1_500]);
  });
});
