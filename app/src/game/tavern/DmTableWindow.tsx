// The DM's table in the Tavern (docs/PARTY_AND_TURNS.md 2, steps 1-2): build and finalize a map,
// then post it as a quest. Posting is one main wallet transaction after the DM confirms the exact
// amount: the DM's session key with its prepayment, `open_session` (the party, its escrow) and
// `post_quest` (the order on this board).
import { useEffect, useState } from "react";
import type { Address } from "@solana/kit";
import { PrepayConfirm } from "../../components/forms/PrepayConfirm";
import { publishRent, postQuestIx } from "../../chain/city";
import { getAccount, mapStatus, openSessionIx, sessionAccounts, type MapAcc } from "../../chain/game";
import { BIOME_NAMES } from "../../chain/names";
import { useTx } from "../../chain/useTx";
import { DATA_SOURCE } from "../../config";
import { MapBuilder } from "../../routes/MapBuilder";
import { FEES } from "../../session-key/sessionKey";
import { commitKey, prepareKey } from "../../session-key/usePartyKey";
import { formatSol, shortAddress } from "../../wallet/ConnectButton";
import { useAppWallet } from "../../wallet/AppWallet";

/** `MIN_PRICE_LAMPORTS` of the session program (docs/PARTY_AND_TURNS.md 3.1): 0.01 SOL. */
const MIN_PRICE_SOL = 0.01;

export function DmTableWindow({ building, onPosted }: { building: Address; onPosted: (session: Address) => void }) {
  const [publishing, setPublishing] = useState<Address | null>(null);
  return publishing ? (
    <PublishQuest building={building} map={publishing} onBack={() => setPublishing(null)} onPosted={onPosted} />
  ) : (
    <MapBuilder onPublish={setPublishing} />
  );
}

function PublishQuest({ building, map, onBack, onPosted }: { building: Address; map: Address; onBack: () => void; onPosted: (session: Address) => void }) {
  const wallet = useAppWallet();
  const { send, busy } = useTx();
  const [price, setPrice] = useState("0.1");
  const [mapAcc, setMapAcc] = useState<MapAcc | null>(null);
  const [rent, setRent] = useState<bigint | null>(null);
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    if (DATA_SOURCE !== "chain") return;
    void getAccount("Map", map).then(setMapAcc, () => setMapAcc(null));
    void publishRent().then(setRent, () => setRent(null));
  }, [map]);

  const lamports = BigInt(Math.round(Number(price) * 1e9));
  const priceOk = Number.isFinite(Number(price)) && Number(price) >= MIN_PRICE_SOL;
  const ready = mapAcc && mapStatus(mapAcc) === "finalized" && mapAcc.core.dm === wallet.address;

  const post = async () => {
    const signer = wallet.signer;
    if (!signer) return void (await send("post_quest", null, []));
    // The DM's key for this party moves the monsters without popups (docs/GAME_COMMISSIONS.md 7).
    const dmKey = await prepareKey(signer);
    const res = await send("Post the quest", signer, async () => [
      dmKey.instruction,
      await openSessionIx(signer, map, lamports, dmKey.signer.address),
      await postQuestIx(signer, building, map),
    ]);
    if (!res) return;
    const { session } = await sessionAccounts(map);
    commitKey(signer.address, session, dmKey);
    onPosted(session);
  };

  return (
    <div className="panel">
      <p>
        <button type="button" className="btn" onClick={onBack}>
          ← Maps
        </button>
      </p>
      <h3>Post a quest</h3>
      <p className="mono">
        Map {shortAddress(map)}
        {mapAcc && ` · ${mapAcc.core.width}x${mapAcc.core.height} · tier ${mapAcc.tier} · ${BIOME_NAMES[mapAcc.biome] ?? BIOME_NAMES[0]} · at least ${mapAcc.minPlayers} players · ${mapStatus(mapAcc)}`}
      </p>
      {mapAcc && !ready && <p className="error">Only a finalized map of yours can be posted.</p>}
      <div className="row">
        <label>
          Party price (SOL){" "}
          <input value={price} onChange={(e) => setPrice(e.target.value)} size={6} inputMode="decimal" />
        </label>
        {!confirming && (
          <button type="button" className="btn primary" disabled={!ready || !priceOk || rent === null || busy !== null} onClick={() => setConfirming(true)}>
            {busy ?? "Post the quest"}
          </button>
        )}
      </div>
      {!priceOk && <p className="error">The price is at least {MIN_PRICE_SOL} SOL.</p>}
      {confirming && rent !== null && (
        <PrepayConfirm
          action="Post the quest"
          lamports={FEES.prepay + rent}
          detail={`(${formatSol(FEES.prepay)} prepayment for your DM session key, plus a small deposit for its token, and ${formatSol(rent)} rent for the party, its escrow and the quest)`}
          busy={busy !== null}
          onConfirm={() => {
            setConfirming(false);
            void post();
          }}
          onCancel={() => setConfirming(false)}
        />
      )}
      <p className="muted">
        Players pay the price into the party escrow when they take the quest; you get it after the final audit. The rent of the quest comes back when you
        take it down.
      </p>
    </div>
  );
}
