import { ASPECT_NAMES, CLASS_NAMES, RACE_NAMES, STAT_NAMES } from "../../chain/names";
import type { Aspect, Class, Race } from "../../chain/types";
import { nameBytes, nameValid, pointsLeft } from "./format";
import type { CharacterDraft, CharacterFormProps } from "./types";
import "./forms.css";

function patchStat(stats: CharacterDraft["stats"], index: number, next: number): CharacterDraft["stats"] {
  return stats.map((s, i) => (i === index ? next : s)) as CharacterDraft["stats"];
}

export function CharacterForm(p: CharacterFormProps) {
  const bytes = nameBytes(p.value.name);
  const nameOk = nameValid(p.value.name);
  const left = pointsLeft(p.value.stats, p.statMin, p.statPoints);
  const predOk = p.allowedPredispositions.includes(p.value.predisposition);
  const locked = Boolean(p.busy);
  const canSubmit = nameOk && left === 0 && predOk && !locked;

  function update(next: CharacterDraft) {
    p.onChange(next);
  }

  function changeStat(index: number, delta: number) {
    const current = p.value.stats[index] ?? p.statMin;
    const next = current + delta;
    if (next < p.statMin) return;
    if (delta > 0 && left <= 0) return;
    update({ ...p.value, stats: patchStat(p.value.stats, index, next) });
  }

  return (
    <div className="forms-root forms-split">
      <form
        className="panel forms-card"
        onSubmit={(event) => {
          event.preventDefault();
          if (canSubmit) p.onSubmit(p.value);
        }}
      >
        <label className="forms-field" htmlFor="forms-name">
          Name
          <input
            id="forms-name"
            value={p.value.name}
            disabled={locked}
            aria-invalid={!nameOk}
            onChange={(event) => update({ ...p.value, name: event.target.value })}
          />
        </label>
        <p className={nameOk || p.value.name === "" ? "muted" : "error"}>
          {bytes}/32, plain ASCII letters, digits and punctuation
        </p>

        <div className="forms-field">
          <span>Race</span>
          <div className="forms-seg" role="group" aria-label="Race">
            {RACE_NAMES.map((label, index) => (
              <button
                key={label}
                type="button"
                className="btn"
                aria-pressed={p.value.race === index}
                disabled={locked}
                onClick={() => update({ ...p.value, race: index as Race })}
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        <div className="forms-field">
          <span>Class</span>
          <div className="forms-seg" role="group" aria-label="Class">
            {CLASS_NAMES.map((label, index) => (
              <button
                key={label}
                type="button"
                className="btn"
                aria-pressed={p.value.class === index}
                disabled={locked}
                onClick={() => update({ ...p.value, class: index as Class })}
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        <div className="forms-field">
          <span>Predisposition</span>
          <div className="forms-seg" role="group" aria-label="Predisposition">
            {ASPECT_NAMES.map((label, index) => {
              const aspect = index as Aspect;
              const allowed = p.allowedPredispositions.includes(aspect);
              return (
                <button
                  key={label}
                  type="button"
                  className="btn"
                  aria-pressed={p.value.predisposition === aspect}
                  disabled={locked || !allowed}
                  onClick={() => update({ ...p.value, predisposition: aspect })}
                >
                  {label}
                </button>
              );
            })}
          </div>
        </div>

        <div className="forms-field">
          <span>Stats</span>
          {STAT_NAMES.map((label, index) => {
            const value = p.value.stats[index] ?? p.statMin;
            return (
              <div key={label} className="forms-stat">
                <span>{label}</span>
                <button
                  type="button"
                  className="btn"
                  aria-label={`decrease ${label}`}
                  disabled={locked || value <= p.statMin}
                  onClick={() => changeStat(index, -1)}
                >
                  -
                </button>
                <span className="forms-stat-value mono">{value}</span>
                <button
                  type="button"
                  className="btn"
                  aria-label={`increase ${label}`}
                  disabled={locked || left <= 0}
                  onClick={() => changeStat(index, 1)}
                >
                  +
                </button>
              </div>
            );
          })}
          <p>Points left: {left}</p>
        </div>

        {p.error ? <p className="error">{p.error}</p> : null}

        <button type="submit" className="btn primary" disabled={!canSubmit}>
          {locked ? "Creating..." : "Create character"}
        </button>
      </form>
      <div className="forms-frame panel">{p.preview}</div>
    </div>
  );
}
