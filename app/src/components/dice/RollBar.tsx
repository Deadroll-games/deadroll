// 0..100 track for a chance roll shown "high roll wins": success zone at the top end, marker at the shown roll.
import type { RollBarData } from "./cascade";

const pct = (hundredths: number): string => `${Math.min(100, Math.max(0, hundredths / 100))}%`;

export function RollBar({ bar }: { bar: RollBarData }) {
  return (
    <span className="dice-bar" aria-hidden="true">
      {bar.needed != null ? <span className="dice-bar-zone" style={{ left: pct(bar.needed) }} /> : null}
      <span className={`dice-bar-mark ${bar.success ? "dice-bar-good" : "dice-bar-bad"}`} style={{ left: pct(bar.shown) }} />
    </span>
  );
}
