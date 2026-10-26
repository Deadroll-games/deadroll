import { useEffect, useRef, useState } from "react";
import { BESTIARY_NAMES, BIOME_NAMES } from "../../chain/names";
import { tileVariants } from "../../chain/appearance";
import { DEMO_MAP, MAP_PRESETS, MOCK_CHARACTERS, presetToLayout } from "../../chain/mock";
import { Aspect, Class, Race, Tile, type Aspect as AspectId } from "../../chain/types";
import { CharacterForm } from "./CharacterForm";
import { MapPalette } from "./MapPalette";
import { PartyLobby } from "./PartyLobby";
import type { BuilderTool, CharacterDraft, LobbyPlayer } from "./types";
import "./forms.css";

const PRICE = 100_000_000n;
const STAT_MIN = 5;
const STAT_POINTS = 6;
const DM = "Dm11111111111111111111111111111111111111dm";

const MAGE_ASPECTS: AspectId[] = [Aspect.Fire, Aspect.Cold, Aspect.Lightning];
const SUPPORT_ASPECTS: AspectId[] = [Aspect.Blood, Aspect.Blunt, Aspect.Piercing];
const ALL_ASPECTS: AspectId[] = [
  Aspect.Fire,
  Aspect.Cold,
  Aspect.Lightning,
  Aspect.Blood,
  Aspect.Blunt,
  Aspect.Piercing,
];

function allowedFor(kind: Class): AspectId[] {
  if (kind === Class.Mage) return MAGE_ASPECTS;
  if (kind === Class.Support) return SUPPORT_ASPECTS;
  return ALL_ASPECTS;
}

function PreviewCanvas({ label, color }: { label: string; color: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    ctx.imageSmoothingEnabled = false;
    ctx.fillStyle = "#120c16";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = color;
    ctx.fillRect(10, 6, 12, 18);
    ctx.fillStyle = "#e9e1d4";
    ctx.fillRect(13, 10, 2, 2);
    ctx.fillRect(17, 10, 2, 2);
  }, [color]);
  return <canvas ref={ref} width={32} height={32} className="pixel forms-preview" aria-label={label} />;
}

const BESTIARY = Object.entries(BESTIARY_NAMES).map(([id, name]) => ({ id: Number(id), name }));
const PRESETS = MAP_PRESETS.map((preset) => ({ name: preset.name, width: preset.width, height: preset.height }));

export default function Demo() {
  const [log, setLog] = useState("idle");
  const [draft, setDraft] = useState<CharacterDraft>({
    name: "Bran",
    race: Race.Human,
    class: Class.Warrior,
    predisposition: Aspect.Piercing,
    stats: [STAT_MIN, STAT_MIN, STAT_MIN, STAT_MIN],
  });
  const [tool, setTool] = useState<BuilderTool>({ kind: "tile", tile: Tile.StoneFloor, variant: 0 });
  const [biome, setBiome] = useState(0);
  const [spawns, setSpawns] = useState(DEMO_MAP.spawns);
  const [errors, setErrors] = useState<string[]>([]);
  const [canFinalize, setCanFinalize] = useState(false);
  const [checks, setChecks] = useState(0);
  const [role, setRole] = useState<"dm" | "player" | "guest">("dm");
  const [players, setPlayers] = useState<LobbyPlayer[]>(
    MOCK_CHARACTERS.map((character, index) => ({
      slot: index,
      player: character.address,
      characterName: character.name,
      race: character.race,
      class: character.class,
      ready: false,
      paid: PRICE,
    })),
  );

  const collected = players.reduce((sum, player) => sum + player.paid, 0n);
  const mapName = MAP_PRESETS[0]?.name ?? "demo_16x16";

  function changeDraft(next: CharacterDraft) {
    if (next.class !== draft.class) {
      const first = allowedFor(next.class)[0] ?? Aspect.Fire;
      setDraft({ ...next, predisposition: first });
    } else {
      setDraft(next);
    }
    setLog("onChange");
  }

  return (
    <div className="forms-demo">
      <p className="forms-log mono muted">Last callback: {log}</p>

      <section>
        <h2>Create a character</h2>
        <CharacterForm
          value={draft}
          onChange={changeDraft}
          onSubmit={(value) => setLog(`onSubmit ${value.name}`)}
          statPoints={STAT_POINTS}
          statMin={STAT_MIN}
          allowedPredispositions={allowedFor(draft.class)}
          preview={<PreviewCanvas label="Character preview" color="#4f8fd9" />}
        />
      </section>

      <section>
        <h2>Map builder</h2>
        <MapPalette
          tool={tool}
          onToolChange={(next) => {
            setTool(next);
            if (next.kind === "tile") setLog(`onToolChange tile ${next.tile} variant ${next.variant}`);
            else if (next.kind === "spawn") setLog(`onToolChange spawn ${next.bestiaryId}`);
            else setLog("onToolChange erase");
          }}
          biome={biome}
          biomes={BIOME_NAMES}
          onBiomeChange={setBiome}
          tileVariants={(tile) => tileVariants(biome, tile)}
          bestiary={BESTIARY}
          presets={PRESETS}
          onLoadPreset={(name) => {
            const preset = MAP_PRESETS.find((item) => item.name === name);
            if (preset) setSpawns(presetToLayout(preset).spawns);
            setErrors([]);
            setCanFinalize(false);
            setChecks(0);
            setLog(`onLoadPreset ${name}`);
          }}
          spawns={spawns}
          onRemoveSpawn={(index) => {
            setSpawns((current) => current.filter((_, i) => i !== index));
            setErrors([]);
            setCanFinalize(false);
            setChecks(0);
            setLog(`onRemoveSpawn ${index}`);
          }}
          errors={errors}
          onValidate={() => {
            const next = checks + 1;
            setChecks(next);
            if (next === 1) {
              setErrors(["Entrance is missing", "A spawn sits on a wall"]);
              setCanFinalize(false);
            } else {
              setErrors([]);
              setCanFinalize(true);
            }
            setLog("onValidate");
          }}
          canFinalize={canFinalize}
          onFinalize={() => setLog("onFinalize")}
        />
      </section>

      <section>
        <h2>Party lobby</h2>
        <div className="forms-row" role="group" aria-label="Role">
          {(["dm", "player", "guest"] as const).map((item) => (
            <button key={item} type="button" className="btn" aria-pressed={role === item} onClick={() => setRole(item)}>
              {item}
            </button>
          ))}
        </div>
        <PartyLobby
          mapName={mapName}
          dm={DM}
          priceLamports={PRICE}
          collected={collected}
          players={players}
          maxPlayers={4}
          role={role}
          mySlot={0}
          myCharacters={MOCK_CHARACTERS.map((character) => ({ address: character.address, name: character.name }))}
          onJoin={(address) => setLog(`onJoin ${address}`)}
          onReady={(ready) => {
            setPlayers((current) => current.map((player) => (player.slot === 0 ? { ...player, ready } : player)));
            setLog(`onReady ${ready}`);
          }}
          onStart={() => setLog("onStart")}
          mapPreview={<PreviewCanvas label="Map preview" color="#2f6b3a" />}
        />
      </section>
    </div>
  );
}
