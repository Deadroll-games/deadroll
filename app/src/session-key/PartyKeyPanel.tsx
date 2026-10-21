// The party's session key at a glance (docs/GAME_COMMISSIONS.md 6.2, 8): balance, time left,
// the low-balance warning and the main-wallet buttons. Pure UI over its props.
import { formatSol, shortAddress } from "../wallet/ConnectButton";

export type PartyKeyPanelProps = {
  /** This browser's key for the party, null when there is none. */
  keyAddress: string | null;
  /** The party has this key for me (my participation, or the DM's key). */
  bound: boolean;
  /** Its MagicBlock token is on chain and not expired. */
  live: boolean;
  balance: bigint | null;
  low: boolean;
  lowLamports: bigint;
  /** Seconds until the token expires, null when unknown. */
  secondsLeft: number | null;
  /** The party is over: only leaving is left. */
  ended: boolean;
  busy: boolean;
  /** Undefined hides the button. */
  onTopUp?: () => void;
  topUpLamports: bigint;
  onReplace?: () => void;
  onLeave?: () => void;
};

function timeLeft(secs: number): string {
  if (secs <= 0) return "expired";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h > 0 ? `${h} h ${m} min left` : `${m} min left`;
}

export function PartyKeyPanel(p: PartyKeyPanelProps) {
  const playing = p.keyAddress !== null && p.bound && p.live;
  return (
    <div className="panel" aria-label="Session key">
      <h3>Session key</h3>
      {p.keyAddress === null ? (
        <p className="muted">No session key for this party in this browser: the wallet signs every move.</p>
      ) : (
        <p className="mono">
          {shortAddress(p.keyAddress)} · {p.balance === null ? "..." : formatSol(p.balance)}
          {p.secondsLeft !== null && ` · ${timeLeft(p.secondsLeft)}`}
          {playing ? " · signs moves, no popups" : p.bound ? " · token expired" : " · not bound to this party"}
        </p>
      )}
      {p.keyAddress !== null && p.low && !p.ended && (
        <p className="error">Low balance: below {formatSol(p.lowLamports)}. Top up, or the next moves need the wallet.</p>
      )}
      <div className="row">
        {p.onTopUp && (
          <button className="btn" disabled={p.busy} onClick={p.onTopUp}>
            Top up {formatSol(p.topUpLamports)}
          </button>
        )}
        {p.onReplace && (
          <button className="btn" disabled={p.busy} onClick={p.onReplace}>
            {p.keyAddress === null ? "Start a session key" : "Replace the key"}
          </button>
        )}
        {p.onLeave && (
          <button className="btn primary" disabled={p.busy} onClick={p.onLeave}>
            Leave: return {p.balance === null ? "the rest" : formatSol(p.balance)} to the wallet
          </button>
        )}
      </div>
      {p.onReplace && !p.ended && (
        <p className="muted">Lost the key (another browser, cleared storage)? A new one replaces it with one wallet signature.</p>
      )}
    </div>
  );
}
