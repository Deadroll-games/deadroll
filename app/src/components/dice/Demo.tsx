import { useState } from "react";
import { MOCK_HITS } from "../../chain/mock";
import { DiceCascade } from "./DiceCascade";
import { HitBreakdown } from "./HitBreakdown";
import "./dice.css";

type HitKey = keyof typeof MOCK_HITS;

const OPTIONS: { id: HitKey; label: string }[] = [
  { id: "resonance", label: "Resonance hit" },
  { id: "miss", label: "Miss" },
  { id: "crit", label: "Crit" },
];

export default function Demo() {
  const [which, setWhich] = useState<HitKey>("resonance");
  const [instant, setInstant] = useState(false);
  const [replay, setReplay] = useState(0);
  const hit = MOCK_HITS[which];

  return (
    <div className="dice-demo">
      <div className="dice-toolbar">
        {OPTIONS.map((option) => (
          <button
            key={option.id}
            type="button"
            className={which === option.id ? "btn primary" : "btn"}
            aria-pressed={which === option.id}
            onClick={() => setWhich(option.id)}
          >
            {option.label}
          </button>
        ))}
        <button type="button" className="btn" onClick={() => setReplay((n) => n + 1)}>
          Replay
        </button>
        <label className="dice-check">
          <input type="checkbox" checked={instant} onChange={(e) => setInstant(e.target.checked)} />
          instant
        </label>
      </div>
      <DiceCascade
        key={`${which}-${replay}`}
        hit={hit}
        attackerLabel="Aria"
        targetLabel="Wolf"
        instant={instant}
      />
      <HitBreakdown hit={hit} />
    </div>
  );
}
