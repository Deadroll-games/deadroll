import { useState } from "react";
import { bestiaryName, TILE_NAMES } from "../../chain/names";
import { Tile, type Tile as TileId } from "../../chain/types";
import type { BuilderTool, MapPaletteProps } from "./types";
import "./forms.css";

const TILE_COLORS: Record<TileId, string> = {
  [Tile.Floor]: "#3b3043",
  [Tile.Wall]: "#17111c",
  [Tile.Water]: "#2b4f7a",
  [Tile.Door]: "#7a5a2b",
  [Tile.Entrance]: "#2f6b3a",
  [Tile.Exit]: "#8a6a1f",
  [Tile.Trap]: "#6b2f2f",
};

const SPAWN_CAP = 24;

function toolPressed(current: BuilderTool, next: BuilderTool): boolean {
  if (current.kind !== next.kind) return false;
  if (current.kind === "tile" && next.kind === "tile") return current.tile === next.tile;
  if (current.kind === "spawn" && next.kind === "spawn") return current.bestiaryId === next.bestiaryId;
  return current.kind === "erase";
}

export function MapPalette(p: MapPaletteProps) {
  const [preset, setPreset] = useState(p.presets[0]?.name ?? "");
  const locked = Boolean(p.busy);
  const chosen = p.presets.some((item) => item.name === preset) ? preset : (p.presets[0]?.name ?? "");

  return (
    <div className="forms-root panel">
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
                aria-pressed={toolPressed(p.tool, { kind: "tile", tile })}
                disabled={locked}
                onClick={() => p.onToolChange({ kind: "tile", tile })}
              >
                <span className="forms-swatch" style={{ background: TILE_COLORS[tile] }} />
                {label}
              </button>
            );
          })}
        </div>
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
            aria-pressed={p.tool.kind === "erase"}
            disabled={locked}
            onClick={() => p.onToolChange({ kind: "erase" })}
          >
            Erase
          </button>
        </div>
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
