import "./forms.css";

export type StalledActionProps = {
  /** The action that stopped, e.g. "Create the character". */
  what: string;
  /** The 1-based step that did not land, of `total`. */
  step: number;
  total: number;
  onRetry: () => void;
  onDismiss: () => void;
  busy?: boolean;
};

/**
 * A multi-step action stopped part way: the steps before `step` landed. Retry signs the steps
 * left in one wallet confirmation and sends them in order.
 */
export function StalledAction(p: StalledActionProps) {
  return (
    <div className="forms-confirm" role="alert" aria-label={`${p.what} stopped`}>
      <p>
        {p.what} stopped at step {p.step} of {p.total}
        {p.step > 1 ? `; steps 1-${p.step - 1} are done` : ""}. Retry signs the rest in one wallet confirmation.
      </p>
      <div className="forms-row">
        <button type="button" className="btn primary" disabled={p.busy} onClick={p.onRetry}>
          Retry from step {p.step}
        </button>
        <button type="button" className="btn" disabled={p.busy} onClick={p.onDismiss}>
          Dismiss
        </button>
      </div>
    </div>
  );
}
