// The skills a player takes onto the map before Ready (docs/SKILL_BUILDER.md 3.4): every category
// full, 4 active + 1 reactive + 1 passive, or all of a kind when there are fewer. The selection is
// SkillSet indexes by slot (skillcheck.ts checkMapSelection, the same rule the program checks).
import { Category, NO_SKILL, SELECTION_SLOTS, SKILL_ERROR_MESSAGES, STARTING_MAX, checkMapSelection } from "../../chain/skillcheck";
import type { LobbySkill } from "./types";

const CATEGORY_LABEL: Record<Category, string> = {
  [Category.Active]: "Active",
  [Category.Reactive]: "Reactive",
  [Category.Passive]: "Passive",
};

/** The checked skills as a selection: each category's indexes, in SkillSet order, in its slots (the
 * picker never checks more than a category's slots). */
export function selectionOf(skills: readonly LobbySkill[], checked: ReadonlySet<number>): number[] {
  const out: number[] = Array.from({ length: STARTING_MAX }, () => NO_SKILL);
  for (const [category, start, slots] of SELECTION_SLOTS) {
    const mine = skills.filter((s) => s.category === category && checked.has(s.index)).map((s) => s.index);
    mine.sort((a, b) => a - b);
    mine.slice(0, slots).forEach((index, k) => (out[start + k] = index));
  }
  return out;
}

/** The rule's message for a selection, or null when it may go on chain. */
export function selectionError(skills: readonly LobbySkill[], selection: readonly number[]): string | null {
  const owned: Category[] = [];
  for (const s of skills) owned[s.index] = s.category;
  const e = checkMapSelection(owned, selection);
  return e ? SKILL_ERROR_MESSAGES[e] : null;
}

export function MapSkillPicker({
  skills,
  checked,
  disabled,
  onToggle,
}: {
  skills: readonly LobbySkill[];
  checked: ReadonlySet<number>;
  disabled: boolean;
  onToggle: (index: number) => void;
}) {
  return (
    <fieldset className="forms-field forms-skills" aria-label="skills for the map">
      <legend>Skills for this map</legend>
      {SELECTION_SLOTS.map(([category, , slots]) => {
        const mine = skills.filter((s) => s.category === category);
        const taken = mine.filter((s) => checked.has(s.index)).length;
        return (
          <div key={category}>
            <strong>
              {CATEGORY_LABEL[category]} {taken}/{Math.min(slots, mine.length)}
            </strong>
            {mine.length === 0 ? <span className="muted"> none</span> : null}
            {mine.map((s) => (
              <label key={s.index} className="forms-check">
                <input
                  type="checkbox"
                  checked={checked.has(s.index)}
                  disabled={disabled || (!checked.has(s.index) && taken >= slots)}
                  onChange={() => onToggle(s.index)}
                />
                {s.name}
              </label>
            ))}
          </div>
        );
      })}
    </fieldset>
  );
}
