// Starting skill builder: up to 6 skills checked live by the TS mirror of crates/skillcheck,
// the same rules create_character enforces. Class presets are one-click options.
import { ASPECT_NAMES } from "../../chain/names";
import {
  CATEGORY_NAMES,
  Category,
  EFFECT_NAMES,
  FORM_NAMES,
  PRESETS,
  PURPOSE_NAMES,
  Purpose,
  SKILL_ERROR_MESSAGES,
  SKILL_NAME_LEN,
  SOURCE_NAMES,
  STARTING_MAX,
  Source,
  TARGET_NAMES,
  TRIGGER_NAMES,
  AURA_RADIUS_MAX,
  Effect,
  Form,
  checkLoadout,
  derive,
  formRange,
  makeBlocks,
  skillProblems,
  usableInCombat,
  type NamedSkill,
  type SkillBlocks,
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
  if (cls === Class.Support)
    return { name: `Skill ${n}`, blocks: makeBlocks(Category.Active, Purpose.Buff, Source.Magical, Form.Projectile, [Aspect.Lightning], { effect: Effect.Haste }) };
  const source = cls === Class.Warrior ? Source.Combat : Source.Magical;
  return { name: `Skill ${n}`, blocks: makeBlocks(Category.Active, Purpose.Damage, source, Form.Strike, [cls === Class.Warrior ? Aspect.Piercing : Aspect.Cold]) };
}

/** Keeps one canonical encoding: unused aspect slots go back to the default (Fire). */
function withAspectCount(b: SkillBlocks, count: number): SkillBlocks {
  const aspects = b.aspects.map((a, i) => (i < count ? a : Aspect.Fire)) as SkillBlocks["aspects"];
  return { ...b, aspectCount: count, aspects };
}

function preview(b: SkillBlocks): string {
  const d = derive(b);
  const shares = b.aspects
    .slice(0, Math.max(1, Math.min(b.aspectCount, 3)))
    .map((a, i) => `${ASPECT_NAMES[a]} ${formatBp(d.sharesBp[i] ?? 0)}`)
    .join(" / ");
  const parts = [`power ${formatBp(d.powerBp)}`, shares, `cooldown ${d.cooldown}`, `range ${formRange(b.form)}`];
  if (d.fusion) parts.push(`fusion #${d.fusion}`);
  return parts.join(" · ");
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
  const set = (patch: Partial<SkillBlocks>) => onChange({ ...skill, blocks: { ...b, ...patch } });
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
        <Pick label={`Skill ${n} effect`} value={b.effect} names={EFFECT_NAMES} disabled={disabled} onChange={(effect) => set({ effect })} />
        <Pick label={`Skill ${n} trigger`} value={b.trigger} names={TRIGGER_NAMES} disabled={disabled} onChange={(trigger) => set({ trigger })} />
        <Pick label={`Skill ${n} aura`} value={Math.min(b.auraRadius, AURA_RADIUS_MAX)} names={AURA_NAMES} disabled={disabled} onChange={(auraRadius) => set({ auraRadius })} />
      </div>
      <p className="muted skill-preview">
        {preview(b)} · {usableInCombat(b) ? "usable in combat" : "not usable in combat (only active skills are)"}
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
