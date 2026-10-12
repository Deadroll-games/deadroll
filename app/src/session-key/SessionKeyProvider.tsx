import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import type { Address, KeyPairSigner } from "@solana/kit";
import { useConnectedWallet } from "@solana/kit-plugin-wallet/react";
import { DATA_SOURCE, PROGRAMS } from "../config";
import { client } from "../wallet/client";
import {
  SESSION_VALIDITY_SECS,
  clearSessionKey,
  createSessionV2Instruction,
  loadSessionKey,
  newSeed,
  saveSessionKey,
  signerFromSeed,
} from "./sessionKey";

export type SessionKeyState = {
  /** Burner signer for game moves; null until started. */
  signer: KeyPairSigner | null;
  /** SessionTokenV2 address (null in mock mode). */
  token: Address | null;
  validUntil: number | null;
  status: "idle" | "starting" | "active" | "error";
  error: string | null;
  start: () => Promise<void>;
  stop: () => void;
};

const Ctx = createContext<SessionKeyState | null>(null);

export function SessionKeyProvider({ children }: { children: ReactNode }) {
  const connected = useConnectedWallet(client);
  const [signer, setSigner] = useState<KeyPairSigner | null>(null);
  const [token, setToken] = useState<Address | null>(null);
  const [validUntil, setValidUntil] = useState<number | null>(null);
  const [status, setStatus] = useState<SessionKeyState["status"]>("idle");
  const [error, setError] = useState<string | null>(null);
  const authority = connected?.account.address ?? null;

  // Restore a key started earlier in this tab for the same wallet.
  useEffect(() => {
    const saved = loadSessionKey();
    if (!saved || (DATA_SOURCE === "chain" && saved.authority !== authority)) return;
    void signerFromSeed(saved.seed).then((s) => {
      setSigner(s);
      setToken(saved.token as Address | null);
      setValidUntil(saved.validUntil);
      setStatus("active");
    });
  }, [authority]);

  const start = useCallback(async () => {
    setStatus("starting");
    setError(null);
    try {
      const seed = newSeed();
      const burner = await signerFromSeed(seed);
      const until = Math.floor(Date.now() / 1000) + SESSION_VALIDITY_SECS;
      let tokenAddr: Address | null = null;
      if (DATA_SOURCE === "chain") {
        const walletSigner = connected?.signer;
        if (!walletSigner) throw new Error("Connect a wallet first");
        const { instruction, sessionToken } = await createSessionV2Instruction({
          authority: walletSigner,
          sessionSigner: burner,
          targetProgram: PROGRAMS.session,
          validUntil: BigInt(until),
        });
        await client.sendTransaction([instruction]);
        tokenAddr = sessionToken;
      }
      saveSessionKey({ seed, authority: authority ?? "mock", validUntil: until, token: tokenAddr });
      setSigner(burner);
      setToken(tokenAddr);
      setValidUntil(until);
      setStatus("active");
    } catch (e) {
      setStatus("error");
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [connected, authority]);

  const stop = useCallback(() => {
    // TODO(post-demo): revoke_session_v2 to return the token rent; the token expires on its own.
    clearSessionKey();
    setSigner(null);
    setToken(null);
    setValidUntil(null);
    setStatus("idle");
  }, []);

  const value = useMemo(
    () => ({ signer, token, validUntil, status, error, start, stop }),
    [signer, token, validUntil, status, error, start, stop],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useSessionKey(): SessionKeyState {
  const v = useContext(Ctx);
  if (!v) throw new Error("useSessionKey outside SessionKeyProvider");
  return v;
}

export function SessionKeyButton() {
  const sk = useSessionKey();
  if (sk.status === "active" && sk.signer) {
    const mins = sk.validUntil ? Math.max(0, Math.round((sk.validUntil * 1000 - Date.now()) / 60_000)) : 0;
    return (
      <button className="btn" onClick={sk.stop} title={`Session key ${sk.signer.address}, ${mins} min left`}>
        Session key on
      </button>
    );
  }
  return (
    <span>
      <button className="btn" disabled={sk.status === "starting"} onClick={() => void sk.start()}>
        {sk.status === "starting" ? "Starting..." : "Start session key"}
      </button>
      {sk.error && <span className="error"> {sk.error}</span>}
    </span>
  );
}
