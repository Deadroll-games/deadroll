import { formatSolExact } from "./format";
import "./forms.css";

export type PrepayConfirmProps = {
  /** The button the player pressed, e.g. "Ready", "Open party", "Top up". */
  action: string;
  /** Lamports the main wallet moves to the party's session key (BalanceConfig.fees). */
  lamports: bigint;
  /** A new key: its token's rent deposit is paid too (false for a top-up of the same key). */
  newKey?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
  busy?: boolean;
};

/**
 * The explicit yes before a transaction moves SOL from the main wallet to a session key
 * (docs/GAME_COMMISSIONS.md 6.2), with the exact amount. Nothing is signed until Confirm.
 */
export function PrepayConfirm(p: PrepayConfirmProps) {
  const newKey = p.newKey ?? true;
  return (
    <div className="forms-confirm" role="alertdialog" aria-label={`Confirm ${p.action}`}>
      <p>
        {p.action} takes <span className="mono">{formatSolExact(p.lamports)} SOL</span> from your wallet for the fee
        prepayment of this party's session key{newKey ? ", plus a small deposit for its token" : ""}. What the key does
        not spend comes back when you leave the party.
      </p>
      <div className="forms-row">
        <button type="button" className="btn primary" disabled={p.busy} onClick={p.onConfirm}>
          Confirm {p.action}
        </button>
        <button type="button" className="btn" disabled={p.busy} onClick={p.onCancel}>
          Cancel
        </button>
      </div>
    </div>
  );
}
