import { useEffect, useMemo, useRef, useState } from "react";
import { cascadeSteps, type CascadeStep } from "./cascade";
import { RollBar } from "./RollBar";
import type { DiceCascadeProps } from "./types";
import "./dice.css";

function prefersReducedMotion(): boolean {
  try {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

function openingCount(showAll: boolean, total: number, alreadyDone: boolean): number {
  if (showAll || alreadyDone) return total;
  return total > 0 ? 1 : 0;
}

/** Deterministic stand-in digits so the tumble never depends on Math.random. */
function flickerDigit(value: string, frame: number): string | null {
  let changed = false;
  let out = "";
  for (let i = 0; i < value.length; i++) {
    const ch = value[i] ?? "";
    if (ch >= "0" && ch <= "9") {
      changed = true;
      const n = Number(ch);
      const next = (n + frame + i) % 9;
      out += String(next >= n ? next + 1 : next);
    } else {
      out += ch;
    }
  }
  return changed ? out : null;
}

function StepValue({ step, animate }: { step: CascadeStep; animate: boolean }) {
  const big = step.kind === "damage";
  const tone = big ? "dice-gold" : step.success === true ? "dice-good" : step.success === false ? "dice-bad" : "";
  const ghosts = animate ? [1, 2, 3].map((frame) => flickerDigit(step.value, frame)) : [];
  const valueClass = ["dice-value", tone, big ? "dice-value-final" : "", animate ? "dice-tumble" : ""].filter(Boolean).join(" ");
  return (
    <span className={valueClass}>
      {ghosts.map((ghost, i) =>
        ghost != null ? <span key={i} className="dice-ghost" data-flicker={ghost} aria-hidden="true" /> : null,
      )}
      <span className="dice-settle">{step.value}</span>
    </span>
  );
}

type View = { steps: CascadeStep[]; showAll: boolean; count: number };

/**
 * Steps revealed one per `stepMs` (all at once with `instant` or reduced motion). A new `steps`
 * array (by identity) restarts the reveal, so callers memoize it on their roll.
 */
export function StepCascade({
  steps,
  header,
  ariaLabel = "Dice cascade",
  stepMs = 450,
  instant = false,
  onDone,
}: {
  steps: CascadeStep[];
  header: string;
  ariaLabel?: string;
  stepMs?: number;
  instant?: boolean;
  onDone?: () => void;
}) {
  const showAll = instant || prefersReducedMotion();
  const finished = useRef<CascadeStep[] | null>(null);
  const [view, setView] = useState<View>(() => ({
    steps,
    showAll,
    count: openingCount(showAll, steps.length, false),
  }));

  if (view.steps !== steps || view.showAll !== showAll) {
    setView({
      steps,
      showAll,
      count: openingCount(showAll, steps.length, finished.current === steps),
    });
  }

  const count = view.steps === steps && view.showAll === showAll ? view.count : openingCount(showAll, steps.length, finished.current === steps);

  const onDoneRef = useRef(onDone);
  onDoneRef.current = onDone;

  useEffect(() => {
    if (finished.current === steps) return;

    const finish = () => {
      if (finished.current === steps) return;
      finished.current = steps;
      onDoneRef.current?.();
    };

    if (showAll || steps.length <= 1) {
      finish();
      return;
    }

    let shown = 1;
    const id = window.setInterval(() => {
      shown += 1;
      setView((prev) => (prev.steps === steps && !prev.showAll ? { ...prev, count: shown } : prev));
      if (shown >= steps.length) {
        window.clearInterval(id);
        finish();
      }
    }, stepMs);

    return () => window.clearInterval(id);
  }, [steps, showAll, stepMs]);

  const visible = steps.slice(0, count);

  return (
    <section className="dice-cascade panel" aria-label={ariaLabel}>
      <p className="dice-header">{header}</p>
      <ol className="dice-steps" aria-live="polite">
        {visible.map((step) => (
          <li key={step.key} className="dice-step" data-kind={step.kind} title={step.title}>
            <span className="dice-label">{step.label}</span>
            <StepValue step={step} animate={!showAll} />
            <span className="dice-meta">
              {step.target != null ? <span className="dice-target">{step.target}</span> : null}
              {step.success === true ? <span className="dice-flag dice-good">{step.flag ?? "pass"}</span> : null}
              {step.success === false ? <span className="dice-flag dice-bad">{step.flag ?? "fail"}</span> : null}
            </span>
            {step.bar != null ? <RollBar bar={step.bar} /> : null}
          </li>
        ))}
      </ol>
    </section>
  );
}

export function DiceCascade({ hit, attackerLabel, targetLabel, stepMs, instant, onDone, critChanceBp }: DiceCascadeProps) {
  const steps = useMemo(() => cascadeSteps(hit, critChanceBp), [hit, critChanceBp]);
  return <StepCascade steps={steps} header={`${attackerLabel} → ${targetLabel}`} stepMs={stepMs} instant={instant} onDone={onDone} />;
}
