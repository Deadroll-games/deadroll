// The Tavern's guild board (docs/PARTY_AND_TURNS.md 2, step 3): the quests posted on it and the
// live state of each party. Looking is free: account reads, no signature.
import { useCallback, useEffect, useState } from "react";
import type { Address } from "@solana/kit";
import { questState, questsOn, type QuestView } from "../../chain/city";
import { BIOME_NAMES } from "../../chain/names";
import { PARTY_MAX } from "../../chain/types";
import { DATA_SOURCE } from "../../config";
import { formatSol, shortAddress } from "../../wallet/ConnectButton";
import { useAppWallet } from "../../wallet/AppWallet";
import { useActiveCharacter } from "../ActiveCharacter";

const STATE_TEXT = { lobby: "gathering players", active: "in the dungeon", ended: "over", gone: "closed" } as const;
/** How often the open board re-reads its quests. */
const BOARD_POLL_MS = 10_000;

export function BoardWindow({ board, onOpen }: { board: Address; onOpen: (q: QuestView) => void }) {
  const wallet = useAppWallet();
  const [quests, setQuests] = useState<QuestView[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [mineOnly, setMineOnly] = useState(false);

  const refresh = useCallback(async () => {
    if (DATA_SOURCE !== "chain") return setQuests([]);
    try {
      setQuests(await questsOn(board));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [board]);
  useEffect(() => {
    void refresh();
    const t = setInterval(() => void refresh(), BOARD_POLL_MS);
    return () => clearInterval(t);
  }, [refresh]);

  const me = wallet.address;
  const { characters } = useActiveCharacter();
  const mineSet = new Set(characters.map((c) => c.address as string));
  // `GameSession.players` holds the characters in the party.
  const joined = (q: QuestView) => !!q.session && q.session.core.players.slice(0, q.session.core.playerCount).some((c) => mineSet.has(c));
  const mine = (q: QuestView) => q.quest.core.dm === me || joined(q);
  const shown = (quests ?? []).filter((q) => (mineOnly ? mine(q) : questState(q) !== "gone"));

  return (
    <div className="board">
      <div className="row">
        <button type="button" className="btn" aria-pressed={!mineOnly} onClick={() => setMineOnly(false)}>
          All quests
        </button>
        <button type="button" className="btn" aria-pressed={mineOnly} disabled={!me} onClick={() => setMineOnly(true)}>
          My quests
        </button>
        <button type="button" className="btn" onClick={() => void refresh()}>
          Refresh
        </button>
      </div>
      {error && <p className="error">{error}</p>}
      {DATA_SOURCE !== "chain" && <p className="muted">The board reads the chain: start the site against a validator.</p>}
      {quests === null && DATA_SOURCE === "chain" && <p className="muted">Reading the board…</p>}
      {quests !== null && shown.length === 0 && <p className="muted">{mineOnly ? "None of the quests here are yours." : "No quests yet. A DM posts one from the table by the fire."}</p>}
      <ul className="quest-list">
        {shown.map((q) => {
          const s = q.session;
          const state = questState(q);
          const collected = q.escrow?.collected ?? 0n;
          const players = s?.core.playerCount ?? 0;
          return (
            <li key={q.address}>
              <button type="button" className={`quest-card panel ${state}`} onClick={() => onOpen(q)}>
                <span className="quest-title">
                  Tier {q.quest.core.tier} · {BIOME_NAMES[q.quest.core.biome] ?? BIOME_NAMES[0]}
                  {q.quest.core.dm === me ? " · your quest" : joined(q) ? " · you are in" : ""}
                </span>
                <span>
                  Price {formatSol(q.quest.core.priceLamports)} · paid {formatSol(collected)}
                </span>
                <span>
                  Players {players}/{PARTY_MAX} (at least {q.quest.core.minPlayers}) · {STATE_TEXT[state]}
                </span>
                <span className="muted mono">DM {shortAddress(q.quest.core.dm)}</span>
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

