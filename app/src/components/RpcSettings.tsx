import { useState } from "react";
import { CUSTOM_RPC, RPC_URL } from "../config";
import { hostOf, parseCustomRpc, saveCustomRpc } from "../rpcSettings";

/** Header control: point this browser at another RPC node. Saved in localStorage, applied on reload. */
export function RpcSettings() {
  const [open, setOpen] = useState(false);
  const [url, setUrl] = useState(CUSTOM_RPC?.url ?? "");
  const [ws, setWs] = useState(CUSTOM_RPC?.ws ?? "");
  const [error, setError] = useState<string | null>(null);

  const save = () => {
    const parsed = parseCustomRpc(url, ws);
    if (typeof parsed === "string") return setError(parsed);
    saveCustomRpc(parsed);
    window.location.reload();
  };
  const reset = () => {
    saveCustomRpc(null);
    window.location.reload();
  };

  return (
    <span className="rpc-settings">
      <button type="button" className="badge" onClick={() => setOpen((o) => !o)} aria-expanded={open} title="RPC endpoint">
        RPC: {hostOf(RPC_URL)}
        {CUSTOM_RPC ? " (custom)" : ""}
      </button>
      {open && (
        <form
          className="rpc-form"
          onSubmit={(e) => {
            e.preventDefault();
            save();
          }}
        >
          <label>
            Custom RPC URL
            <input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://..." autoComplete="off" spellCheck={false} />
          </label>
          <label>
            WebSocket URL (optional)
            <input value={ws} onChange={(e) => setWs(e.target.value)} placeholder="wss://..." autoComplete="off" spellCheck={false} />
          </label>
          <p className="muted">Stored only in this browser. Use the default to go back to the built-in endpoint.</p>
          {error && <p role="alert">{error}</p>}
          <button type="submit">Save and reload</button>
          <button type="button" onClick={reset}>
            Use default
          </button>
        </form>
      )}
    </span>
  );
}
