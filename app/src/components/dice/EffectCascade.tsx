import { useMemo } from "react";
import { StepCascade } from "./DiceCascade";
import { effectSteps } from "./cascade";
import type { EffectCascadeProps } from "./types";
import "./dice.css";

/** The rolls of a heal, buff, debuff or a damage skill's effect (EffectResolved), step by step. */
export function EffectCascade({ effect, casterLabel, targetLabel, skillLabel, stepMs, instant, onDone, critChanceBp }: EffectCascadeProps) {
  const steps = useMemo(() => effectSteps(effect, critChanceBp), [effect, critChanceBp]);
  const header = effect.rider ? `${skillLabel} effect → ${targetLabel}` : `${casterLabel} · ${skillLabel} → ${targetLabel}`;
  return <StepCascade steps={steps} header={header} ariaLabel="Effect rolls" stepMs={stepMs} instant={instant} onDone={onDone} />;
}
