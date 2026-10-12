// Retry with backoff when the RPC node answers HTTP 429 (public devnet rate-limits per IP), and a tiny
// store that tells the UI a request is waiting so it can show "network busy, retrying".
import { useSyncExternalStore } from "react";

const RETRY_STATUS = new Set([429]);
export const MAX_RETRIES = 6;

type Fetch = typeof fetch;
export type RetryOptions = {
  isRpc: (url: string) => boolean;
  sleep?: (ms: number) => Promise<void>;
  random?: () => number;
  maxRetries?: number;
};

/** 1s, 2s, 4s ... capped at 15 s, +-20% jitter; a numeric Retry-After header (seconds) wins. */
export function retryDelayMs(attempt: number, retryAfter: string | null, random: () => number = Math.random): number {
  const secs = retryAfter == null ? NaN : Number(retryAfter);
  if (Number.isFinite(secs) && secs >= 0) return Math.min(30_000, Math.round(secs * 1_000));
  return Math.round(Math.min(15_000, 1_000 * 2 ** attempt) * (0.8 + 0.4 * random()));
}

// --- busy store
let waiting = 0;
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((l) => l());
const setWaiting = (delta: number) => {
  waiting += delta;
  emit();
};
export const networkBusy = (): boolean => waiting > 0;
export function useNetworkBusy(): boolean {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    networkBusy,
    () => false,
  );
}

const urlOf = (input: RequestInfo | URL): string => (typeof input === "string" ? input : input instanceof URL ? input.href : input.url);

/** Wraps `fetch`: requests to the RPC endpoint are retried on 429 with backoff; everything else passes through. */
export function retryingFetch(inner: Fetch, opts: RetryOptions): Fetch {
  const sleep = opts.sleep ?? ((ms: number) => new Promise<void>((r) => setTimeout(r, ms)));
  const max = opts.maxRetries ?? MAX_RETRIES;
  return async (input, init) => {
    if (!opts.isRpc(urlOf(input))) return inner(input, init);
    for (let attempt = 0; ; attempt++) {
      const res = await inner(input, init);
      if (!RETRY_STATUS.has(res.status) || attempt >= max) return res;
      setWaiting(1);
      try {
        await sleep(retryDelayMs(attempt, res.headers.get("retry-after"), opts.random));
      } finally {
        setWaiting(-1);
      }
    }
  };
}

let installed = false;
/** Installs the retrying fetch once; kit's HTTP transport calls the global `fetch` at request time. */
export function installRpcRetry(rpcUrl: string): void {
  if (installed || typeof globalThis.fetch !== "function") return;
  installed = true;
  globalThis.fetch = retryingFetch(globalThis.fetch.bind(globalThis), { isRpc: (u) => u.startsWith(rpcUrl) });
}
