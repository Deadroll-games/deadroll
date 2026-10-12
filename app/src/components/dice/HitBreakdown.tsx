import { Applied, HitOutcome, type StateRoll } from "../../chain/types";
import { aspectName, carrierName, formatBp, formatScaled, resonanceName, stateName } from "../../chain/names";
import { chanceView, tintLabel, tintView, type ChanceView } from "./cascade";
import { RollBar } from "./RollBar";
import type { HitBreakdownProps } from "./types";
import "./dice.css";

function outcomeLabel(outcome: HitOutcome): string {
  if (outcome === HitOutcome.Miss) return "Miss";
  if (outcome === HitOutcome.Immune) return "Immune";
  return "Hit";
}

function appliedLabel(applied: Applied): string {
  if (applied === Applied.Added) return "added";
  if (applied === Applied.Refreshed) return "refreshed";
  if (applied === Applied.Froze) return "froze";
  return "none";
}

/** "Crit: needs 95.00+, rolled 81.04, fail" with the raw roll in the tooltip and a bar below. */
function RollLine({ label, view, suffix = "" }: { label: string; view: ChanceView; suffix?: string }) {
  return (
    <span className="dice-roll-line" title={view.title}>
      {`${label}: ${view.target}, ${view.value}, ${view.flag}${suffix}`}
      <RollBar bar={view.bar} />
    </span>
  );
}

function StateRollLine({ roll }: { roll: StateRoll }) {
  return <RollLine label={stateName(roll.kind)} view={chanceView(roll.roll, roll.chanceBp, roll.success)} suffix={`, ${appliedLabel(roll.applied)}`} />;
}

export function HitBreakdown({ hit, defaultOpen = true, critChanceBp }: HitBreakdownProps) {
  const resonances = hit.resonances.filter((r) => r != null);
  const stateRolls = [...hit.packets.map((p) => p.state), ...hit.resonanceStates].filter((roll) => roll.rolled);

  return (
    <details className="dice-breakdown panel" open={defaultOpen}>
      <summary>Hit breakdown</summary>
      <div className="dice-break-body">
        <p className="dice-line">
          To hit: d20 {hit.d20}, total {hit.attackTotal} vs defense {hit.defense}, {outcomeLabel(hit.outcome)}
        </p>
        {hit.critRoll !== null ? (
          <p className="dice-line">
            <RollLine label="Crit" view={chanceView(hit.critRoll, critChanceBp ?? null, hit.crit, "CRIT!")} />
          </p>
        ) : (
          <p className="dice-line">Crit: {hit.crit ? "yes" : "no"}</p>
        )}

        {hit.tintRolls.length > 0 && (
          <ul className="dice-list">
            {hit.tintRolls.map((tint, i) => (
              <li key={i}>
                <RollLine label={tintLabel(tint)} view={tintView(tint)} />
              </li>
            ))}
          </ul>
        )}

        {hit.packets.map((packet, i) => (
          <table key={i} className="dice-packet" data-testid="packet">
            <caption>
              {aspectName(packet.aspect)}
              {packet.immune ? " (immune)" : ""}
            </caption>
            <tbody>
              <tr>
                <th scope="row">Base range</th>
                <td>
                  {formatScaled(packet.baseMin)}-{formatScaled(packet.baseMax)}
                </td>
              </tr>
              <tr>
                <th scope="row">Increased</th>
                <td>+{formatBp(packet.increasedBp)}</td>
              </tr>
              <tr>
                <th scope="row">More</th>
                <td>+{formatBp(packet.moreBp)}</td>
              </tr>
              <tr>
                <th scope="row">Weaken</th>
                <td>{formatBp(packet.weakenBp)}</td>
              </tr>
              <tr>
                <th scope="row">Final range</th>
                <td>
                  {formatScaled(packet.min)}-{formatScaled(packet.max)}
                </td>
              </tr>
              <tr>
                <th scope="row">Rolled</th>
                <td>{formatScaled(packet.rolled)}</td>
              </tr>
              <tr>
                <th scope="row">Armor</th>
                <td>{packet.armor}</td>
              </tr>
              <tr>
                <th scope="row">Resist</th>
                <td>{formatBp(packet.resistBp)}</td>
              </tr>
              <tr>
                <th scope="row">Vulnerability</th>
                <td>{formatBp(packet.vulnBp)}</td>
              </tr>
              <tr>
                <th scope="row">After defense</th>
                <td>{packet.immune ? "immune" : formatScaled(packet.afterDefense)}</td>
              </tr>
            </tbody>
          </table>
        ))}

        <section>
          <h3 className="dice-subhead">Resonances</h3>
          {resonances.length === 0 ? (
            <p className="muted dice-line">None</p>
          ) : (
            <ul className="dice-list">
              {hit.resonances.map((resonance, i) =>
                resonance == null ? null : (
                  <li key={i}>
                    {resonanceName(resonance.id)}, tier {resonance.tier}, {carrierName(resonance.carrier)}, second tint{" "}
                    {resonance.secondTint == null ? "none" : aspectName(resonance.secondTint)}
                    {resonance.active ? "" : ", inactive"}
                  </li>
                ),
              )}
            </ul>
          )}
        </section>

        <section>
          <h3 className="dice-subhead">State rolls</h3>
          {stateRolls.length === 0 ? (
            <p className="muted dice-line">None</p>
          ) : (
            <ul className="dice-list">
              {stateRolls.map((roll, i) => (
                <li key={i}>
                  <StateRollLine roll={roll} />
                </li>
              ))}
            </ul>
          )}
        </section>

        <section>
          <h3 className="dice-subhead">Target states</h3>
          {hit.states.length === 0 ? (
            <p className="muted dice-line">None</p>
          ) : (
            <ul className="dice-list">
              {hit.states.map((state, i) => (
                <li key={i}>
                  {stateName(state.kind)}, turns {state.turns}, power {state.power}
                </li>
              ))}
            </ul>
          )}
        </section>

        <p className="dice-line">Total damage: {hit.damage}</p>
        <p className="dice-line">Rolls used: {hit.rollsUsed}</p>
      </div>
    </details>
  );
}
