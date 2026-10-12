import { useState } from "react";
import { CharacterForm } from "../components/forms/CharacterForm";
import type { CharacterDraft } from "../components/forms/types";
import { aspectName, className, raceName } from "../chain/names";
import { Aspect, Class, Race } from "../chain/types";
import { DATA_SOURCE } from "../config";
import { useRasterizer, useRenderImage } from "../raster/RasterizerProvider";
import { SpriteCanvas } from "../raster/SpriteCanvas";

const STAT_MIN = 5;
const STAT_POINTS = 6;
const ALL = Object.values(Aspect);

// TODO(L9.6): the allowed set per class comes from the registry tables once they are on chain.
function allowedFor(c: Class): Aspect[] {
  if (c === Class.Mage) return [Aspect.Fire, Aspect.Cold, Aspect.Lightning];
  if (c === Class.Support) return [Aspect.Blood, Aspect.Blunt, Aspect.Piercing];
  return ALL;
}

export function CharacterPage() {
  const [draft, setDraft] = useState<CharacterDraft>({
    name: "",
    race: Race.Human,
    class: Class.Warrior,
    predisposition: Aspect.Piercing,
    stats: [STAT_MIN, STAT_MIN, STAT_MIN, STAT_MIN],
  });
  const [created, setCreated] = useState<CharacterDraft | null>(null);
  const raster = useRasterizer();
  // TODO(L9.6): build Character.appearance from race/class presets once the art books are uploaded;
  // until then the preview is the reference sprite from the loaded book.
  const preview = useRenderImage(raster.status === "ready" ? raster.sample : null);

  const onChange = (next: CharacterDraft) => {
    const allowed = allowedFor(next.class);
    setDraft(allowed.includes(next.predisposition) ? next : { ...next, predisposition: allowed[0]! });
  };

  return (
    <section>
      <h1>Create a character</h1>
      <CharacterForm
        value={draft}
        onChange={onChange}
        onSubmit={(d) => setCreated(d)}
        statPoints={STAT_POINTS}
        statMin={STAT_MIN}
        allowedPredispositions={allowedFor(draft.class)}
        preview={<SpriteCanvas image={preview.image} scale={6} label="character preview" />}
      />
      {created && (
        <p className="ok">
          {DATA_SOURCE === "mock" ? "Mock: " : ""}
          {created.name}, {raceName(created.race)} {className(created.class)} with {aspectName(created.predisposition)}{" "}
          predisposition. {DATA_SOURCE === "mock" ? "create_character is sent in L9.6." : ""}
        </p>
      )}
    </section>
  );
}
