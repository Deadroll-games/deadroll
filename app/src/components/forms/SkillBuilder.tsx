// Starting skill builder: up to 6 skills checked live by the TS mirror of crates/skillcheck,
// the same rules create_character enforces. Class presets are one-click options.
import { ASPECT_NAMES } from "../../chain/names";
import {
  BUILDER_EFFECTS,
  CATEGORY_NAMES,
  Category,
  EFFECT_NAMES,
  FORM_NAMES,
  HP_BELOW_MAX_PCT,
  HP_BELOW_MIN_PCT,
  MELEE_EXT,
  PRESETS,
  PURPOSE_NAMES,
  Purpose,
  RANGE_NAMES,
  RangeTag,
  SKILL_DESCRIPTION_LEN,
  SKILL_ERROR_MESSAGES,
  SKILL_NAME_LEN,
  SOURCE_NAMES,
  STARTING_MAX,
  Source,
  TARGET_NAMES,
  TIMING_NAMES,
  TRIGGER_NAMES,
  AURA_RADIUS_MAX,
  Effect,
  Form,
  checkLoadout,
  derive,
  makeBlocks,
  rangeReach,
  skillProblems,
  usableInCombat,
  usesThreshold,
  type NamedSkill,
  type SkillBlocks,
  type SkillExt,
} from "../../chain/skillcheck";
import { formatBp } from "../../chain/names";
import { Aspect, Class } from "../../chain/types";

export type SkillBuilderProps = {
  cls: Class;
  skills: NamedSkill[];
  onChange: (skills: NamedSkill[]) => void;
  disabled?: boolean;
};

export type SkillCheck = { perSkill: string[][]; loadout: string | null; ok: boolean };

/** Every problem of a starting set: per skill (name + first failing rule) and the loadout. */
export function checkSkills(skills: NamedSkill[], cls: Class): SkillCheck {
  const perSkill = skills.map((s) => skillProblems(s, cls));
  const tooMany = skills.length > STARTING_MAX ? `At most ${STARTING_MAX} starting skills.` : null;
  const err = checkLoadout(skills.map((s) => s.blocks.category));
  const loadout = tooMany ?? (err ? SKILL_ERROR_MESSAGES[err] : null);
  return { perSkill, loadout, ok: loadout === null && perSkill.every((p) => p.length === 0) };
}

/** A valid new skill for the class to start editing from. */
export function blankSkill(cls: Class, n: number): NamedSkill {
  const description = "My own skill.";
  if (cls === Class.Support)
    return {
      name: `Skill ${n}`,
      blocks: makeBlocks(Category.Active, Purpose.Buff, Source.Magical, Form.Projectile, [Aspect.Lightning], { effect: Effect.Haste }),
      ext: { ...MELEE_EXT, range: RangeTag.Ranged },
      description,
    };
  const source = cls === Class.Warrior ? Source.Combat : Source.Magical;
  return {
    name: `Skill ${n}`,
    blocks: makeBlocks(Category.Active, Purpose.Damage, source, Form.Strike, [cls === Class.Warrior ? Aspect.Piercing : Aspect.Cold]),
    ext: MELEE_EXT,
    description,
  };
}

/** Keeps one canonical encoding: unused aspect slots go back to the default (Fire). */
function withAspectCount(b: SkillBlocks, count: number): SkillBlocks {
  const aspects = b.aspects.map((a, i) => (i < count ? a : Aspect.Fire)) as SkillBlocks["aspects"];
  return { ...b, aspectCount: count, aspects };
}

/** What an active skill costs: its action points (docs/PARTY_AND_TURNS.md 5.1). */
export function apLabel(b: SkillBlocks): string {
  return b.category === Category.Active ? `${derive(b).apCost} AP` : "no AP (not an action)";
}

function preview(b: SkillBlocks, e: SkillExt): string {
  const d = derive(b);
  const shares = b.aspects
    .slice(0, Math.max(1, Math.min(b.aspectCount, 3)))
    .map((a, i) => `${ASPECT_NAMES[a]} ${formatBp(d.sharesBp[i] ?? 0)}`)
    .join(" / ");
  const parts = [`power ${formatBp(d.powerBp)}`, shares, apLabel(b), `cooldown ${d.cooldown}`, `reach ${rangeReach(e.range)}`];
  if (d.fusion) parts.push(`fusion #${d.fusion}`);
  return parts.join(" · ");
}

/** A select over some values of an enum (its names by value). */
function PickOf<T extends number>({ label, value, values, names, onChange, disabled }: { label: string; value: T; values: readonly T[]; names: readonly string[]; onChange: (v: T) => void; disabled?: boolean }) {
  return (
    <label className="skill-pick">
      <span>{label}</span>
      <select aria-label={label} value={value} disabled={disabled} onChange={(e) => onChange(Number(e.target.value) as T)}>
        {values.map((v) => (
          <option key={v} value={v}>
            {names[v]}
          </option>
        ))}
      </select>
    </label>
  );
}

function Pick<T extends number>({ label, value, names, onChange, disabled, max }: { label: string; value: T; names: readonly string[]; onChange: (v: T) => void; disabled?: boolean; max?: number }) {
  return (
    <label className="skill-pick">
      <span>{label}</span>
      <select aria-label={label} value={value} disabled={disabled} onChange={(e) => onChange(Number(e.target.value) as T)}>
        {names.slice(0, max ?? names.length).map((n, i) => (
          <option key={n} value={i}>
            {n}
          </option>
        ))}
      </select>
    </label>
  );
}

const AURA_NAMES = Array.from({ length: AURA_RADIUS_MAX + 1 }, (_, i) => (i === 0 ? "No aura" : `Aura ${i}`));
const COUNT_NAMES = ["1 aspect", "2 aspects", "3 aspects"];

function SkillCard({ skill, index, problems, onChange, onRemove, disabled }: { skill: NamedSkill; index: number; problems: string[]; onChange: (s: NamedSkill) => void; onRemove: () => void; disabled?: boolean }) {
  const b = skill.blocks;
  const e = skill.ext;
  const set = (patch: Partial<SkillBlocks>) => onChange({ ...skill, blocks: { ...b, ...patch } });
  const setExt = (patch: Partial<SkillExt>) => onChange({ ...skill, ext: { ...e, ...patch } });
  const n = index + 1;
  return (
    <li className={`skill-card${problems.length ? " invalid" : ""}`} aria-label={`Skill ${n}`}>
      <div className="skill-card-head">
        <label className="skill-pick skill-name">
          <span>Name</span>
          <input aria-label={`Skill ${n} name`} value={skill.name} maxLength={SKILL_NAME_LEN} disabled={disabled} onChange={(e) => onChange({ ...skill, name: e.target.value })} />
        </label>
        <button type="button" className="btn" disabled={disabled} onClick={onRemove} aria-label={`Remove skill ${n}`}>
          Remove
        </button>
      </div>
      <div className="skill-grid">
        <Pick label={`Skill ${n} category`} value={b.category} names={CATEGORY_NAMES} disabled={disabled} onChange={(category) => set({ category })} />
        <Pick label={`Skill ${n} purpose`} value={b.purpose} names={PURPOSE_NAMES} disabled={disabled} onChange={(purpose) => set({ purpose })} />
        <Pick label={`Skill ${n} source`} value={b.source} names={SOURCE_NAMES} disabled={disabled} onChange={(source) => set({ source })} />
        <Pick label={`Skill ${n} form`} value={b.form} names={FORM_NAMES} disabled={disabled} onChange={(form) => set({ form })} />
        <Pick label={`Skill ${n} target`} value={b.target} names={TARGET_NAMES} disabled={disabled} onChange={(target) => set({ target })} />
        <Pick
          label={`Skill ${n} aspect count`}
          value={Math.min(Math.max(b.aspectCount, 1), 3) - 1}
          names={COUNT_NAMES}
          disabled={disabled}
          onChange={(i) => onChange({ ...skill, blocks: withAspectCount(b, i + 1) })}
        />
        {b.aspects.slice(0, Math.max(1, b.aspectCount)).map((a, i) => (
          <Pick
            key={i}
            label={`Skill ${n} aspect ${i + 1}`}
            value={a}
            names={ASPECT_NAMES}
            disabled={disabled}
            onChange={(aspect) => set({ aspects: b.aspects.map((x, j) => (j === i ? aspect : x)) as SkillBlocks["aspects"] })}
          />
        ))}
        <PickOf label={`Skill ${n} effect`} value={b.effect} values={BUILDER_EFFECTS} names={EFFECT_NAMES} disabled={disabled} onChange={(effect) => set({ effect })} />
        <Pick label={`Skill ${n} range`} value={e.range} names={RANGE_NAMES} disabled={disabled} onChange={(range) => setExt({ range })} />
        <Pick label={`Skill ${n} timing`} value={e.timing} names={TIMING_NAMES} disabled={disabled} onChange={(timing) => setExt({ timing })} />
        <Pick
          label={`Skill ${n} trigger`}
          value={b.trigger}
          names={TRIGGER_NAMES}
          disabled={disabled}
          // A threshold only with an HP trigger; a sensible default when one is picked.
          onChange={(trigger) =>
            onChange({
              ...skill,
              blocks: { ...b, trigger },
              ext: { ...e, hpBelowPct: usesThreshold(trigger) ? e.hpBelowPct || 40 : 0 },
            })
          }
        />
        {usesThreshold(b.trigger) && (
          <label className="skill-pick">
            <span>HP below, %</span>
            <input
              aria-label={`Skill ${n} HP threshold`}
              type="number"
              min={HP_BELOW_MIN_PCT}
              max={HP_BELOW_MAX_PCT}
              value={e.hpBelowPct}
              disabled={disabled}
              onChange={(ev) => setExt({ hpBelowPct: Math.max(0, Math.min(255, Math.floor(Number(ev.target.value) || 0))) })}
            />
          </label>
        )}
        <Pick label={`Skill ${n} aura`} value={Math.min(b.auraRadius, AURA_RADIUS_MAX)} names={AURA_NAMES} disabled={disabled} onChange={(auraRadius) => set({ auraRadius })} />
      </div>
      <label className="skill-pick skill-description">
        <span>
          Description ({skill.description.length}/{SKILL_DESCRIPTION_LEN})
        </span>
        <textarea
          aria-label={`Skill ${n} description`}
          value={skill.description}
          maxLength={SKILL_DESCRIPTION_LEN}
          rows={2}
          disabled={disabled}
          onChange={(ev) => onChange({ ...skill, description: ev.target.value })}
        />
      </label>
      <p className="muted skill-preview">
        {preview(b, e)} · {usableInCombat(b) ? "usable in combat" : b.category === Category.Active ? "not usable in combat" : "works by itself in combat"}
      </p>
      {problems.length > 0 && (
        <ul className="skill-problems">
          {problems.map((p) => (
            <li key={p} className="error">
              {p}
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}

export function SkillBuilder({ cls, skills, onChange, disabled }: SkillBuilderProps) {
  const check = checkSkills(skills, cls);
  const presets = PRESETS[cls];
  const full = skills.length >= STARTING_MAX;
  const replace = (i: number, s: NamedSkill) => onChange(skills.map((x, j) => (j === i ? s : x)));
  const anyCombat = skills.some((s) => usableInCombat(s.blocks));

  return (
    <div className="forms-field skill-builder">
      <div className="skill-builder-head">
        <span>
          Skills ({skills.length}/{STARTING_MAX}): at most 4 active, 1 reactive, 1 passive
        </span>
        <button type="button" className="btn" disabled={disabled} onClick={() => onChange(presets.map((p) => ({ ...p })))}>
          Use class presets
        </button>
        <button type="button" className="btn" disabled={disabled || full} onClick={() => onChange([...skills, blankSkill(cls, skills.length + 1)])}>
          Add custom skill
        </button>
      </div>
      <div className="forms-seg" role="group" aria-label="Add a preset">
        {presets.map((p) => (
          <button key={p.name} type="button" className="btn" disabled={disabled || full} onClick={() => onChange([...skills, { ...p }])} title="Add this preset">
            + {p.name}
          </button>
        ))}
      </div>
      {check.loadout && <p className="error">{check.loadout}</p>}
      {!anyCombat && <p className="muted">No active skill: in combat this character only has the weapon attack.</p>}
      <ol className="skill-list">
        {skills.map((s, i) => (
          <SkillCard
            key={i}
            skill={s}
            index={i}
            problems={check.perSkill[i] ?? []}
            disabled={disabled}
            onChange={(next) => replace(i, next)}
            onRemove={() => onChange(skills.filter((_, j) => j !== i))}
          />
        ))}
      </ol>
    </div>
  );
}
