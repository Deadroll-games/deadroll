import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Effect, PRESETS, SKILL_ERROR_MESSAGES, type NamedSkill } from "../../chain/skillcheck";
import { Aspect, Class, Race } from "../../chain/types";
import { CharacterForm } from "./CharacterForm";
import { blankSkill, checkSkills, SkillBuilder } from "./SkillBuilder";
import type { CharacterDraft } from "./types";

afterEach(cleanup);

const ALL = [Aspect.Fire, Aspect.Cold, Aspect.Lightning, Aspect.Blood, Aspect.Blunt, Aspect.Piercing];
const DRAFT: CharacterDraft = { name: "Bran", race: Race.Human, class: Class.Warrior, predisposition: Aspect.Piercing, stats: [11, 5, 5, 5] };

function Builder({ cls, initial }: { cls: Class; initial: NamedSkill[] }) {
  const [skills, setSkills] = useState(initial);
  return <SkillBuilder cls={cls} skills={skills} onChange={setSkills} />;
}

function Form({ onSubmit, initial = DRAFT }: { onSubmit: (d: CharacterDraft) => void; initial?: CharacterDraft }) {
  const [value, setValue] = useState(initial);
  return <CharacterForm value={value} onChange={setValue} onSubmit={onSubmit} statPoints={6} statMin={5} allowedPredispositions={ALL} />;
}

const select = (label: string, value: number) => fireEvent.change(screen.getByLabelText(label), { target: { value: String(value) } });

describe("checkSkills", () => {
  it("accepts every class preset set and a blank skill of the class", () => {
    for (const cls of [Class.Warrior, Class.Mage, Class.Support]) {
      expect(checkSkills(PRESETS[cls], cls).ok).toBe(true);
      expect(checkSkills([blankSkill(cls, 1)], cls).ok).toBe(true);
    }
  });

  it("rejects another class's presets with the class message", () => {
    const r = checkSkills(PRESETS[Class.Warrior], Class.Mage);
    expect(r.ok).toBe(false);
    expect(r.perSkill[0]).toEqual([SKILL_ERROR_MESSAGES.ClassSource]);
  });

  it("reports the loadout limit", () => {
    const actives = PRESETS[Class.Warrior].slice(0, 4);
    const r = checkSkills([...actives, actives[0]!], Class.Warrior);
    expect(r.loadout).toBe(SKILL_ERROR_MESSAGES.TooManyActive);
    expect(r.ok).toBe(false);
  });
});

describe("SkillBuilder", () => {
  it("shows a rule message live and clears it when fixed", () => {
    render(<Builder cls={Class.Warrior} initial={PRESETS[Class.Warrior]} />);
    // Shield Bash (Blunt, Stun) -> Freeze needs Cold
    select("Skill 2 effect", Effect.Freeze);
    expect(screen.getByText(SKILL_ERROR_MESSAGES.HardControlAspect)).toBeTruthy();
    select("Skill 2 aspect 1", Aspect.Cold);
    expect(screen.queryByText(SKILL_ERROR_MESSAGES.HardControlAspect)).toBeNull();
  });

  it("reducing the aspect count resets unused slots so the encoding stays canonical", () => {
    render(<Builder cls={Class.Warrior} initial={[PRESETS[Class.Warrior][3]!]} />);
    // Bleeding Cleave: Blood + Piercing, line
    expect(screen.getByLabelText("Skill 1 aspect 2")).toBeTruthy();
    select("Skill 1 aspect count", 0);
    expect(screen.queryByLabelText("Skill 1 aspect 2")).toBeNull();
    expect(screen.queryByText(SKILL_ERROR_MESSAGES.UnusedAspectSet)).toBeNull();
  });

  it("adds presets in one click and stops at 6", () => {
    render(<Builder cls={Class.Mage} initial={[]} />);
    expect(screen.getByText("No active skill: in combat this character only has the weapon attack.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "+ Frost Bolt" }));
    expect(screen.getByLabelText("Skill 1 name")).toHaveProperty("value", "Frost Bolt");
    fireEvent.click(screen.getByRole("button", { name: "Use class presets" }));
    expect(screen.getAllByRole("listitem", { name: /^Skill \d$/ })).toHaveLength(6);
    expect(screen.getByRole("button", { name: "+ Arc" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "Add custom skill" })).toHaveProperty("disabled", true);
    fireEvent.click(screen.getByRole("button", { name: "Remove skill 1" }));
    expect(screen.getAllByRole("listitem", { name: /^Skill \d$/ })).toHaveLength(5);
  });

  it("shows the derived preview", () => {
    render(<Builder cls={Class.Mage} initial={[PRESETS[Class.Mage][1]!]} />);
    // Frost Burn: Cold 65% / Fire 35%, fusion 1 + 1*6 + 0 = 7
    const card = screen.getByRole("listitem", { name: "Skill 1" });
    expect(within(card).getByText(/power 100% · Cold 65% \/ Fire 35% · cooldown 1 · range 6 · fusion #7 · usable in combat/)).toBeTruthy();
  });
});

describe("CharacterForm with skills", () => {
  it("blocks submit on an invalid skill and submits custom skills", () => {
    const onSubmit = vi.fn();
    render(<Form onSubmit={onSubmit} />);
    const submit = screen.getByRole("button", { name: "Create character" });
    expect(submit).toHaveProperty("disabled", false);

    fireEvent.change(screen.getByLabelText("Skill 1 name"), { target: { value: " bad" } });
    expect(submit).toHaveProperty("disabled", true);
    expect(screen.getByText("Fix the skills above to create the character.")).toBeTruthy();

    fireEvent.change(screen.getByLabelText("Skill 1 name"), { target: { value: "Spear Lunge" } });
    expect(submit).toHaveProperty("disabled", false);
    fireEvent.click(submit);
    const sent = onSubmit.mock.calls[0]![0] as CharacterDraft;
    expect(sent.skills?.[0]?.name).toBe("Spear Lunge");
    expect(sent.skills).toHaveLength(6);
  });

  it("untouched skills follow the class presets", () => {
    const onSubmit = vi.fn();
    render(<Form onSubmit={onSubmit} />);
    fireEvent.click(screen.getByRole("button", { name: "Mage" }));
    expect(screen.getByLabelText("Skill 1 name")).toHaveProperty("value", "Frost Bolt");
    fireEvent.click(screen.getByRole("button", { name: "Create character" }));
    expect((onSubmit.mock.calls[0]![0] as CharacterDraft).skills).toBeUndefined();
  });
});
