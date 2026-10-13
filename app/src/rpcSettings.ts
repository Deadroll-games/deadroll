// The optional "Custom RPC" override: kept in localStorage (never in the bundle), read once at startup.
export const RPC_STORAGE_KEY = "deadroll.rpc";
export const PUBLIC_DEVNET_RPC = "https://api.devnet.solana.com";
export const PUBLIC_DEVNET_WS = "wss://api.devnet.solana.com";

export type CustomRpc = { url: string; ws?: string };

const okUrl = (s: string, protocols: string[]): boolean => {
  try {
    return protocols.includes(new URL(s).protocol);
  } catch {
    return false;
  }
};

/** Validates the form values; returns the setting or an error text. */
export function parseCustomRpc(url: string, ws: string): CustomRpc | string {
  const u = url.trim();
  const w = ws.trim();
  if (!okUrl(u, ["http:", "https:"])) return "RPC URL must start with http:// or https://";
  if (w && !okUrl(w, ["ws:", "wss:"])) return "WebSocket URL must start with ws:// or wss://";
  return w ? { url: u, ws: w } : { url: u };
}

function safeStorage(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

export function loadCustomRpc(storage: Pick<Storage, "getItem"> | null = safeStorage()): CustomRpc | null {
  try {
    const raw = storage?.getItem(RPC_STORAGE_KEY);
    if (!raw) return null;
    const v = JSON.parse(raw) as Partial<CustomRpc>;
    const parsed = parseCustomRpc(String(v.url ?? ""), String(v.ws ?? ""));
    return typeof parsed === "string" ? null : parsed;
  } catch {
    return null;
  }
}

export function saveCustomRpc(value: CustomRpc | null, storage: Pick<Storage, "setItem" | "removeItem"> | null = safeStorage()): void {
  try {
    if (value) storage?.setItem(RPC_STORAGE_KEY, JSON.stringify(value));
    else storage?.removeItem(RPC_STORAGE_KEY);
  } catch {
    // storage blocked: the override just does not persist
  }
}

/** Host only, so a key in the path or query is never shown on screen. */
export function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

/** Provider hosts and key parameters that must never appear in an emitted bundle. */
const PROVIDER_RPC = /helius-rpc\.com|helius\.xyz|alchemy\.com|alchemyapi\.io|quiknode\.pro|quicknode\.com|rpcpool\.com|triton\.one|[?&]api[-_]key=/i;

/** The first provider RPC reference in `code`, or null. vite.config.ts runs it over every built chunk. */
export function findBundledRpcKey(code: string): string | null {
  return PROVIDER_RPC.exec(code)?.[0] ?? null;
}

/** Hosted (production) builds may only be configured with these endpoints; a provider URL carries a key. */
export function isBundleSafeRpc(url: string): boolean {
  try {
    const h = new URL(url).hostname;
    return h === "api.devnet.solana.com" || h === "127.0.0.1" || h === "localhost";
  } catch {
    return false;
  }
}
