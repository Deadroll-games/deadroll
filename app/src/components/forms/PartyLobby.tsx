import { useState } from "react";
import { className, raceName } from "../../chain/names";
import { formatSol, shortAddress } from "./format";
import type { PartyLobbyProps } from "./types";
import "./forms.css";

export function PartyLobby(p: PartyLobbyProps) {
  const [picked, setPicked] = useState(p.myCharacters[0]?.address ?? "");
  const locked = Boolean(p.busy);
  const full = p.players.length >= p.maxPlayers;
  const allReady = p.players.length > 0 && p.players.every((player) => player.ready);
  const me = p.players.find((player) => player.slot === p.mySlot);
  const selected = p.myCharacters.some((character) => character.address === picked)
    ? picked
    : (p.myCharacters[0]?.address ?? "");

  return (
    <div className="forms-root forms-split">
      <section className="panel forms-card">
        <header className="forms-header">
          <h2>{p.mapName}</h2>
          <span>
            DM <span className="mono">{shortAddress(p.dm)}</span>
          </span>
          <span>
            Price <span className="mono">{formatSol(p.priceLamports)} SOL</span>
          </span>
          <span>
            Escrow collected <span className="mono">{formatSol(p.collected)} SOL</span>
          </span>
        </header>

        <ul className="forms-slots">
          {Array.from({ length: p.maxPlayers }, (_, slot) => {
            const player = p.players.find((entry) => entry.slot === slot);
            if (!player) {
              return (
                <li key={slot} className="forms-slot muted">
                  Open slot
                </li>
              );
            }
            return (
              <li key={slot} className="forms-slot">
                <span>{player.characterName}</span>
                <span className="muted">
                  {raceName(player.race)} {className(player.class)}
                </span>
                <span className={player.ready ? "badge forms-ready" : "badge"}>
                  {player.ready ? "Ready" : "Not ready"}
                </span>
                <span className="mono">{formatSol(player.paid)} SOL</span>
              </li>
            );
          })}
        </ul>

        {p.role === "guest" ? (
          <div className="forms-row">
            <label className="forms-field" htmlFor="forms-character">
              Character
              <select
                id="forms-character"
                value={selected}
                disabled={locked || p.myCharacters.length === 0}
                onChange={(event) => setPicked(event.target.value)}
              >
                {p.myCharacters.map((character) => (
                  <option key={character.address} value={character.address}>
                    {character.name}
                  </option>
                ))}
              </select>
            </label>
            <button
              type="button"
              className="btn primary"
              disabled={locked || p.myCharacters.length === 0 || full || selected.length === 0}
              onClick={() => p.onJoin(selected)}
            >
              Join and pay {formatSol(p.joinLamports ?? p.priceLamports)} SOL
            </button>
          </div>
        ) : null}

        {p.role === "player" ? (
          <button
            type="button"
            className="btn primary"
            disabled={locked || !me}
            aria-pressed={Boolean(me?.ready)}
            onClick={() => p.onReady(!me?.ready)}
          >
            {me?.ready ? "Not ready" : "Ready"}
          </button>
        ) : null}

        {p.role === "dm" ? (
          <button type="button" className="btn primary" disabled={locked || !allReady} onClick={() => p.onStart()}>
            Start session
          </button>
        ) : null}

        {p.error ? <p className="error">{p.error}</p> : null}
      </section>
      <div className="forms-frame panel">{p.mapPreview}</div>
    </div>
  );
}
