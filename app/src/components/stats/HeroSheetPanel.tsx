// Core stats with the level-up point plan, and the derived stats the plan would give. Pure UI:
// the numbers come from the combat module (props), Save sends the plan.
import { formatBp, STAT_NAMES } from "../../chain/names";
import type { HeroSheetPanelProps, HeroSheetView } from "./types";
import "./stats.css";

type Row = { label: string; value: (s: HeroSheetView) => string; hint: string; group: "general" | "attack" | "defense" };

/** What each derived stat is and where it comes from (docs/CHARACTER.md 2-3). */
const ROWS: Row[] = [
  { label: "HP", value: (s) => String(s.hpMax), hint: "class base, Endurance, level, items", group: "defense" },
  { label: "Accuracy", value: (s) => `+${s.accuracy}`, hint: "d20 bonus: Dexterity, items", group: "attack" },
  { label: "Evasion", value: (s) => String(s.evasion), hint: "the hit threshold: Dexterity, items", group: "defense" },
  { label: "Armor", value: (s) => String(s.armor), hint: "class, items", group: "defense" },
  { label: "Crit chance", value: (s) => formatBp(s.critBp), hint: "besides a natural 20: items", group: "attack" },
  { label: "Action points", value: (s) => String(s.actionPoints), hint: "speed: race, class, Dexterity, items", group: "general" },
  { label: "Initiative", value: (s) => String(s.initiative), hint: "Intelligence, items", group: "general" },
  { label: "Martial strike damage", value: (s) => `+${formatBp(s.martialStrikeBp)}`, hint: "Strength", group: "attack" },
  { label: "Martial projectile damage", value: (s) => `+${formatBp(s.martialProjectileBp)}`, hint: "Dexterity", group: "attack" },
  { label: "Magical damage", value: (s) => `+${formatBp(s.magicalBp)}`, hint: "Intelligence", group: "attack" },
  { label: "Martial state chance", value: (s) => `+${formatBp(s.martialStateBp)}`, hint: "states and debuffs of martial skills: Strength", group: "attack" },
  { label: "Magical state chance", value: (s) => `+${formatBp(s.magicalStateBp)}`, hint: "states and debuffs of magical skills: Intelligence", group: "attack" },
  { label: "Tempo effect chance", value: (s) => `+${formatBp(s.tempoStateBp)}`, hint: "Haste, Slow: Dexterity", group: "attack" },
  { label: "Own buffs on self", value: (s) => `+${s.selfBuffTurns} turns`, hint: "Endurance", group: "general" },
];

function SheetTable({ rows, sheet, planned, spent }: { rows: Row[]; sheet: HeroSheetView; planned: HeroSheetView | null; spent: number }) {
  return (
    <table className="stats-table hero-sheet-table">
      <tbody>
        {rows.map((r) => {
          const now = r.value(sheet);
          const next = planned && spent > 0 ? r.value(planned) : now;
          return (
            <tr key={r.label}>
              <th scope="row">{r.label}</th>
              <td>
                {now}
                {next !== now && <span className="good"> → {next}</span>}
              </td>
              <td className="muted">{r.hint}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

export function HeroSheetPanel({ stats, plan, unspent, sheet, planned, locked, busy, onPlan, onReset, onSave }: HeroSheetPanelProps) {
  const spent = plan.reduce((a, b) => a + b, 0);
  const left = unspent - spent;
  const canPlan = unspent > 0 && locked === null;
  return (
    <div className="hero-sheet">
      <dl className="char-stats">
        {STAT_NAMES.map((name, i) => {
          const add = plan[i] ?? 0;
          return (
            <div key={name} data-testid={`stat-${name}`}>
              <dt>{name}</dt>
              <dd>
                {(stats[i] ?? 0) + add}
                {add > 0 && <span className="good"> (+{add})</span>}
                {canPlan && (
                  <span className="stat-plan">
                    <button type="button" className="btn small" aria-label={`Less ${name}`} disabled={busy || add === 0} onClick={() => onPlan(i, -1)}>
                      −
                    </button>
                    <button type="button" className="btn small" aria-label={`More ${name}`} disabled={busy || left === 0} onClick={() => onPlan(i, 1)}>
                      +
                    </button>
                  </span>
                )}
              </dd>
            </div>
          );
        })}
      </dl>

      {unspent > 0 && (
        <div className="stat-points">
          {locked ? (
            <p className="muted">
              {unspent} unspent point{unspent === 1 ? "" : "s"}. {locked}
            </p>
          ) : (
            <>
              <p>
                {left} of {unspent} point{unspent === 1 ? "" : "s"} left to place. Save writes the whole choice in one transaction.
              </p>
              <div className="row">
                <button type="button" className="btn" disabled={busy || spent === 0} onClick={onReset}>
                  Reset
                </button>
                <button type="button" className="btn primary" disabled={busy || spent === 0} onClick={onSave}>
                  Save
                </button>
              </div>
            </>
          )}
        </div>
      )}

      <h3>Derived stats</h3>
      {sheet ? (
        <>
          <SheetTable rows={ROWS.filter((r) => r.group === "general")} sheet={sheet} planned={planned} spent={spent} />
          <div className="hero-cols">
            <div>
              <h4>Attack</h4>
              <SheetTable rows={ROWS.filter((r) => r.group === "attack")} sheet={sheet} planned={planned} spent={spent} />
            </div>
            <div>
              <h4>Defense</h4>
              <SheetTable rows={ROWS.filter((r) => r.group === "defense")} sheet={sheet} planned={planned} spent={spent} />
            </div>
          </div>
        </>
      ) : (
        <p className="muted">Loading the combat module…</p>
      )}
    </div>
  );
}
