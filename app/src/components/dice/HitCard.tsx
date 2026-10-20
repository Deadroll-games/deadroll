import { useEffect, useMemo, useState } from "react";
import type { EffectResolved } from "../../chain/support";
import type { HitResult } from "../../chain/types";
import { cascadeSteps, effectSteps, type CascadeStep } from "./cascade";
import { HitBreakdown } from "./HitBreakdown";
import type { HitBreakdownProps } from "./types";
import "./dice.css";

/** How long each roll of a hit stays on the card before the next one replaces it. */
export const HIT_STEP_MS = 1500;

/** Rolls that already played in this page: a card mounted again for one shows its result at once. */
const played = new Set<string>();

function prefersReducedMotion(): boolean {
  try {
    return typeof window !== "undefined" && typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

type Props = {
  /** Identity of the roll: a card with a key that already played does not play again. */
  rollKey: string;
  header: string;
  hit?: HitResult;
  effect?: EffectResolved;
  critChanceBp?: number;
  /** The full list of rolls under "Details" (hits only). */
  breakdown?: Omit<HitBreakdownProps, "hit" | "defaultOpen" | "critChanceBp">;
  stepMs?: number;
};

function Step({ step }: { step: CascadeStep }) {
  const tone = step.kind === "damage" ? "dice-gold" : step.success === true ? "dice-good" : step.success === false ? "dice-bad" : "";
  return (
    <p className="hit-card-step" data-kind={step.kind} title={step.title}>
      <span className="dice-label">{step.label}</span> <span className={`dice-value ${tone}`}>{step.value}</span>
      {step.target != null && <span className="dice-target"> {step.target}</span>}
      {step.success === true && <span className="dice-flag dice-good"> {step.flag ?? "pass"}</span>}
      {step.success === false && <span className="dice-flag dice-bad"> {step.flag ?? "fail"}</span>}
    </p>
  );
}

/**
 * One hit (or one effect) as a small card. The rolls show one at a time, each replacing the last,
 * and the card ends on the result. It plays once per `rollKey`; a new roll is a new card (key it).
 */
export function HitCard({ rollKey, header, hit, effect, critChanceBp, breakdown, stepMs = HIT_STEP_MS }: Props) {
  const steps = useMemo(() => (hit ? cascadeSteps(hit, critChanceBp) : effect ? effectSteps(effect, critChanceBp) : []), [hit, effect, critChanceBp]);
  const last = Math.max(0, steps.length - 1);
  const [at, setAt] = useState(() => (played.has(rollKey) || prefersReducedMotion() ? last : 0));
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (at >= last) {
      played.add(rollKey);
      return;
    }
    const id = window.setTimeout(() => setAt(at + 1), stepMs);
    return () => window.clearTimeout(id);
  }, [at, last, rollKey, stepMs]);

  const step = steps[Math.min(at, last)];
  return (
    <section className="hit-card panel" aria-label={hit ? "Hit" : "Effect rolls"}>
      <p className="hit-card-head">{header}</p>
      <div aria-live="polite">{step && <Step key={step.key} step={step} />}</div>
      <button type="button" className="btn small" aria-expanded={open} onClick={() => setOpen((v) => !v)}>
        {open ? "Hide details" : "Details"}
      </button>
      {open &&
        (hit ? (
          <HitBreakdown hit={hit} defaultOpen critChanceBp={critChanceBp} {...breakdown} />
        ) : (
          <ol className="hit-card-list">
            {steps.map((s) => (
              <li key={s.key}>
                <Step step={s} />
              </li>
            ))}
          </ol>
        ))}
    </section>
  );
}
