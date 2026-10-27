// The active character: the one the side panel's bag and skills show and the one that enters a
// quest. Chosen in the characters window and remembered per wallet in this browser.
import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import type { Address } from "@solana/kit";
import { myCharacters, optional, type CharacterAcc } from "../chain/game";
import { TX_SENT_EVENT } from "../chain/txEvents";
import { DATA_SOURCE } from "../config";
import { useAppWallet } from "../wallet/AppWallet";

export type OwnedCharacter = { address: Address; data: CharacterAcc };

type ActiveCharacterState = {
  /** The wallet's characters, in creation order. */
  characters: OwnedCharacter[];
  active: OwnedCharacter | null;
  setActive: (address: Address) => void;
  /** Re-reads the characters (after creating one, a party, a death). */
  refresh: () => Promise<void>;
  loading: boolean;
};

const Ctx = createContext<ActiveCharacterState | null>(null);

const storageKey = (wallet: string) => `deadroll:active-character:${wallet}`;

function load(wallet: string): string | null {
  try {
    return localStorage.getItem(storageKey(wallet));
  } catch {
    return null;
  }
}

function save(wallet: string, address: string) {
  try {
    localStorage.setItem(storageKey(wallet), address);
  } catch {
    // the choice lasts this visit only
  }
}

/** The remembered character if it is still the wallet's, else the first living one, else any. */
export function pickActive(characters: OwnedCharacter[], remembered: string | null): OwnedCharacter | null {
  return characters.find((c) => c.address === remembered) ?? characters.find((c) => c.data.core.alive) ?? characters[0] ?? null;
}

export function ActiveCharacterProvider({ children }: { children: ReactNode }) {
  const wallet = useAppWallet();
  const [characters, setCharacters] = useState<OwnedCharacter[]>([]);
  const [chosen, setChosen] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const me = wallet.address;

  const refresh = useCallback(async () => {
    if (DATA_SOURCE !== "chain" || !me) return setCharacters([]);
    setLoading(true);
    try {
      setCharacters(await myCharacters(me));
    } catch {
      // the list stays as it was
    } finally {
      setLoading(false);
    }
  }, [me]);

  useEffect(() => {
    setChosen(me ? load(me) : null);
    void refresh();
  }, [me, refresh]);

  // The main wallet's transactions create, lock and free characters: read the list again after
  // each (not after every game move a session key pays for).
  useEffect(() => {
    const onSent = (e: Event) => {
      if ((e as CustomEvent<string>).detail === me) void refresh();
    };
    window.addEventListener(TX_SENT_EVENT, onSent);
    return () => window.removeEventListener(TX_SENT_EVENT, onSent);
  }, [me, refresh]);

  const setActive = useCallback(
    (address: Address) => {
      setChosen(address);
      if (me) save(me, address);
    },
    [me],
  );

  const value = useMemo(
    () => ({ characters, active: pickActive(characters, chosen), setActive, refresh, loading }),
    [characters, chosen, setActive, refresh, loading],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useActiveCharacter(): ActiveCharacterState {
  const v = useContext(Ctx);
  if (!v) throw new Error("useActiveCharacter outside ActiveCharacterProvider");
  return v;
}

/** The party the character is in (locked to), if any. */
export const partyOf = (c: OwnedCharacter | null): Address | null => (c ? (optional(c.data.core.inSession) ?? null) : null);
