// Mock battle on the 16x16 demo map. Entrance tile is (1, 1).
import { useState } from "react";
import { DEMO_MAP } from "../../chain/mock";
import { bestiaryName } from "../../chain/names";
import { Tile, cellIndex, type Cell } from "../../chain/types";
import { Grid } from "./Grid";
import type { GridUnit } from "./types";
import "./grid.css";

const MAP_W = 16;
const MAP_H = 16;
const MOVE = 3;

const WALKABLE: ReadonlySet<number> = new Set([Tile.Floor, Tile.Door, Tile.Entrance, Tile.Exit, Tile.Trap]);

function initialUnits(): GridUnit[] {
  const players: GridUnit[] = [
    { id: 0, kind: "player", x: 1, y: 2, hp: 20, hpMax: 20, alive: true, label: "Bran" },
    { id: 1, kind: "player", x: 2, y: 1, hp: 20, hpMax: 20, alive: true, label: "Aria" },
  ];
  const monsters = DEMO_MAP.spawns.map((spawn, i): GridUnit => ({
    id: 4 + i,
    kind: "monster",
    x: spawn.x,
    y: spawn.y,
    hp: 12,
    hpMax: 12,
    alive: true,
    label: bestiaryName(spawn.bestiaryId),
  }));
  return players.concat(monsters);
}

function reachableCells(units: readonly GridUnit[], active: GridUnit): Cell[] {
  if (!active.alive) return [];
  const cells: Cell[] = [];
  for (let y = 0; y < MAP_H; y++) {
    for (let x = 0; x < MAP_W; x++) {
      const dist = Math.abs(x - active.x) + Math.abs(y - active.y);
      if (dist === 0 || dist > MOVE) continue;
      const tile = DEMO_MAP.tiles[cellIndex(x, y)] ?? Tile.Wall;
      if (!WALKABLE.has(tile)) continue;
      if (units.some((unit) => unit.x === x && unit.y === y)) continue;
      cells.push({ x, y });
    }
  }
  return cells;
}

/** Living monsters in the 4-neighborhood of the active player. */
function targetCells(units: readonly GridUnit[], active: GridUnit): Cell[] {
  if (!active.alive || active.kind !== "player") return [];
  const cells: Cell[] = [];
  for (const unit of units) {
    if (unit.kind !== "monster" || !unit.alive) continue;
    const dist = Math.abs(unit.x - active.x) + Math.abs(unit.y - active.y);
    if (dist !== 1) continue;
    if (cells.some((cell) => cell.x === unit.x && cell.y === unit.y)) continue;
    cells.push({ x: unit.x, y: unit.y });
  }
  return cells;
}

export default function Demo() {
  const [units, setUnits] = useState<GridUnit[]>(initialUnits);
  const [activeUnitId, setActiveUnitId] = useState(0);
  const [selectedUnitId, setSelectedUnitId] = useState<number | null>(null);
  const [hover, setHover] = useState<Cell | null>(null);

  const active = units.find((unit) => unit.id === activeUnitId);
  const selected = units.find((unit) => unit.id === selectedUnitId);
  const reachable = active ? reachableCells(units, active) : [];
  const targets = active ? targetCells(units, active) : [];

  function onCellClick(cell: Cell) {
    setUnits((prev) => {
      const mover = prev.find((unit) => unit.id === activeUnitId);
      if (!mover?.alive) return prev;
      if (!reachableCells(prev, mover).some((spot) => spot.x === cell.x && spot.y === cell.y)) return prev;
      return prev.map((unit) => (unit.id === mover.id ? { ...unit, x: cell.x, y: cell.y } : unit));
    });
  }

  function onNext() {
    const living = units.filter((unit) => unit.alive).sort((a, b) => a.id - b.id);
    if (living.length === 0) return;
    const next = living.find((unit) => unit.id > activeUnitId) ?? living[0];
    if (next) setActiveUnitId(next.id);
  }

  function onKill() {
    if (selectedUnitId === null) return;
    setUnits((prev) => prev.map((unit) => (unit.id === selectedUnitId ? { ...unit, alive: false, hp: 0 } : unit)));
  }

  return (
    <div className="grid-demo">
      <div className="grid-map">
        <Grid
          width={MAP_W}
          height={MAP_H}
          tiles={DEMO_MAP.tiles}
          units={units}
          activeUnitId={activeUnitId}
          selectedUnitId={selectedUnitId}
          reachable={reachable}
          targets={targets}
          onCellClick={onCellClick}
          onUnitClick={(unit) => setSelectedUnitId(unit.id)}
          onCellHover={setHover}
        />
      </div>
      <aside className="panel grid-side">
        <h2 className="grid-title">Battle</h2>
        <p className="grid-stat">
          Turn <span className="mono">{active ? active.label : "none"}</span>
        </p>
        <p className="grid-stat">
          Hover <span className="mono">{hover ? `${hover.x}, ${hover.y}` : "none"}</span>
        </p>
        <div>
          <p className="muted grid-stat">Selected</p>
          {selected ? (
            <>
              <p className="grid-stat">{selected.label}</p>
              <p className="mono grid-stat">
                {selected.alive ? `HP ${selected.hp}/${selected.hpMax}` : `Dead, HP 0/${selected.hpMax}`}
              </p>
            </>
          ) : (
            <p className="muted grid-stat">Select a unit</p>
          )}
        </div>
        <div className="grid-actions">
          <button type="button" className="btn primary" onClick={onNext}>
            Next turn
          </button>
          <button type="button" className="btn" onClick={onKill} disabled={selectedUnitId === null}>
            Kill selected
          </button>
        </div>
      </aside>
    </div>
  );
}
