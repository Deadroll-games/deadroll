import { useState } from "react";
import { bestiaryName, TILE_NAMES } from "../../chain/names";
import { Tile, type Tile as TileId } from "../../chain/types";
import { SpriteCanvas } from "../../raster/SpriteCanvas";
import type { BuilderTool, MapPaletteProps } from "./types";
import "./forms.css";

/** Patch C: `SecretKind` (crates/types) and the starting numbers of each secret tool. */
const SECRET_TRAP = 1;
const SECRET_AMBUSH = 2;
const SECRET_DOOR = 4;
const SECRET_TOOLS = [
  { secret: SECRET_TRAP, label: "Trap", a: 4, b: 10 },
  { secret: SECRET_AMBUSH, label: "Ambush", a: 0, b: 0 },
  { secret: 3, label: "Hidden stash", a: 0, b: 0 },
  { secret: SECRET_DOOR, label: "Secret door", a: 0, b: 0 },
];
type SecretTool = Extract<BuilderTool, { kind: "secret" }>;

const TILE_COLORS: Record<TileId, string> = {
  [Tile.Void]: "#07050a",
  [Tile.StoneFloor]: "#3b3043",
  [Tile.WoodenFloor]: "#4a3628",
  [Tile.Dirt]: "#3f3a2a",
  [Tile.Wall]: "#17111c",
  [Tile.Door]: "#7a5a2b",
  [Tile.ShallowWater]: "#2b4f7a",
  [Tile.DeepWater]: "#1b3352",
  [Tile.Stairs]: "#5a4f62",
  [Tile.Rubble]: "#4b4350",
  [Tile.Chasm]: "#07050a",
};

const SPAWN_CAP = 24;

function toolPressed(current: BuilderTool, next: BuilderTool): boolean {
  if (current.kind !== next.kind) return false;
  if (current.kind === "tile" && next.kind === "tile") return current.tile === next.tile && current.variant === next.variant;
  if (current.kind === "spawn" && next.kind === "spawn") return current.bestiaryId === next.bestiaryId;
  return current.kind === "erase" || current.kind === "chest" || current.kind === "entrance" || current.kind === "exit";
}

export function MapPalette(p: MapPaletteProps) {
  const [preset, setPreset] = useState(p.presets[0]?.name ?? "");
  const locked = Boolean(p.busy);
  const chosen = p.presets.some((item) => item.name === preset) ? preset : (p.presets[0]?.name ?? "");
  const brush = p.tool.kind === "tile" ? p.tool : null;
  const variants = brush ? (p.tileVariants?.(brush.tile) ?? 1) : 1;

  return (
    <div className="forms-root panel">
      <label className="forms-field" htmlFor="forms-biome">
        Biome
        <select id="forms-biome" value={p.biome} disabled={locked} onChange={(event) => p.onBiomeChange(Number(event.target.value))}>
          {p.biomes.map((name, id) => (
            <option key={name} value={id}>
              {name}
            </option>
          ))}
        </select>
      </label>

      <div className="forms-field">
        <span>Tiles</span>
        <div className="forms-tools" role="group" aria-label="Tile tools">
          {TILE_NAMES.map((label, index) => {
            const tile = index as TileId;
            return (
              <button
                key={label}
                type="button"
                className="btn"
                aria-pressed={brush?.tile === tile}
                disabled={locked}
                onClick={() => p.onToolChange({ kind: "tile", tile, variant: 0 })}
              >
                <span className="forms-swatch" style={{ background: TILE_COLORS[tile] }} />
                {label}
              </button>
            );
          })}
        </div>
        {brush && variants > 1 && (
          <div className="forms-tools" role="group" aria-label="Tile variants">
            {Array.from({ length: variants }, (_, variant) => {
              const next = { kind: "tile" as const, tile: brush.tile, variant };
              const image = p.tilePreview?.(brush.tile, variant) ?? null;
              return (
                <button
                  key={variant}
                  type="button"
                  className="btn forms-variant"
                  aria-label={`variant ${variant + 1}`}
                  aria-pressed={toolPressed(p.tool, next)}
                  disabled={locked}
                  onClick={() => p.onToolChange(next)}
                >
                  {image ? <SpriteCanvas image={image} scale={1} label={`variant ${variant + 1}`} /> : variant + 1}
                </button>
              );
            })}
          </div>
        )}
      </div>

      <div className="forms-field">
        <span>Spawns</span>
        <div className="forms-tools" role="group" aria-label="Spawn tools">
          {p.bestiary.map((entry) => (
            <button
              key={entry.id}
              type="button"
              className="btn"
              aria-pressed={toolPressed(p.tool, { kind: "spawn", bestiaryId: entry.id })}
              disabled={locked}
              onClick={() => p.onToolChange({ kind: "spawn", bestiaryId: entry.id })}
            >
              {entry.name}
            </button>
          ))}
          <button
            type="button"
            className="btn"
            aria-pressed={p.tool.kind === "entrance"}
            disabled={locked}
            onClick={() => p.onToolChange({ kind: "entrance" })}
          >
            Entrance
          </button>
          <button
            type="button"
            className="btn"
            aria-pressed={p.tool.kind === "exit"}
            disabled={locked}
            onClick={() => p.onToolChange({ kind: "exit" })}
          >
            Exit
          </button>
          <button
            type="button"
            className="btn"
            aria-pressed={p.tool.kind === "chest"}
            disabled={locked}
            onClick={() => p.onToolChange({ kind: "chest" })}
          >
            Chest
          </button>
          <button
            type="button"
            className="btn"
            aria-pressed={p.tool.kind === "erase"}
            disabled={locked}
            onClick={() => p.onToolChange({ kind: "erase" })}
          >
            Erase
          </button>
        </div>
      </div>

      <div className="panel">
        <h3>Secrets</h3>
        <p className="muted">Only you see them: the map stores them encrypted with your wallet's key. Players find them by stepping on a cell or examining it.</p>
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          {SECRET_TOOLS.map((t) => (
            <button
              key={t.secret}
              type="button"
              className="btn"
              aria-pressed={p.tool.kind === "secret" && p.tool.secret === t.secret}
              disabled={locked}
              onClick={() => p.onToolChange({ kind: "secret", secret: t.secret, a: t.a, b: t.b })}
            >
              {t.label}
            </button>
          ))}
        </div>
        {p.tool.kind === "secret" && p.tool.secret !== SECRET_DOOR && (
          <div style={{ marginTop: 6 }}>
            <label>
              {p.tool.secret === SECRET_TRAP ? "Damage " : p.tool.secret === SECRET_AMBUSH ? "Pack " : "Reward table "}
              <input type="number" min={0} max={255} value={p.tool.a} disabled={locked} onChange={(e) => p.onToolChange({ ...(p.tool as SecretTool), a: Number(e.target.value) })} />
            </label>
            {p.tool.secret === SECRET_TRAP && (
              <label>
                {" "}Detection difficulty{" "}
                <input type="number" min={0} max={255} value={p.tool.b} disabled={locked} onChange={(e) => p.onToolChange({ ...(p.tool as SecretTool), b: Number(e.target.value) })} />
              </label>
            )}
          </div>
        )}
      </div>

      <div className="forms-row">
        <label className="forms-field" htmlFor="forms-preset">
          Preset
          <select
            id="forms-preset"
            value={chosen}
            disabled={locked || p.presets.length === 0}
            onChange={(event) => setPreset(event.target.value)}
          >
            {p.presets.map((item) => (
              <option key={item.name} value={item.name}>
                {item.name} ({item.width}x{item.height})
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          className="btn"
          disabled={locked || chosen.length === 0}
          onClick={() => p.onLoadPreset(chosen)}
        >
          Load
        </button>
      </div>

      {p.loot}

      <p className="muted">
        {p.spawns.length}/{SPAWN_CAP} spawns
      </p>
      <ul className="forms-spawns">
        {p.spawns.map((spawn, index) => (
          <li key={`${spawn.bestiaryId}-${spawn.x}-${spawn.y}-${index}`}>
            <span>
              {bestiaryName(spawn.bestiaryId)} lvl {spawn.level} at ({spawn.x}, {spawn.y}) pack {spawn.pack}
            </span>
            <button
              type="button"
              className="btn"
              aria-label={`remove spawn ${index}`}
              disabled={locked}
              onClick={() => p.onRemoveSpawn(index)}
            >
              Remove
            </button>
            {p.renderSpawn?.(spawn, index)}
          </li>
        ))}
      </ul>

      <div className="forms-row">
        <button type="button" className="btn" disabled={locked} onClick={() => p.onValidate()}>
          Validate
        </button>
        <button
          type="button"
          className="btn primary"
          disabled={!p.canFinalize || locked}
          onClick={() => p.onFinalize()}
        >
          Finalize
        </button>
      </div>

      {p.errors.length > 0 ? (
        <ul className="forms-errors error">
          {p.errors.map((message, index) => (
            <li key={`${index}-${message}`}>{message}</li>
          ))}
        </ul>
      ) : null}
      {p.canFinalize ? <p className="ok">Map is valid</p> : null}
    </div>
  );
}
