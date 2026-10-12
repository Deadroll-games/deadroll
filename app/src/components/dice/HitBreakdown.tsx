import { Applied, HitOutcome, type StateRoll } from "../../chain/types";
import { aspectName, carrierName, formatBp, formatScaled, resonanceName, stateName } from "../../chain/names";
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

function stateRollLine(roll: StateRoll): string {
  const verdict = roll.success ? "pass" : "fail";
  return `${stateName(roll.kind)}, roll ${roll.roll} < ${roll.chanceBp} (${formatBp(roll.chanceBp)}), ${verdict}, ${appliedLabel(roll.applied)}`;
}

export function HitBreakdown({ hit, defaultOpen = true }: HitBreakdownProps) {
  const resonances = hit.resonances.filter((r) => r != null);
  const stateRolls = [...hit.packets.map((p) => p.state), ...hit.resonanceStates].filter((roll) => roll.rolled);

  return (
    <details className="dice-breakdown panel" open={defaultOpen}>
      <summary>Hit breakdown</summary>
      <div className="dice-break-body">
        <p className="dice-line">
          To hit: d20 {hit.d20}, total {hit.attackTotal} vs defense {hit.defense}, {outcomeLabel(hit.outcome)}
        </p>
        <p className="dice-line">
          Crit: {hit.crit ? "yes" : "no"}
          {hit.critRoll !== null ? `, roll ${hit.critRoll}` : ""}
        </p>

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
                <li key={i}>{stateRollLine(roll)}</li>
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
