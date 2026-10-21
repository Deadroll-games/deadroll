import { ASPECT_NAMES, STATE_NAMES, TAG_NAMES, formatBp, maskNames, stateName } from "../../chain/names";
import { ASPECT_STATE_SHARE_BP, deltaText, type AspectStat, type UnitStats } from "../../chain/stats";
import { modIsBuff, modLabel } from "../../chain/support";
import type { StatsPanelProps, StatsUnit } from "./types";
import "./stats.css";

const pct = (bp: number) => formatBp(bp);

/** Resistances: the special ones (its own, or a Resistance on that aspect) by name; a flat
 * Protection on the rest collapses into one line. */
function resistLines(s: UnitStats): { text: string; tone: "good" | "" }[] {
  const moved = (r: AspectStat) => r.final !== r.base;
  const own = (r: AspectStat) => r.base > 0 || r.by.includes("Resistance");
  const special = s.resist.filter(own);
  const rest = s.resist.filter((r) => !own(r));
  const lines = special.map((r) => ({ text: deltaText(ASPECT_NAMES[r.aspect], r, pct), tone: moved(r) ? ("good" as const) : ("" as const) }));
  const flat = rest[0];
  if (flat && flat.final > 0) {
    const label = special.length ? "other aspects" : "all aspects";
    lines.push({ text: deltaText(label, flat, pct), tone: "good" });
  }
  return lines;
}

function UnitRow({ u }: { u: StatsUnit }) {
  const s = u.stats;
  const ap = deltaText("AP", s.ap);
  const resist = resistLines(s);
  const vuln = s.vuln.filter((v) => v.final > 0);
  // Patch 9b: states resisted above the general state resistance.
  const stateExtra = s.stateResistByKind.flatMap((bp, k) => (bp > s.stateResist ? [`${STATE_NAMES[k] ?? "?"} ${pct(bp)}`] : []));
  const tags = s.tagResist.flatMap((bp, i) => (bp > 0 ? [`${TAG_NAMES[i] ?? "?"} ${pct(bp)}`] : []));
  const immune = [...maskNames(s.immuneAspects, ASPECT_NAMES), ...maskNames(s.immuneStates, STATE_NAMES), ...maskNames(s.immuneTags, TAG_NAMES)];
  return (
    <tr className={`stats-row ${u.kind}${u.current ? " current" : ""}`} data-testid="stats-row">
      <th scope="row">{u.label}</th>
      <td>
        HP {s.hp}/{s.hpMax}
      </td>
      <td className={s.ap.final > s.ap.base ? "good" : s.ap.final < s.ap.base ? "bad" : ""}>
        {ap}
        {s.ap.now !== null && <span className="muted"> · now {s.ap.now}</span>}
      </td>
      <td>
        Def {s.defense} · Armor {s.armor}
      </td>
      <td>
        {resist.length === 0 ? (
          <span className="muted">Resist none</span>
        ) : (
          resist.map((r, i) => (
            <div key={i} className={r.tone}>
              Resist {r.text}
            </div>
          ))
        )}
        {vuln.map((v) => (
          <div key={v.aspect} className="bad">
            Vuln {deltaText(ASPECT_NAMES[v.aspect], v, pct)}
          </div>
        ))}
        {tags.length > 0 && <div>Resist {tags.join(", ")}</div>}
        {s.stateResist > 0 && <div>State resist {pct(s.stateResist)}</div>}
        {stateExtra.length > 0 && <div>State resist {stateExtra.join(", ")}</div>}
        {immune.length > 0 && <div className="good">Immune {immune.join(", ")}</div>}
      </td>
      <td>
        {s.increasedBp > 0 && <div className="good">Damage +{pct(s.increasedBp)} (Empowerment)</div>}
        {s.weakenBp > 0 && <div className="bad">Weaken {pct(s.weakenBp)} (Weakening)</div>}
        {s.increasedBp === 0 && s.weakenBp === 0 && <span className="muted">Damage x1</span>}
      </td>
      <td>
        {s.mods.length === 0 && s.states.length === 0 ? (
          <span className="muted">none</span>
        ) : (
          <>
            {s.states.map((st) => (
              <span key={st.kind} className="stats-mod bad">
                {stateName(st.kind)} {st.turns}t
              </span>
            ))}
            {s.mods.map((m) => (
              <span key={m.kind} className={`stats-mod ${modIsBuff(m.kind) ? "good" : "bad"}`}>
                {modLabel(m)}
              </span>
            ))}
          </>
        )}
      </td>
    </tr>
  );
}

/** Every unit's combat stats with its buffs and debuffs: base -> final and the mod that moved it. */
export function StatsPanel({ units }: StatsPanelProps) {
  if (units.length === 0) return null;
  return (
    <div className="panel stats-panel">
      <h3>Combat stats</h3>
      <div className="stats-scroll">
        <table className="stats-table mono">
          <thead>
            <tr>
              <th scope="col">Unit</th>
              <th scope="col">HP</th>
              <th scope="col">AP / turn</th>
              <th scope="col">Defense</th>
              <th scope="col">Resist / vuln</th>
              <th scope="col">Outgoing</th>
              <th scope="col">Buffs and debuffs</th>
            </tr>
          </thead>
          <tbody>
            {units.map((u) => (
              <UnitRow key={u.unit} u={u} />
            ))}
          </tbody>
        </table>
      </div>
      <p className="muted">
        Resist divides the damage of its aspect, Vuln multiplies it; Vulnerability counts only for its own aspect. State resist divides the
        chance of states and effects; {pct(ASPECT_STATE_SHARE_BP)} of an aspect's resist also
        divides the chance of that aspect's own state. Immune: no damage of that aspect or tag, that state never lands. (n): own turns left after the current
        one.
      </p>
    </div>
  );
}
