// Patch B: the arena's turn bar (docs/PARTY_AND_TURNS.md 6): the unit acting now, then the next
// turns of the initiative timeline. A fast unit shows up twice when it acts twice.
import type { TurnBarProps } from "./types";

export function TurnBar({ turns, title }: TurnBarProps) {
  if (turns.length === 0) return null;
  return (
    <div className="turn-bar" aria-label={title ?? "Turn order"}>
      <span className="turn-bar-title muted">{title ?? "Next turns"}</span>
      <ol>
        {turns.map((t, i) => (
          <li key={`${i}:${t.id}`} className={`turn-bar-item ${t.kind}${t.now ? " now" : ""}${t.mine ? " mine" : ""}`} title={t.now ? `${t.label}: acting now` : t.label}>
            {t.now && <span className="turn-bar-now">now</span>}
            <span className="turn-bar-label">{t.label}</span>
          </li>
        ))}
      </ol>
    </div>
  );
}
