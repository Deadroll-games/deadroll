import { formatSol } from "./format";
import "./forms.css";

export type PrepayConfirmProps = {
  /** The button the player pressed, e.g. "Ready" or "Open party". */
  action: string;
  /** Lamports the main wallet moves to the party's session key (BalanceConfig.fees). */
  lamports: bigint;
  onConfirm: () => void;
  onCancel: () => void;
  busy?: boolean;
};

/**
 * The explicit yes before a transaction moves the fee prepayment from the main wallet to a session
 * key (docs/GAME_COMMISSIONS.md 6.2). Nothing is signed until Confirm.
 */
export function PrepayConfirm(p: PrepayConfirmProps) {
  return (
    <div className="forms-confirm" role="alertdialog" aria-label={`Confirm ${p.action}`}>
      <p>
        {p.action} takes <span className="mono">{formatSol(p.lamports)} SOL</span> from your wallet for the fee
        prepayment of this party's session key, plus a small deposit for its token. What the key does not spend
        comes back when you leave the party.
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
