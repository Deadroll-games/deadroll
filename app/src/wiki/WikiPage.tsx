// The wiki: what is in the game now, as the code has it (app/scripts/wiki.ts builds the data from
// crates/types, crates/combat, BalanceConfig and the registry, with the short texts of
// assets/wiki). No RPC.
import { useState } from "react";
import wiki from "virtual:wiki";
import { Category, Form, Purpose, SKILL_ERROR_MESSAGES, Source, derive, makeBlocks, validate } from "../chain/skillcheck";
import { formatBp } from "../chain/names";
import type { Aspect } from "../chain/types";
import "./wiki.css";

const SECTIONS = [
  ["aspects", "Aspects"],
  ["states", "States"],
  ["resonances", "Resonances"],
  ["preview", "Skill preview"],
  ["tags", "Tags and forms"],
  ["effects", "Effects and triggers"],
  ["hit", "How a hit is counted"],
  ["turns", "Turns and action points"],
  ["numbers", "All the numbers"],
] as const;

export default function WikiPage() {
  return (
    <div className="wiki">
      <nav className="wiki-toc panel">
        <h2>Wiki</h2>
        <ul>
          {SECTIONS.map(([id, label]) => (
            <li key={id}>
              <a href={`#${id}`}>{label}</a>
            </li>
          ))}
        </ul>
        <p className="muted">Built from the game's code and data: it shows what is in the game now.</p>
      </nav>
      <article className="wiki-body">
        <Aspects />
        <States />
        <Resonances />
        <SkillPreview />
        <Tags ids={["Source", "Form", "TimeTag", "RangeTag", "Target", "Category", "Purpose"]} id="tags" title="Tags and forms" />
        <Tags ids={["Effect", "Trigger"]} id="effects" title="Effects and triggers" />
        <Hit />
        <Turns />
        <Numbers />
      </article>
    </div>
  );
}

function Aspects() {
  return (
    <section id="aspects">
      <h2>Aspects</h2>
      <p className="muted">{wiki.enums.Aspect?.doc}</p>
      <table className="wiki-table">
        <thead>
          <tr>
            <th>Aspect</th>
            <th>Family</th>
            <th>Native state</th>
            <th>As a tint</th>
            <th>Condition of its pairs</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {wiki.aspects.map((a) => (
            <tr key={a.name}>
              <td>
                <b>{a.name}</b>
              </td>
              <td>{a.family}</td>
              <td>{a.native}</td>
              <td>{a.trait}</td>
              <td className="mono">{a.condition}</td>
              <td>{a.about}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

function States() {
  return (
    <section id="states">
      <h2>States</h2>
      <p className="muted">{wiki.enums.StateKind?.doc}</p>
      <table className="wiki-table">
        <thead>
          <tr>
            <th>State</th>
            <th>Turns</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {wiki.states.map((s) => (
            <tr key={s.name}>
              <td>
                <b>{s.name}</b>
              </td>
              <td>{s.turns ?? ""}</td>
              <td>{s.about}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

const opText = (op: unknown): string => {
  const o = op as { kind: string; arg?: string; value?: number; crit?: boolean };
  return `${o.kind}${o.arg ? ` ${o.arg}` : ""}${o.value !== undefined ? ` ${o.value}` : ""}${o.crit ? " (crit)" : ""}`;
};

function Resonances() {
  const named = wiki.resonances.filter((r) => r.name);
  const generated = wiki.resonances.filter((r) => !r.name);
  return (
    <section id="resonances">
      <h2>Resonances</h2>
      <p>
        Each aspect of a skill is a base; the other aspects of the character (race, class, items) are tints. A tint that rolls under its chance makes a
        resonance of its base: the base's native state plus the tint's trait, unless the pair has its own mechanics below.
      </p>
      <table className="wiki-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Base</th>
            <th>Tint</th>
            <th>Condition</th>
            <th>In the code</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {named.map((r) => (
            <tr key={`${r.base}-${r.tint}`}>
              <td>
                <b>{r.name}</b>
              </td>
              <td>{r.base}</td>
              <td>{r.tint}</td>
              <td className="mono">{r.condition}</td>
              <td className="mono">{r.ops.map(opText).join(", ")}</td>
              <td>{r.about}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {generated.length > 0 && (
        <p className="muted">
          Pairs from the tint's trait alone: {generated.map((r) => `${r.base} with ${r.tint}`).join(", ")}.
        </p>
      )}
      <h3>Tint traits</h3>
      <ul>
        {wiki.traits.map((t) => (
          <li key={t.trait}>
            <b>{t.trait}</b> ({t.aspect}, "{t.epithet}"): {t.hit ? `on every hit ${opText(t.hit)}; ` : ""}on a base {(t.template ?? []).map(opText).join(", ")}
          </li>
        ))}
      </ul>
    </section>
  );
}

/** Up to three aspects in order: what a damage skill with them is, from the skill builder's rules. */
function SkillPreview() {
  const names = wiki.enums.Aspect?.variants.map((v) => v.name) ?? [];
  const [picked, setPicked] = useState<number[]>([]);
  const toggle = (i: number) => setPicked((p) => (p.includes(i) ? p.filter((x) => x !== i) : p.length < 3 ? [...p, i] : p));
  const blocks = picked.length ? makeBlocks(Category.Active, Purpose.Damage, Source.Magical, Form.Projectile, picked as Aspect[]) : null;
  const error = blocks ? validate(blocks) : null;
  const d = blocks && !error ? derive(blocks) : null;
  return (
    <section id="preview">
      <h2>Skill preview</h2>
      <p className="muted">Pick up to three aspects in order (the first is the main one). The numbers are those of an active damage skill on one target.</p>
      <div className="row">
        {names.map((n, i) => (
          <button key={n} type="button" className="btn" aria-pressed={picked.includes(i)} onClick={() => toggle(i)}>
            {picked.includes(i) ? `${picked.indexOf(i) + 1}. ` : ""}
            {n}
          </button>
        ))}
        {picked.length > 0 && (
          <button type="button" className="btn" onClick={() => setPicked([])}>
            Clear
          </button>
        )}
      </div>
      {error && <p className="error">{SKILL_ERROR_MESSAGES[error]}</p>}
      {d && (
        <div className="panel wiki-preview">
          <p>
            Damage shares:{" "}
            {picked.map((a, i) => `${names[a]} ${formatBp(d.sharesBp[i] ?? 0)}`).join(" · ")} · power {formatBp(d.powerBp)} · {d.apCost} action points ·
            cooldown {d.cooldown} {d.cooldown === 1 ? "turn" : "turns"}
          </p>
          {d.fusion > 0 && (
            <p className="muted">
              Fusion {names[picked[0]!]} + {names[picked[1]!]} (id {d.fusion}): stored with the skill; it does nothing in combat yet.
            </p>
          )}
          {picked.map((a) => {
            const aspect = wiki.aspects[a]!;
            const pairs = wiki.resonances.filter((r) => r.base === aspect.name && r.name);
            return (
              <div key={a}>
                <h4>
                  {aspect.name}: native state {aspect.native}
                </h4>
                <p className="muted">As a base, with a tint from the character:</p>
                <ul>
                  {pairs.map((r) => (
                    <li key={r.tint}>
                      {r.tint} tint: <b>{r.name}</b>. {r.about}
                    </li>
                  ))}
                </ul>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}

function Tags({ ids, id, title }: { ids: string[]; id: string; title: string }) {
  return (
    <section id={id}>
      <h2>{title}</h2>
      {ids.map((name) => {
        const e = wiki.enums[name];
        if (!e) return null;
        return (
          <div key={name}>
            <h3>{name}</h3>
            {e.doc && <p className="muted">{e.doc}</p>}
            <ul>
              {e.variants.map((v) => (
                <li key={v.name}>
                  <b>{v.name}</b>
                  {v.about ? `: ${v.about}` : ""}
                </li>
              ))}
            </ul>
          </div>
        );
      })}
    </section>
  );
}

function Hit() {
  return (
    <section id="hit">
      <h2>How a hit is counted</h2>
      <p className="muted">The steps of the hit as the program runs them (crates/combat, resolve_hit), in this order. One VRF value per action, every die derived from it.</p>
      <ol className="wiki-steps">
        {wiki.hitSteps.map((s) => (
          <li key={s.steps}>
            <span className="mono">step {s.steps}</span> {s.text[0]!.toUpperCase() + s.text.slice(1)}
          </li>
        ))}
      </ol>
    </section>
  );
}

const field = (group: string, name: string) => wiki.numbers.find((g) => g.group.startsWith(group))?.fields.find((f) => f.field === name)?.value;

function Turns() {
  const apBase = field("Heroes", "ap_base") as number[][] | undefined;
  const races = wiki.enums.Race?.variants.map((v) => v.name) ?? [];
  const classes = wiki.enums.Class?.variants.map((v) => v.name) ?? [];
  return (
    <section id="turns">
      <h2>Turns and action points</h2>
      <p>
        One action per turn: a move, the basic attack ({String(field("Heroes", "basic_attack_ap"))} action points) or one active skill (its own cost). Action
        points per turn are the base of the race and class, plus one per {String(field("Heroes", "ap_dex_div"))} Dexterity, from{" "}
        {String(field("Heroes", "ap_min"))} to {String(field("Heroes", "ap_max"))}. Initiative is Intelligence / {String(field("Heroes", "initiative_div"))}: who acts
        more often. At the start of its turn a unit takes over-time damage, skips the turn if frozen or stunned, and every state, buff and cooldown goes down by
        one.
      </p>
      {apBase && (
        <table className="wiki-table">
          <thead>
            <tr>
              <th>Base action points</th>
              {classes.map((c) => (
                <th key={c}>{c}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {races.map((r, i) => (
              <tr key={r}>
                <td>{r}</td>
                {classes.map((c, j) => (
                  <td key={c}>{apBase[i]?.[j]}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

function Numbers() {
  return (
    <section id="numbers">
      <h2>All the numbers</h2>
      <p className="muted">BalanceConfig as the programs run it. bp: 10000 = 100%.</p>
      {wiki.numbers.map((g) => (
        <details key={g.group}>
          <summary>{g.group}</summary>
          <table className="wiki-table">
            <tbody>
              {g.fields.map((f) => (
                <tr key={f.field}>
                  <td className="mono">{f.field}</td>
                  <td className="mono">{JSON.stringify(f.value)}</td>
                  <td>{f.doc}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </details>
      ))}
    </section>
  );
}
