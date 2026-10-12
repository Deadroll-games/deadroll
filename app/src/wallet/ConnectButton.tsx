import { useConnect, useConnectedWallet, useDisconnect, useWallets } from "@solana/kit-plugin-wallet/react";
import { useState } from "react";
import { client } from "./client";

export function shortAddress(a: string): string {
  return a.length > 10 ? `${a.slice(0, 4)}..${a.slice(-4)}` : a;
}

export function ConnectButton() {
  const wallets = useWallets(client);
  const connected = useConnectedWallet(client);
  const connect = useConnect(client);
  const disconnect = useDisconnect(client);
  const [open, setOpen] = useState(false);

  if (connected) {
    return (
      <button className="btn" onClick={() => disconnect.dispatch()} title="Disconnect">
        {shortAddress(connected.account.address)}
      </button>
    );
  }
  return (
    <div className="wallet-menu">
      <button className="btn primary" disabled={connect.isRunning} onClick={() => setOpen((o) => !o)}>
        {connect.isRunning ? "Connecting..." : "Connect wallet"}
      </button>
      {open && (
        <ul className="wallet-list">
          {wallets.length === 0 && <li className="muted">No Wallet Standard wallet found</li>}
          {wallets.map((w) => (
            <li key={w.name}>
              <button
                className="btn"
                onClick={() => {
                  setOpen(false);
                  connect.dispatch(w);
                }}
              >
                {w.icon && <img src={w.icon} alt="" width={16} height={16} />} {w.name}
              </button>
            </li>
          ))}
        </ul>
      )}
      {connect.error != null && <span className="error">{String((connect.error as Error).message ?? connect.error)}</span>}
    </div>
  );
}
