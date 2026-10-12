import { describe, expect, it } from "vitest";
import { hostOf, isBundleSafeRpc, loadCustomRpc, parseCustomRpc, RPC_STORAGE_KEY, saveCustomRpc } from "./rpcSettings";

const memory = () => {
  const m = new Map<string, string>();
  return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v), removeItem: (k: string) => void m.delete(k) };
};

describe("custom RPC setting", () => {
  it("validates the urls", () => {
    expect(parseCustomRpc(" https://rpc.example/x ", "")).toEqual({ url: "https://rpc.example/x" });
    expect(parseCustomRpc("http://127.0.0.1:8899", "ws://127.0.0.1:8900")).toEqual({ url: "http://127.0.0.1:8899", ws: "ws://127.0.0.1:8900" });
    expect(parseCustomRpc("rpc.example", "")).toMatch(/http/);
    expect(parseCustomRpc("https://rpc.example", "https://x")).toMatch(/ws/);
  });
  it("round-trips through storage and ignores junk", () => {
    const s = memory();
    expect(loadCustomRpc(s)).toBeNull();
    saveCustomRpc({ url: "https://rpc.example/k", ws: "wss://rpc.example/k" }, s);
    expect(loadCustomRpc(s)).toEqual({ url: "https://rpc.example/k", ws: "wss://rpc.example/k" });
    s.setItem(RPC_STORAGE_KEY, "{not json");
    expect(loadCustomRpc(s)).toBeNull();
    saveCustomRpc(null, s);
    expect(loadCustomRpc(s)).toBeNull();
  });
  it("shows only the host", () => {
    expect(hostOf("https://solana-devnet.g.alchemy.com/v2/secret")).toBe("solana-devnet.g.alchemy.com");
  });
  it("only the public devnet endpoints and localhost may be bundled", () => {
    expect(isBundleSafeRpc("https://api.devnet.solana.com")).toBe(true);
    expect(isBundleSafeRpc("wss://api.devnet.solana.com")).toBe(true);
    expect(isBundleSafeRpc("http://127.0.0.1:8899")).toBe(true);
    expect(isBundleSafeRpc("https://solana-devnet.g.alchemy.com/v2/key")).toBe(false);
  });
});
