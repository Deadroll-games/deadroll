// One quest from the board (docs/PARTY_AND_TURNS.md 2, steps 3 and 2.1): while the party gathers,
// its lobby (take the quest with a contribution, Ready, the DM's Start); once it started, the way
// into the dungeon. The DM takes the quest down here.
import type { Address } from "@solana/kit";
import { closeQuestIx, questState, type QuestView } from "../../chain/city";
import { status } from "../../chain/game";
import { BIOME_NAMES } from "../../chain/names";
import { useTx } from "../../chain/useTx";
import { PartyProvider, PartyView, useLiveParty } from "../../routes/PartyChain";
import { formatSol, shortAddress } from "../../wallet/ConnectButton";
import { useAppWallet } from "../../wallet/AppWallet";
import { useActiveCharacter } from "../ActiveCharacter";

export function QuestWindow({ quest, onEnter, onClosed }: { quest: QuestView; onEnter: (session: Address) => void; onClosed: () => void }) {
  const session = quest.quest.core.session;
  return (
    <PartyProvider address={session}>
      <QuestBody quest={quest} onEnter={onEnter} onClosed={onClosed} />
    </PartyProvider>
  );
}

function QuestBody({ quest, onEnter, onClosed }: { quest: QuestView; onEnter: (session: Address) => void; onClosed: () => void }) {
  const q = quest.quest.core;
  const live = useLiveParty(q.session);
  const wallet = useAppWallet();
  const { characters } = useActiveCharacter();
  const { send, busy } = useTx();
  const s = live?.party?.session ?? quest.session;
  const st = s ? status(s) : questState(quest) === "gone" ? "gone" : null;
  const isDm = wallet.address === q.dm;
  const mine = new Set(characters.map((c) => c.address as string));
  const joined = !!s && s.core.players.slice(0, s.core.playerCount).some((c) => mine.has(c));
  // close_quest: the party left the lobby, or nobody joined, or it is gone.
  const closable = isDm && (!s || st !== "lobby" || s.core.playerCount === 0);

  const close = async () => {
    const signer = wallet.signer;
    if (!signer) return void (await send("close_quest", null, []));
    const res = await send("Take the quest down", signer, async () => [await closeQuestIx(signer, q.board, q.session)]);
    if (res) onClosed();
  };

  return (
    <div className="quest-window">
      <p className="mono">
        Tier {q.tier} · {BIOME_NAMES[q.biome] ?? BIOME_NAMES[0]} · price {formatSol(q.priceLamports)} · at least {q.minPlayers}{" "}
        {q.minPlayers === 1 ? "player" : "players"} · DM {shortAddress(q.dm)}
        {isDm ? " (you)" : ""}
      </p>
      {st === "lobby" && <PartyView address={q.session} />}
      {st !== null && st !== "lobby" && (
        <div className="panel">
          <p>{st === "active" ? "The party is in the dungeon." : st === "gone" ? "This party no longer exists." : `The party is over (${st}).`}</p>
          {(isDm || joined) && st !== "gone" && (
            <button type="button" className="btn primary" onClick={() => onEnter(q.session)}>
              {st === "active" ? "Enter the dungeon" : "Open the party"}
            </button>
          )}
        </div>
      )}
      {st === null && <p className="muted">Reading the party…</p>}
      {closable && (
        <p>
          <button type="button" className="btn" disabled={busy !== null} onClick={() => void close()}>
            Take the quest down
          </button>{" "}
          <span className="muted">Its rent comes back to you.</span>
        </p>
      )}
    </div>
  );
}
