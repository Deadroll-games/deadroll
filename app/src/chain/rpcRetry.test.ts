import { describe, expect, it } from "vitest";
import { MAX_RETRIES, networkBusy, retryDelayMs, retryingFetch } from "./rpcRetry";

const res = (status: number, headers: Record<string, string> = {}) => new Response("{}", { status, headers });
const isRpc = (u: string) => u.startsWith("https://rpc.test");

describe("retryDelayMs", () => {
  it("doubles from 1 s, caps at 15 s and honors Retry-After", () => {
    const mid = () => 0.5;
    expect([0, 1, 2, 3, 10].map((n) => retryDelayMs(n, null, mid))).toEqual([1_000, 2_000, 4_000, 8_000, 15_000]);
    expect(retryDelayMs(0, "3", mid)).toBe(3_000);
  });
});

describe("retryingFetch", () => {
  it("retries a 429 with backoff, shows busy meanwhile, then returns the good answer", async () => {
    const answers = [res(429), res(429), res(200)];
    let busyDuringWait = false;
    const f = retryingFetch(async () => answers.shift()!, {
      isRpc,
      random: () => 0.5,
      sleep: async (ms) => {
        busyDuringWait = networkBusy();
        expect([1_000, 2_000]).toContain(ms);
      },
    });
    expect((await f("https://rpc.test/x", { method: "POST" })).status).toBe(200);
    expect(busyDuringWait).toBe(true);
    expect(networkBusy()).toBe(false);
  });

  it("gives up after the retry budget and returns the 429", async () => {
    let calls = 0;
    const f = retryingFetch(async () => (calls++, res(429)), { isRpc, sleep: async () => undefined });
    expect((await f("https://rpc.test/x")).status).toBe(429);
    expect(calls).toBe(MAX_RETRIES + 1);
  });

  it("does not touch other hosts or other statuses", async () => {
    let calls = 0;
    const f = retryingFetch(async () => (calls++, res(429)), { isRpc, sleep: async () => undefined });
    expect((await f("https://other.test/x")).status).toBe(429);
    expect(calls).toBe(1);
    const g = retryingFetch(async () => (calls++, res(500)), { isRpc, sleep: async () => undefined });
    expect((await g("https://rpc.test/x")).status).toBe(500);
    expect(calls).toBe(2);
  });
});
