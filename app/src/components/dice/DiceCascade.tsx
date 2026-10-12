import { useEffect, useMemo, useRef, useState } from "react";
import type { HitResult } from "../../chain/types";
import { cascadeSteps, type CascadeStep } from "./cascade";
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

type View = { hit: HitResult; showAll: boolean; count: number };

export function DiceCascade({
  hit,
  attackerLabel,
  targetLabel,
  stepMs = 450,
  instant = false,
  onDone,
}: DiceCascadeProps) {
  const steps = useMemo(() => cascadeSteps(hit), [hit]);
  const showAll = instant || prefersReducedMotion();
  const finishedHit = useRef<HitResult | null>(null);
  const [view, setView] = useState<View>(() => ({
    hit,
    showAll,
    count: openingCount(showAll, steps.length, false),
  }));

  if (view.hit !== hit || view.showAll !== showAll) {
    setView({
      hit,
      showAll,
      count: openingCount(showAll, steps.length, finishedHit.current === hit),
    });
  }

  const count = view.hit === hit && view.showAll === showAll ? view.count : openingCount(showAll, steps.length, finishedHit.current === hit);

  const onDoneRef = useRef(onDone);
  onDoneRef.current = onDone;

  useEffect(() => {
    if (finishedHit.current === hit) return;

    const finish = () => {
      if (finishedHit.current === hit) return;
      finishedHit.current = hit;
      onDoneRef.current?.();
    };

    if (showAll || steps.length <= 1) {
      finish();
      return;
    }

    let shown = 1;
    const id = window.setInterval(() => {
      shown += 1;
      setView((prev) => (prev.hit === hit && !prev.showAll ? { ...prev, count: shown } : prev));
      if (shown >= steps.length) {
        window.clearInterval(id);
        finish();
      }
    }, stepMs);

    return () => window.clearInterval(id);
  }, [hit, showAll, stepMs, steps.length]);

  const visible = steps.slice(0, count);

  return (
    <section className="dice-cascade panel" aria-label="Dice cascade">
      <p className="dice-header">
        {attackerLabel} → {targetLabel}
      </p>
      <ol className="dice-steps" aria-live="polite">
        {visible.map((step) => (
          <li key={step.key} className="dice-step" data-kind={step.kind}>
            <span className="dice-label">{step.label}</span>
            <StepValue step={step} animate={!showAll} />
            <span className="dice-meta">
              {step.target != null ? <span className="dice-target">{step.target}</span> : null}
              {step.success === true ? <span className="dice-flag dice-good">pass</span> : null}
              {step.success === false ? <span className="dice-flag dice-bad">fail</span> : null}
            </span>
          </li>
        ))}
      </ol>
    </section>
  );
}
