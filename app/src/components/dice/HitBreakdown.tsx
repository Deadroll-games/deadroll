import { Applied, HitOutcome, ModKind, type HitResult, type PacketResult, type StateRoll, type TimedMod } from "../../chain/types";
import { aspectName, carrierName, formatBp, formatScaled, resonanceName, stateName } from "../../chain/names";
import { MOD_NAMES, modEffectText } from "../../chain/support";
import { itemLabel, UNARMED, weaponText } from "../../chain/items";
import { chanceView, tintLabel, tintView, type ChanceView } from "./cascade";
import { RollBar } from "./RollBar";
import type { HitBreakdownProps, HitItemSides, HitMods } from "./types";
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

const find = (mods: TimedMod[] | undefined, kind: ModKind) => mods?.find((m) => m.turns > 0 && m.kind === kind);
const named = (m: TimedMod) => `${MOD_NAMES[m.kind]} ${modEffectText(m)}`;

export type ModLine = { side: "Attacker" | "Target"; text: string; applied: boolean };

/** Where each mod of the attacker and the target went in this hit (deadroll_combat::support). */
export function modLines(hit: HitResult, mods: HitMods): ModLine[] {
  const hits = hit.packets.filter((p) => !p.immune);
  const any = hits.length > 0;
  const out: ModLine[] = [];
  for (const m of mods.attacker.filter((x) => x.turns > 0)) {
    const text =
      m.kind === ModKind.Empowerment ? "in Increased of every packet"
      : m.kind === ModKind.Weakening ? "in Weaken of every packet"
      : m.kind === ModKind.Haste || m.kind === ModKind.Slow ? "action points only, not damage"
      : "the attacker's own defense, not this hit";
    out.push({ side: "Attacker", text: `${named(m)}: ${text}`, applied: any && (m.kind === ModKind.Empowerment || m.kind === ModKind.Weakening) });
  }
  for (const m of mods.target.filter((x) => x.turns > 0)) {
    const aspect = aspectName(m.aspect);
    let text: string;
    let applied = false;
    if (m.kind === ModKind.Protection) [text, applied] = ["in Resist of every packet", any];
    else if (m.kind === ModKind.Vulnerability) {
      applied = hits.some((p) => p.aspect === m.aspect);
      text = applied ? `in Vulnerability of the ${aspect} packet` : `no ${aspect} packet in this hit, does nothing`;
    } else if (m.kind === ModKind.Haste || m.kind === ModKind.Slow) text = "action points only, not damage";
    else text = "the target's own attacks, not this hit";
    out.push({ side: "Target", text: `${named(m)}: ${text}`, applied });
  }
  return out;
}

/** What the worn items of each player side put into this hit (HitResolved.items). */
export function itemLines(hit: HitResult, items: HitItemSides): ModLine[] {
  const out: ModLine[] = [];
  const a = items.attacker;
  if (a) {
    out.push({
      side: "Attacker",
      text: a.weapon ? `${itemLabel(a.weapon)}: the damage base (${weaponText(a.weapon) ?? "?"})` : `bare hands: the damage base (Blunt ${UNARMED[0]}-${UNARMED[1]})`,
      applied: true,
    });
    if (a.attack > 0) out.push({ side: "Attacker", text: `+${a.attack} attack from items, in the total ${hit.attackTotal}`, applied: true });
  }
  const t = items.target;
  if (t) {
    const physical = hit.packets.some((p) => !p.immune && p.armor > 0);
    out.push({
      side: "Target",
      text: t.armor > 0 ? `+${t.armor} armor from items${physical ? ", in Armor of the physical packets" : ", no physical packet in this hit"}` : "no armor from items",
      applied: t.armor > 0 && physical,
    });
    if (t.defense > 0) out.push({ side: "Target", text: `+${t.defense} defense from items, in the defense ${hit.defense}`, applied: true });
  }
  return out;
}

/** " (incl. Vulnerability 25%)" after a packet row the mod fed. */
function note(mods: HitMods | undefined, row: "increased" | "weaken" | "resist" | "vuln", p: PacketResult): string {
  const m =
    row === "increased" ? find(mods?.attacker, ModKind.Empowerment)
    : row === "weaken" ? find(mods?.attacker, ModKind.Weakening)
    : row === "resist" ? find(mods?.target, ModKind.Protection)
    : find(mods?.target, ModKind.Vulnerability);
  if (!m || p.immune || (row === "vuln" && m.aspect !== p.aspect)) return "";
  return ` (incl. ${MOD_NAMES[m.kind]} ${formatBp(m.powerBp)})`;
}

export function HitBreakdown({ hit, defaultOpen = true, critChanceBp, mods, items }: HitBreakdownProps) {
  const lines = mods ? modLines(hit, mods) : [];
  const gear = items ? itemLines(hit, items) : [];
  const itemArmor = items?.target?.armor ?? 0;
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
                <td>
                  +{formatBp(packet.increasedBp)}
                  {note(mods, "increased", packet)}
                </td>
              </tr>
              <tr>
                <th scope="row">More</th>
                <td>+{formatBp(packet.moreBp)}</td>
              </tr>
              <tr>
                <th scope="row">Weaken</th>
                <td>
                  {formatBp(packet.weakenBp)}
                  {note(mods, "weaken", packet)}
                </td>
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
                <td>
                  {packet.armor}
                  {itemArmor > 0 && packet.armor > 0 ? ` (incl. items ${itemArmor})` : ""}
                  {packet.penAttackerBp > 0 ? `, attacker ${formatBp(packet.penAttackerBp)}` : ""}
                  {packet.penStateBp > 0 ? `, state ${formatBp(packet.penStateBp)}` : ""}
                </td>
              </tr>
              <tr>
                <th scope="row">Resist</th>
                <td>
                  {formatBp(packet.resistBp)}
                  {note(mods, "resist", packet)}
                </td>
              </tr>
              <tr>
                <th scope="row">Vulnerability</th>
                <td>
                  {formatBp(packet.vulnBp)}
                  {note(mods, "vuln", packet)}
                  {packet.vulnShockBp > 0 ? `, Shock ${formatBp(packet.vulnShockBp)}` : ""}
                </td>
              </tr>
              <tr>
                <th scope="row">After defense</th>
                <td>{packet.immune ? "immune" : formatScaled(packet.afterDefense)}</td>
              </tr>
            </tbody>
          </table>
        ))}

        {items && (
          <section data-testid="hit-items">
            <h3 className="dice-subhead">Items</h3>
            {gear.length === 0 ? (
              <p className="muted dice-line">None</p>
            ) : (
              <ul className="dice-list">
                {gear.map((l, i) => (
                  <li key={i} className={l.applied ? "" : "muted"}>
                    {l.side}: {l.text}
                  </li>
                ))}
              </ul>
            )}
          </section>
        )}

        {mods && (
          <section data-testid="hit-mods">
            <h3 className="dice-subhead">Modifiers</h3>
            {lines.length === 0 ? (
              <p className="muted dice-line">None</p>
            ) : (
              <ul className="dice-list">
                {lines.map((l, i) => (
                  <li key={i} className={l.applied ? "" : "muted"}>
                    {l.side}: {l.text}
                  </li>
                ))}
              </ul>
            )}
          </section>
        )}

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
