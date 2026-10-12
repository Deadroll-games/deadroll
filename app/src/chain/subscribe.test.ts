import { describe, expect, it, vi } from "vitest";
import { backoffMs, subscribeResilient, type SubStatus } from "./subscribe";

vi.mock("../wallet/client", () => ({ client: {} }));

type Script = { items?: number[]; fail?: string; openFail?: string };

/** A fake `open`: each call plays the next script entry, then aborts when the scripts run out. */
function harness(scripts: Script[]) {
  const abort = new AbortController();
  const delays: number[] = [];
  const statuses: SubStatus[] = [];
  const got: number[] = [];
  let reconnects = 0;
  let opens = 0;
  let clock = 0;
  const open = async () => {
    const s = scripts[opens++];
    if (!s) {
      abort.abort();
      return (async function* () {})();
    }
    if (s.openFail) throw new Error(s.openFail);
    return (async function* () {
      for (const i of s.items ?? []) yield i;
      if (s.fail) throw new Error(s.fail);
    })();
  };
  const run = subscribeResilient(open, (i: number) => got.push(i), {
    signal: abort.signal,
    onStatus: (s) => statuses.push(s),
    onReconnect: () => reconnects++,
    random: () => 0.5,
    now: () => clock,
    sleep: async (ms) => {
      delays.push(ms);
      clock += ms;
    },
  });
  return { run, delays, statuses, got, reconnects: () => reconnects };
}

describe("backoffMs", () => {
  it("doubles from 1s, caps at 30s, jitters +-20%", () => {
    expect([0, 1, 2, 3, 4, 5, 6].map((a) => backoffMs(a, 1_000, 30_000, () => 0.5))).toEqual([
      1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000,
    ]);
    expect(backoffMs(0, 1_000, 30_000, () => 0)).toBe(800);
    expect(backoffMs(0, 1_000, 30_000, () => 1)).toBe(1_200);
  });
});

describe("subscribeResilient", () => {
  it("backs off on repeated failures and reconnects after them", async () => {
    const h = harness([{ openFail: "refused" }, { openFail: "refused" }, { fail: "socket closed" }, { items: [7] }]);
    await h.run;
    expect(h.delays).toEqual([1_000, 2_000, 4_000, 1_000]);
    expect(h.got).toEqual([7]);
    // live twice (3rd and 4th open), both after a failure
    expect(h.reconnects()).toBe(2);
    expect(h.statuses[0]).toEqual({ state: "connecting" });
    expect(h.statuses).toContainEqual({ state: "reconnecting", attempt: 1, inMs: 1_000, reason: "refused" });
    expect(h.statuses.at(-1)).toEqual({ state: "stopped" });
  });

  it("a stream that delivered items resets the backoff", async () => {
    const h = harness([{ openFail: "x" }, { openFail: "x" }, { items: [1, 2] }, { items: [3] }]);
    await h.run;
    // after two failures (1s, 2s) the 3rd stream delivered, so its end restarts at 1s
    expect(h.delays).toEqual([1_000, 2_000, 1_000, 1_000]);
    expect(h.got).toEqual([1, 2, 3]);
  });

  it("no reconnect callback on a clean first connect, and an ended stream is retried", async () => {
    const h = harness([{ items: [1] }]);
    await h.run;
    expect(h.reconnects()).toBe(0);
    expect(h.statuses).toContainEqual({ state: "reconnecting", attempt: 1, inMs: 1_000, reason: "stream closed" });
  });

  it("a throwing handler does not stop the stream", async () => {
    const abort = new AbortController();
    const seen: number[] = [];
    const err = vi.spyOn(console, "error").mockImplementation(() => undefined);
    await subscribeResilient(
      async () =>
        (async function* () {
          yield 1;
          yield 2;
          abort.abort();
        })(),
      (i: number) => {
        seen.push(i);
        if (i === 1) throw new Error("bad handler");
      },
      { signal: abort.signal, sleep: async () => undefined },
    );
    expect(seen).toEqual([1, 2]);
    err.mockRestore();
  });

  it("stops at once when aborted during the backoff sleep", async () => {
    const abort = new AbortController();
    let opens = 0;
    await subscribeResilient(
      async () => {
        opens++;
        throw new Error("down");
      },
      () => undefined,
      { signal: abort.signal, sleep: async () => abort.abort() },
    );
    expect(opens).toBe(1);
  });
});
