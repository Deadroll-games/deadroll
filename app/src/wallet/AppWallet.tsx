// The app's signer: a Wallet Standard wallet (@solana/react via kit-plugin-wallet), or the tab's
// dev keypair on localnet (`?dev=1`). Every screen reads `useAppWallet()` instead of the wallet
// plugin directly, so both kinds sign the same instructions.
import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import type { Address, KeyPairSigner, TransactionSigner } from "@solana/kit";
import { useConnectedWallet } from "@solana/kit-plugin-wallet/react";
import { DATA_SOURCE } from "../config";
import { client } from "./client";
import { devModeRequested, ensureDevFunds, loadDevSigner } from "./devWallet";

export type AppWallet = {
  kind: "dev" | "wallet" | "none";
  address: Address | null;
  signer: TransactionSigner | null;
  /** Lamports, refreshed after airdrops and on demand (dev mode). */
  balance: bigint | null;
  devMode: boolean;
  airdrop: () => Promise<void>;
  refreshBalance: () => Promise<void>;
  error: string | null;
};

const Ctx = createContext<AppWallet | null>(null);

export function AppWalletProvider({ children }: { children: ReactNode }) {
  const [devMode] = useState(() => DATA_SOURCE === "chain" && devModeRequested());
  const connected = useConnectedWallet(client);
  const [dev, setDev] = useState<KeyPairSigner | null>(null);
  const [balance, setBalance] = useState<bigint | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!devMode) return;
    let live = true;
    void loadDevSigner().then(async (s) => {
      if (!live) return;
      setDev(s);
      try {
        setBalance(await ensureDevFunds(client.rpc as never, s.address));
      } catch (e) {
        setError(`airdrop failed: ${e instanceof Error ? e.message : String(e)}`);
      }
    });
    return () => {
      live = false;
    };
  }, [devMode]);

  const address = devMode ? (dev?.address ?? null) : ((connected?.account.address as Address | undefined) ?? null);
  const signer: TransactionSigner | null = devMode ? dev : (connected?.signer ?? null);

  const refreshBalance = useCallback(async () => {
    if (!address) return;
    try {
      setBalance((await client.rpc.getBalance(address, { commitment: "confirmed" }).send()).value);
    } catch {
      // keep the last value
    }
  }, [address]);

  const airdrop = useCallback(async () => {
    if (!address) return;
    setError(null);
    try {
      setBalance(await ensureDevFunds(client.rpc as never, address, true));
    } catch (e) {
      setError(`airdrop failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  }, [address]);

  useEffect(() => {
    void refreshBalance();
    // Payouts (escrow, rewards) arrive from other windows' transactions: keep the badge current.
    const t = setInterval(() => void refreshBalance(), 5_000);
    return () => clearInterval(t);
  }, [refreshBalance]);

  const value = useMemo<AppWallet>(
    () => ({
      kind: devMode ? "dev" : signer ? "wallet" : "none",
      address,
      signer,
      balance,
      devMode,
      airdrop,
      refreshBalance,
      error,
    }),
    [devMode, signer, address, balance, airdrop, refreshBalance, error],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useAppWallet(): AppWallet {
  const v = useContext(Ctx);
  if (!v) throw new Error("useAppWallet outside AppWalletProvider");
  return v;
}
