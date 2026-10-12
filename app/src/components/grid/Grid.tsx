// Battle map canvas: tiles, units, move highlights, and cell clicks.
import { useEffect, useRef, type MouseEvent } from "react";
import { Tile, cellIndex, type Cell } from "../../chain/types";
import type { GridProps, GridUnit } from "./types";
import "./grid.css";

const FLOOR = "#3b3043";
const WALL = "#17111c";
const WALL_EDGE = "#3d3450";
const WATER = "#2b4f7a";
const DOOR = "#7a5a2b";
const ENTRANCE = "#2f6b3a";
const EXIT = "#8a6a1f";
const TRAP = "#6b2f2f";
const GRID_LINE = "rgba(0,0,0,.25)";
const REACHABLE = "rgba(79,143,217,.28)";
const TARGET = "#d65a4a";
const PLAYER = "#4f8fd9";
const MONSTER = "#c4473a";
const DEAD = "#7d7786";
const ACTIVE_RING = "#d9a441";
const SELECT_RING = "#ffffff";
const PATH_DOT = "#d9a441";
const INK = "#120c16";
const LETTER = "#f4efe6";

type Rect = { left: number; top: number; width: number; height: number };

/** clientX/clientY to a cell. Uses the CSS box, so a scaled canvas still maps. Null outside. */
export function cellFromPoint(px: number, py: number, rect: Rect, width: number, height: number): Cell | null {
  if (rect.width <= 0 || rect.height <= 0 || width <= 0 || height <= 0) return null;
  const relX = px - rect.left;
  const relY = py - rect.top;
  if (relX < 0 || relY < 0 || relX >= rect.width || relY >= rect.height) return null;
  let x = Math.floor((relX / rect.width) * width);
  let y = Math.floor((relY / rect.height) * height);
  if (x >= width) x = width - 1;
  if (y >= height) y = height - 1;
  if (x < 0 || y < 0) return null;
  return { x, y };
}

function context2d(canvas: HTMLCanvasElement): CanvasRenderingContext2D | null {
  try {
    return canvas.getContext("2d");
  } catch {
    return null;
  }
}

function knownTile(value: number | undefined): Tile | null {
  switch (value) {
    case Tile.Floor:
    case Tile.Wall:
    case Tile.Water:
    case Tile.Door:
    case Tile.Entrance:
    case Tile.Exit:
    case Tile.Trap:
      return value;
    default:
      return null;
  }
}

function inMap(cell: Cell, width: number, height: number): boolean {
  return cell.x >= 0 && cell.y >= 0 && cell.x < width && cell.y < height;
}

function paintOutline(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  size: number,
  inset: number,
  thickness: number,
  color: string,
) {
  const left = x + inset;
  const top = y + inset;
  const inner = size - inset * 2;
  if (inner <= 0 || thickness <= 0) return;
  ctx.fillStyle = color;
  ctx.fillRect(left, top, inner, thickness);
  ctx.fillRect(left, top + inner - thickness, inner, thickness);
  ctx.fillRect(left, top, thickness, inner);
  ctx.fillRect(left + inner - thickness, top, thickness, inner);
}

function paintImage(
  ctx: CanvasRenderingContext2D,
  scratch: HTMLCanvasElement,
  image: ImageData,
  dx: number,
  dy: number,
  dw: number,
  dh: number,
) {
  if (image.width <= 0 || image.height <= 0 || dw <= 0 || dh <= 0) return;
  if (scratch.width !== image.width || scratch.height !== image.height) {
    scratch.width = image.width;
    scratch.height = image.height;
  }
  const src = context2d(scratch);
  if (!src) return;
  src.putImageData(image, 0, 0);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(scratch, dx, dy, dw, dh);
}

/** Largest integer scale of `image` that fits in maxW x maxH, at least 1. */
function integerDest(image: ImageData, maxW: number, maxH: number): { w: number; h: number } {
  const sx = Math.floor(maxW / image.width);
  const sy = Math.floor(maxH / image.height);
  const scale = Math.max(1, Math.min(sx, sy));
  return { w: image.width * scale, h: image.height * scale };
}

function hpColor(ratio: number): string {
  const t = Math.max(0, Math.min(1, ratio));
  const r = Math.round(214 + (108 - 214) * t);
  const g = Math.round(90 + (191 - 90) * t);
  const b = Math.round(74 + (106 - 74) * t);
  return `rgb(${r},${g},${b})`;
}

function paintOrder(units: readonly GridUnit[]): GridUnit[] {
  const monsters: GridUnit[] = [];
  const players: GridUnit[] = [];
  for (const unit of units) {
    if (unit.kind === "player") players.push(unit);
    else monsters.push(unit);
  }
  return monsters.concat(players);
}

/** Live unit on a cell. Corpses and tombstones do not occupy a cell, so a click on one is a cell click (a move). */
function unitOnCell(units: readonly GridUnit[], cell: Cell): GridUnit | undefined {
  const ordered = paintOrder(units);
  for (let i = ordered.length - 1; i >= 0; i--) {
    const unit = ordered[i];
    if (unit && unit.alive && unit.x === cell.x && unit.y === cell.y) return unit;
  }
  return undefined;
}

function drawTile(
  ctx: CanvasRenderingContext2D,
  scratch: HTMLCanvasElement,
  tile: Tile | null,
  sprite: ImageData | null,
  x: number,
  y: number,
  cell: number,
) {
  if (sprite) {
    paintImage(ctx, scratch, sprite, x, y, cell, cell);
    return;
  }
  if (tile === Tile.Wall) {
    ctx.fillStyle = WALL;
    ctx.fillRect(x, y, cell, cell);
    ctx.fillStyle = WALL_EDGE;
    ctx.fillRect(x, y, cell, Math.max(2, Math.floor(cell / 10)));
    return;
  }
  if (tile === Tile.Water) ctx.fillStyle = WATER;
  else if (tile === Tile.Door) ctx.fillStyle = DOOR;
  else if (tile === Tile.Entrance) ctx.fillStyle = ENTRANCE;
  else if (tile === Tile.Exit) ctx.fillStyle = EXIT;
  else ctx.fillStyle = FLOOR;
  ctx.fillRect(x, y, cell, cell);
  if (tile === Tile.Trap) {
    const pad = Math.floor(cell * 0.3);
    ctx.strokeStyle = TRAP;
    ctx.lineWidth = Math.max(2, Math.floor(cell / 14));
    ctx.beginPath();
    ctx.moveTo(x + pad, y + pad);
    ctx.lineTo(x + cell - pad, y + cell - pad);
    ctx.moveTo(x + cell - pad, y + pad);
    ctx.lineTo(x + pad, y + cell - pad);
    ctx.stroke();
  }
}

function drawMarker(ctx: CanvasRenderingContext2D, unit: GridUnit, cell: number) {
  const cx = unit.x * cell + cell / 2;
  const cy = unit.y * cell + cell / 2;
  const radius = Math.max(6, Math.floor(cell * 0.26));
  ctx.beginPath();
  ctx.arc(cx, cy, radius, 0, Math.PI * 2);
  if (!unit.alive) ctx.fillStyle = DEAD;
  else ctx.fillStyle = unit.kind === "player" ? PLAYER : MONSTER;
  ctx.fill();
  if (!unit.alive) {
    const d = Math.max(3, Math.floor(radius * 0.55));
    ctx.strokeStyle = LETTER;
    ctx.lineWidth = Math.max(2, Math.floor(cell / 16));
    ctx.beginPath();
    ctx.moveTo(cx - d, cy - d);
    ctx.lineTo(cx + d, cy + d);
    ctx.moveTo(cx + d, cy - d);
    ctx.lineTo(cx - d, cy + d);
    ctx.stroke();
    return;
  }
  const letter = unit.label.charAt(0) || "?";
  ctx.fillStyle = LETTER;
  ctx.font = `${Math.max(10, Math.floor(cell * 0.38))}px "Inter", system-ui, sans-serif`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(letter, cx, cy + 1);
}

function drawUnit(
  ctx: CanvasRenderingContext2D,
  scratch: HTMLCanvasElement,
  unit: GridUnit,
  cell: number,
  sprite: ImageData | null,
  active: boolean,
  selected: boolean,
) {
  if (sprite) {
    const cells = unit.size === 2 ? 2 : 1;
    const dest = integerDest(sprite, cells * cell, cells * cell);
    const dx = Math.round(unit.x * cell + cell / 2 - dest.w / 2);
    const dy = Math.round((unit.y + 1) * cell - dest.h);
    paintImage(ctx, scratch, sprite, dx, dy, dest.w, dest.h);
  } else {
    drawMarker(ctx, unit, cell);
  }

  if (unit.alive && unit.hpMax > 0) {
    const ratio = Math.max(0, Math.min(1, unit.hp / unit.hpMax));
    const barH = Math.max(3, Math.floor(cell / 12));
    const barW = cell - 8;
    const bx = unit.x * cell + 4;
    const by = (unit.y + 1) * cell - 8 - barH;
    ctx.fillStyle = INK;
    ctx.fillRect(bx, by, barW, barH);
    ctx.fillStyle = hpColor(ratio);
    ctx.fillRect(bx, by, Math.round(barW * ratio), barH);
  }

  const ox = unit.x * cell;
  const oy = unit.y * cell;
  if (active) paintOutline(ctx, ox, oy, cell, 3, 2, ACTIVE_RING);
  if (selected) paintOutline(ctx, ox, oy, cell, 6, 2, SELECT_RING);
}

function drawGrid(ctx: CanvasRenderingContext2D, props: GridProps, cell: number) {
  const { width, height, tiles, units } = props;
  const reachable = props.reachable ?? [];
  const targets = props.targets ?? [];
  const path = props.path ?? [];
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, width * cell, height * cell);
  ctx.fillStyle = INK;
  ctx.fillRect(0, 0, width * cell, height * cell);

  const scratch = document.createElement("canvas");
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const tile = knownTile(tiles[cellIndex(x, y)]);
      const sprite = tile != null ? (props.tileSprite?.(tile) ?? null) : null;
      drawTile(ctx, scratch, tile, sprite, x * cell, y * cell, cell);
    }
  }

  const pixelW = width * cell;
  const pixelH = height * cell;
  ctx.fillStyle = GRID_LINE;
  for (let x = 0; x <= width; x++) {
    const px = x === width ? pixelW - 1 : x * cell;
    ctx.fillRect(px, 0, 1, pixelH);
  }
  for (let y = 0; y <= height; y++) {
    const py = y === height ? pixelH - 1 : y * cell;
    ctx.fillRect(0, py, pixelW, 1);
  }

  ctx.fillStyle = REACHABLE;
  for (const spot of reachable) {
    if (!inMap(spot, width, height)) continue;
    ctx.fillRect(spot.x * cell, spot.y * cell, cell, cell);
  }

  ctx.fillStyle = PATH_DOT;
  const dot = Math.max(2, Math.floor(cell / 10));
  for (const spot of path) {
    if (!inMap(spot, width, height)) continue;
    const cx = spot.x * cell + Math.floor(cell / 2);
    const cy = spot.y * cell + Math.floor(cell / 2);
    ctx.beginPath();
    ctx.arc(cx, cy, dot, 0, Math.PI * 2);
    ctx.fill();
  }

  for (const unit of paintOrder(units)) {
    const sprite = props.unitSprite?.(unit) ?? null;
    drawUnit(ctx, scratch, unit, cell, sprite, unit.id === props.activeUnitId, unit.id === props.selectedUnitId);
  }

  for (const spot of targets) {
    if (!inMap(spot, width, height)) continue;
    paintOutline(ctx, spot.x * cell, spot.y * cell, cell, 0, 2, TARGET);
  }
}

function cellKey(cell: Cell | null): string | null {
  return cell ? `${cell.x},${cell.y}` : null;
}

export function Grid(props: GridProps) {
  const { width, height, cellPx = 48, onCellClick, onUnitClick, onCellHover } = props;
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const hoverKey = useRef<string | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = context2d(canvas);
    if (!ctx) return;
    drawGrid(ctx, props, cellPx);
  }, [props, cellPx]);

  function handleClick(event: MouseEvent<HTMLCanvasElement>) {
    const cell = cellFromPoint(event.clientX, event.clientY, event.currentTarget.getBoundingClientRect(), width, height);
    if (!cell) return;
    const unit = unitOnCell(props.units, cell);
    if (unit) onUnitClick?.(unit);
    else onCellClick?.(cell);
  }

  function handleMove(event: MouseEvent<HTMLCanvasElement>) {
    const cell = cellFromPoint(event.clientX, event.clientY, event.currentTarget.getBoundingClientRect(), width, height);
    const key = cellKey(cell);
    if (key === hoverKey.current) return;
    hoverKey.current = key;
    onCellHover?.(cell);
  }

  function handleLeave() {
    hoverKey.current = null;
    onCellHover?.(null);
  }

  return (
    <canvas
      ref={canvasRef}
      className="pixel grid-canvas"
      width={width * cellPx}
      height={height * cellPx}
      role="img"
      aria-label={`Battle map, ${width} by ${height} cells`}
      onClick={handleClick}
      onMouseMove={handleMove}
      onMouseLeave={handleLeave}
    />
  );
}
