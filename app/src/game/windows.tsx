// The side panel's windows: the bag and the skills of the active character, the characters (list,
// creation, one character) and the settings.
import { useState } from "react";
import type { Address } from "@solana/kit";
import { Link } from "react-router-dom";
import { getAccount, type SkillRecordAcc } from "../chain/game";
import { skillSetPda } from "../chain/pda";
import { PRESETS, type NamedSkill } from "../chain/skillcheck";
import { Class } from "../chain/types";
import { SkillBuilder, checkSkills } from "../components/forms/SkillBuilder";
import { RpcSettings } from "../components/RpcSettings";
import { DATA_SOURCE, DEV_TOOLS } from "../config";
import { CharacterCard, CharacterCreate } from "../routes/Character";
import { CharacterScreen, SkillsTable } from "../routes/CharacterDetail";
import { className } from "../chain/names";
import { bytesText } from "../chain/game";
import { useEffect } from "react";
import { useActiveCharacter } from "./ActiveCharacter";

const noCharacter = (
  <p className="muted">{DEV_TOOLS ? "No active character: connect a wallet (or ?dev=1 on localnet) and create one." : "No active character: connect a wallet and create one."}</p>
);

export function BagWindow() {
  const { active } = useActiveCharacter();
  if (!active) return noCharacter;
  return (
    <>
      <h3>{bytesText(active.data.name)}'s bag</h3>
      <CharacterScreen key={active.address} address={active.address} only="bag" />
    </>
  );
}

/** The active character's skills, and the builder to try a set for a new character. */
export function SkillsWindow({ onCreateWith }: { onCreateWith: (cls: Class, skills: NamedSkill[]) => void }) {
  const { active } = useActiveCharacter();
  const [tab, setTab] = useState<"list" | "builder">("list");
  const [skills, setSkills] = useState<SkillRecordAcc[] | null>(null);
  const cls = (active?.data.core.class ?? Class.Warrior) as Class;
  const [draft, setDraft] = useState<NamedSkill[]>(() => structuredClone(PRESETS[cls] ?? []));

  useEffect(() => {
    setSkills(null);
    if (!active || DATA_SOURCE !== "chain") return;
    let live = true;
    void skillSetPda(active.address)
      .then((a) => getAccount("SkillSet", a))
      .then(
        (set) => live && setSkills(set ? set.skills.slice(0, set.count) : []),
        () => live && setSkills([]),
      );
    return () => {
      live = false;
    };
  }, [active]);
  useEffect(() => setDraft(structuredClone(PRESETS[cls] ?? [])), [cls]);

  return (
    <div>
      <div className="row">
        <button type="button" className="btn" aria-pressed={tab === "list"} onClick={() => setTab("list")}>
          {active ? `${bytesText(active.data.name)}'s skills` : "Skills"}
        </button>
        <button type="button" className="btn" aria-pressed={tab === "builder"} onClick={() => setTab("builder")}>
          Skill builder
        </button>
      </div>
      {tab === "list" && (!active ? noCharacter : skills === null ? <p className="muted">Reading the skills…</p> : <SkillsTable skills={skills} />)}
      {tab === "builder" && (
        <>
          <p className="muted">
            A character's skills are chosen when it is created. Try a {className(cls)} set here, then create a character with it.
          </p>
          <SkillBuilder cls={cls} skills={draft} onChange={setDraft} />
          <button type="button" className="btn primary" disabled={!checkSkills(draft, cls).ok} onClick={() => onCreateWith(cls, draft)}>
            Create a {className(cls)} with these skills
          </button>
        </>
      )}
    </div>
  );
}

/** The wallet's characters and "Create new"; a click opens one and makes it active. */
export function CharactersWindow({
  creating,
  initial,
  onOpen,
  onCreate,
}: {
  creating: boolean;
  initial?: { class: Class; skills: NamedSkill[] };
  onOpen: (address: Address) => void;
  onCreate: (yes: boolean) => void;
}) {
  const { characters, active, setActive, refresh } = useActiveCharacter();
  if (creating) {
    return (
      <div>
        <p>
          <button type="button" className="btn" onClick={() => onCreate(false)}>
            ← My characters
          </button>
        </p>
        <CharacterCreate
          initial={initial}
          onCreated={(address) => {
            void refresh().then(() => {
              setActive(address);
              onOpen(address);
            });
          }}
        />
      </div>
    );
  }
  return (
    <div>
      <p>
        <button type="button" className="btn primary" onClick={() => onCreate(true)}>
          Create a new character
        </button>
      </p>
      {DATA_SOURCE === "chain" && characters.length === 0 && <p className="muted">No characters yet.</p>}
      <div className="char-list">
        {characters.map((c) => (
          <CharacterCard
            key={c.address}
            c={c.data}
            active={active?.address === c.address}
            onOpen={() => {
              setActive(c.address);
              onOpen(c.address);
            }}
          />
        ))}
      </div>
    </div>
  );
}

export function CharacterWindow({ address, onOpenParty }: { address: Address; onOpenParty: (session: Address) => void }) {
  return <CharacterScreen key={address} address={address} onOpenParty={onOpenParty} />;
}

export function SettingsWindow() {
  return (
    <div className="settings">
      <h3>RPC</h3>
      <p className="muted">This browser reads the chain through:</p>
      <RpcSettings />
      <h3>Guides</h3>
      <p>
        <Link to="/wiki">Wiki</Link>: aspects, states, resonances, how a hit is counted, turns and action points.
      </p>
      <p>
        <Link to="/bestiary">Bestiary</Link>: every monster drawn so far.
      </p>
      {DEV_TOOLS && (
        <>
          <h3>Development</h3>
          <p className="muted">
            Component demos: <Link to="/dev/grid">grid</Link> · <Link to="/dev/dice">dice</Link> · <Link to="/dev/forms">forms</Link>. On localnet, <code>?dev=1</code>{" "}
            gives this tab its own funded wallet.
          </p>
        </>
      )}
    </div>
  );
}
