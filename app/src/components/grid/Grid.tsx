// Battle map canvas: tiles, units, move highlights, and cell clicks.
import { useEffect, useMemo, useRef, useState, type MouseEvent, type WheelEvent } from "react";
import { TILE_COUNT, Tile, cellIndex, cellKind, cellVariant, type Cell } from "../../chain/types";
import { ParchmentMinimap } from "./ParchmentMinimap";
import { wallMask } from "../../chain/tiles";
import { TILE_PX } from "../../raster/SpriteCanvas";
import type { GridPoint, GridProps, GridSecret, GridUnit } from "./types";
import "./grid.css";

const FLOOR = "#3b3043";
const WALL = "#17111c";
const WALL_EDGE = "#3d3450";
const WATER = "#2b4f7a";
const DEEP_WATER = "#1b3352";
const DOOR = "#7a5a2b";
const WOOD = "#4a3628";
const DIRT = "#3f3a2a";
const STAIRS = "#5a4f62";
const RUBBLE = "#4b4350";
const VOID = "#07050a";
const ENTRANCE = "#2f6b3a";
const EXIT = "#8a6a1f";
const GRID_LINE = "rgba(0,0,0,.25)";
const REACHABLE = "rgba(79,143,217,.28)";
const BURNING = "rgba(230,120,40,.45)";
const AURA = "rgba(120,200,255,.9)";
const AURA_FILL = "rgba(120,200,255,.10)";
const AURA_HOSTILE = "rgba(200,120,255,.9)";
const AURA_HOSTILE_FILL = "rgba(200,120,255,.10)";
const LINE = "rgba(214,90,74,.22)";
const TARGET = "#d65a4a";
const PLAYER = "#4f8fd9";
const MONSTER = "#c4473a";
const DEAD = "#7d7786";
const ACTIVE_RING = "#d9a441";
const SELECT_RING = "#ffffff";
const PATH_DOT = "#d9a441";
const CHEST = "#8b5a2b";
const CHEST_EDGE = "#d9a441";
const INK = "#120c16";
const LETTER = "#f4efe6";

type Rect = { left: number; top: number; width: number; height: number };

/**
 * Canvas pixels per cell: with sprites a whole multiple of the 64 px tile (minimum one tile), so every
 * sprite pixel is an integer number of screen pixels and the canvas is never resized by CSS.
 */
export function integerCell(cellPx: number): number {
  return TILE_PX * Math.max(1, Math.floor(cellPx / TILE_PX));
}

/** Backing cell for a flat-color grid (no sprites): follows `cellPx`, at least one tile. */
export function backingCell(cellPx: number): number {
  return Math.max(TILE_PX, Math.round(cellPx));
}

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

/** The tile type of a cell byte, null for a type the starter set does not have. */
function knownTile(value: number | undefined): Tile | null {
  const kind = cellKind(value ?? 0);
  return kind < TILE_COUNT ? (kind as Tile) : null;
}

const FLAT: Partial<Record<Tile, string>> = {
  [Tile.Void]: VOID,
  [Tile.Chasm]: VOID,
  [Tile.WoodenFloor]: WOOD,
  [Tile.Dirt]: DIRT,
  [Tile.ShallowWater]: WATER,
  [Tile.DeepWater]: DEEP_WATER,
  [Tile.Door]: DOOR,
  [Tile.Stairs]: STAIRS,
  [Tile.Rubble]: RUBBLE,
};

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

/**
 * Size of a unit sprite at the tile pixel scale: a cell is one 64 px tile, so a 96 px character or
 * a 128 px monster keeps the same pixel size as the floor under it.
 */
export function unitDest(image: { width: number; height: number }, cell: number): { w: number; h: number } {
  return { w: Math.round((image.width * cell) / TILE_PX), h: Math.round((image.height * cell) / TILE_PX) };
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

/** Cells per side of a unit (patch B: a 2x2 monster stands on its whole square). */
const sideOf = (unit: GridUnit) => unit.size ?? 1;

/** Live unit on a cell (any cell of a 2x2 one). Corpses and tombstones do not occupy a cell, so a
 * click on one is a cell click (a move). */
function unitOnCell(units: readonly GridUnit[], cell: Cell): GridUnit | undefined {
  const ordered = paintOrder(units);
  for (let i = ordered.length - 1; i >= 0; i--) {
    const unit = ordered[i];
    const n = unit ? sideOf(unit) : 1;
    if (unit && unit.alive && cell.x >= unit.x && cell.x < unit.x + n && cell.y >= unit.y && cell.y < unit.y + n) return unit;
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
  ctx.fillStyle = tile === null ? VOID : (FLAT[tile] ?? FLOOR);
  ctx.fillRect(x, y, cell, cell);
}

/** The entrance or the exit: its sprite over the cell, or a colored frame with a letter. */
function drawPoint(ctx: CanvasRenderingContext2D, scratch: HTMLCanvasElement, point: GridPoint, sprite: ImageData | null, cell: number) {
  const x = point.x * cell;
  const y = point.y * cell;
  if (sprite) {
    paintImage(ctx, scratch, sprite, x, y, cell, cell);
    return;
  }
  const color = point.kind === "entrance" ? ENTRANCE : EXIT;
  paintOutline(ctx, x, y, cell, Math.max(1, Math.floor(cell / 10)), Math.max(2, Math.floor(cell / 12)), color);
  ctx.fillStyle = color;
  ctx.font = `${Math.max(9, Math.floor(cell * 0.34))}px "Inter", system-ui, sans-serif`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(point.kind === "entrance" ? "E" : "X", x + cell / 2, y + cell / 2 + 1);
}

function drawMarker(ctx: CanvasRenderingContext2D, unit: GridUnit, cell: number) {
  const block = cell * sideOf(unit);
  const cx = unit.x * cell + block / 2;
  const cy = unit.y * cell + block / 2;
  const radius = Math.max(6, Math.floor(block * 0.26));
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

/** Patch C: what a secret looks like, its frame color and letter. */
const SECRET_LOOK: Record<GridSecret["kind"], { color: string; letter: string }> = {
  trap: { color: "#e0523c", letter: "T" },
  ambush: { color: "#c03ad8", letter: "A" },
  stash: { color: "#e2b53a", letter: "$" },
  door: { color: "#4fb3e0", letter: "D" },
};

/** Patch C: a found secret highlighted (a colored frame and letter); an unfound one (the DM's view) faint. */
function drawSecret(ctx: CanvasRenderingContext2D, s: GridSecret, cell: number) {
  const look = SECRET_LOOK[s.kind];
  const x = s.x * cell;
  const y = s.y * cell;
  ctx.save();
  ctx.globalAlpha = s.found ? 1 : 0.45;
  ctx.fillStyle = look.color + (s.found ? "55" : "22");
  ctx.fillRect(x, y, cell, cell);
  paintOutline(ctx, x, y, cell, 1, Math.max(1, Math.floor(cell / 16)), look.color);
  ctx.fillStyle = look.color;
  ctx.font = `bold ${Math.max(9, Math.floor(cell * 0.3))}px "Inter", system-ui, sans-serif`;
  ctx.textAlign = "left";
  ctx.textBaseline = "top";
  ctx.fillText(look.letter, x + 3, y + 2);
  ctx.restore();
}

/** Patch C: a unit's badge (a pack's threat) at its top-right corner. */
function drawBadge(ctx: CanvasRenderingContext2D, unit: GridUnit, cell: number) {
  if (!unit.badge) return;
  const size = Math.max(10, Math.floor(cell * 0.32));
  const right = (unit.x + sideOf(unit)) * cell;
  const top = unit.y * cell;
  ctx.font = `bold ${size}px "Inter", system-ui, sans-serif`;
  const w = Math.ceil(ctx.measureText(unit.badge).width) + 6;
  ctx.fillStyle = "#1b1d24";
  ctx.fillRect(right - w, top, w, size + 4);
  ctx.strokeStyle = "#e0523c";
  ctx.lineWidth = 1;
  ctx.strokeRect(right - w + 0.5, top + 0.5, w - 1, size + 3);
  ctx.fillStyle = "#f2f2f2";
  ctx.textAlign = "center";
  ctx.textBaseline = "top";
  ctx.fillText(unit.badge, right - w / 2, top + 2);
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
  // Patch B: a 2x2 monster's block is its whole square.
  const block = cell * sideOf(unit);
  if (sprite) {
    // Bottom-center on its block; a canvas wider or taller than the block overlaps its neighbors.
    const dest = unitDest(sprite, cell);
    const dx = Math.round(unit.x * cell + block / 2 - dest.w / 2);
    const dy = Math.round(unit.y * cell + block - dest.h);
    paintImage(ctx, scratch, sprite, dx, dy, dest.w, dest.h);
  } else {
    drawMarker(ctx, unit, cell);
  }

  if (unit.alive && unit.hpMax > 0) {
    const ratio = Math.max(0, Math.min(1, unit.hp / unit.hpMax));
    const barH = Math.max(3, Math.floor(cell / 12));
    const barW = block - 8;
    const bx = unit.x * cell + 4;
    const by = unit.y * cell + block - 8 - barH;
    ctx.fillStyle = INK;
    ctx.fillRect(bx, by, barW, barH);
    ctx.fillStyle = hpColor(ratio);
    ctx.fillRect(bx, by, Math.round(barW * ratio), barH);
  }

  const ox = unit.x * cell;
  const oy = unit.y * cell;
  if (active) paintOutline(ctx, ox, oy, block, 3, 2, ACTIVE_RING);
  if (selected) paintOutline(ctx, ox, oy, block, 6, 2, SELECT_RING);
}

/** Patch D: the cells a grid shows, its top-left cell and size. */
export type View = { x: number; y: number; cols: number; rows: number };

/** Past this side a map is drawn through a window (patch D: the Small class fits whole). */
export const WHOLE_MAP_MAX = 32;
export const DEFAULT_VIEW = { cols: 24, rows: 14 };

/** The window size of a `width` x `height` map. */
export function viewSize(width: number, height: number, viewport?: { cols: number; rows: number }): { cols: number; rows: number } {
  if (!viewport && width <= WHOLE_MAP_MAX && height <= WHOLE_MAP_MAX) return { cols: width, rows: height };
  const v = viewport ?? DEFAULT_VIEW;
  return { cols: Math.min(width, Math.max(1, v.cols)), rows: Math.min(height, Math.max(1, v.rows)) };
}

/** A window of `cols` x `rows` cells moved to keep inside the map. */
export function clampView(x: number, y: number, cols: number, rows: number, width: number, height: number): View {
  return { x: Math.max(0, Math.min(width - cols, Math.round(x))), y: Math.max(0, Math.min(height - rows, Math.round(y))), cols, rows };
}

function drawGrid(ctx: CanvasRenderingContext2D, props: GridProps, cell: number, view: View) {
  const { width, height, tiles, units } = props;
  const reachable = props.reachable ?? [];
  const targets = props.targets ?? [];
  const path = props.path ?? [];
  const { cols, rows } = view;
  ctx.imageSmoothingEnabled = false;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, cols * cell, rows * cell);
  ctx.fillStyle = INK;
  ctx.fillRect(0, 0, cols * cell, rows * cell);
  // Patch D: everything is drawn in map pixels; the window shifts it.
  ctx.translate(-view.x * cell, -view.y * cell);

  const scratch = document.createElement("canvas");
  const x1 = Math.min(width, view.x + cols);
  const y1 = Math.min(height, view.y + rows);
  for (let y = view.y; y < y1; y++) {
    for (let x = view.x; x < x1; x++) {
      const byte = tiles[cellIndex(x, y, width)];
      const tile = knownTile(byte);
      const mask = tile === Tile.Wall ? wallMask(tiles, width, height, x, y) : 0;
      const sprite = tile != null ? (props.tileSprite?.({ tile, variant: cellVariant(byte ?? 0), mask }) ?? null) : null;
      drawTile(ctx, scratch, tile, sprite, x * cell, y * cell, cell);
    }
  }

  const left = view.x * cell;
  const top = view.y * cell;
  const pixelW = x1 * cell;
  const pixelH = y1 * cell;
  ctx.fillStyle = GRID_LINE;
  for (let x = view.x; x <= x1; x++) {
    const px = x === x1 ? pixelW - 1 : x * cell;
    ctx.fillRect(px, top, 1, pixelH - top);
  }
  for (let y = view.y; y <= y1; y++) {
    const py = y === y1 ? pixelH - 1 : y * cell;
    ctx.fillRect(left, py, pixelW - left, 1);
  }

  ctx.fillStyle = BURNING;
  for (const spot of props.burning ?? []) {
    if (!inMap(spot, width, height)) continue;
    ctx.fillRect(spot.x * cell, spot.y * cell, cell, cell);
  }

  // Auras: a dashed square of `radius` cells around the bearer (Chebyshev reach).
  for (const a of props.auras ?? []) {
    const x0 = Math.max(0, a.center.x - a.radius);
    const y0 = Math.max(0, a.center.y - a.radius);
    const x1 = Math.min(width - 1, a.center.x + a.radius);
    const y1 = Math.min(height - 1, a.center.y + a.radius);
    ctx.fillStyle = a.hostile ? AURA_HOSTILE_FILL : AURA_FILL;
    ctx.fillRect(x0 * cell, y0 * cell, (x1 - x0 + 1) * cell, (y1 - y0 + 1) * cell);
    ctx.save();
    ctx.strokeStyle = a.hostile ? AURA_HOSTILE : AURA;
    ctx.lineWidth = Math.max(1, Math.floor(cell / 24));
    ctx.setLineDash([Math.max(2, Math.floor(cell / 8)), Math.max(2, Math.floor(cell / 8))]);
    ctx.strokeRect(x0 * cell + 1, y0 * cell + 1, (x1 - x0 + 1) * cell - 2, (y1 - y0 + 1) * cell - 2);
    ctx.restore();
  }

  ctx.fillStyle = LINE;
  for (const spot of props.line ?? []) {
    if (!inMap(spot, width, height)) continue;
    ctx.fillRect(spot.x * cell, spot.y * cell, cell, cell);
  }

  ctx.fillStyle = REACHABLE;
  for (const spot of reachable) {
    if (!inMap(spot, width, height)) continue;
    ctx.fillRect(spot.x * cell, spot.y * cell, cell, cell);
  }

  for (const point of props.points ?? []) {
    if (!inMap(point, width, height)) continue;
    drawPoint(ctx, scratch, point, props.pointSprite?.(point.kind) ?? null, cell);
  }

  // Patch C: the secrets the viewer may see, over the floor.
  for (const s of props.secrets ?? []) {
    if (inMap(s, width, height)) drawSecret(ctx, s, cell);
  }

  // Chests: the sprite when there is one; otherwise a brown square while closed, only its outline
  // once opened.
  const inset = Math.max(2, Math.floor(cell / 5));
  for (const chest of props.chests ?? []) {
    if (!inMap(chest, width, height)) continue;
    const sprite = props.chestSprite?.(chest.opened) ?? null;
    if (sprite) {
      paintImage(ctx, scratch, sprite, chest.x * cell, chest.y * cell, cell, cell);
      continue;
    }
    const thickness = Math.max(1, Math.floor(cell / 16));
    if (!chest.opened) {
      ctx.fillStyle = CHEST;
      ctx.fillRect(chest.x * cell + inset, chest.y * cell + inset, cell - inset * 2, cell - inset * 2);
    }
    paintOutline(ctx, chest.x * cell, chest.y * cell, cell, inset, thickness, CHEST_EDGE);
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
  for (const unit of units) if (unit.alive) drawBadge(ctx, unit, cell);

  for (const spot of targets) {
    if (!inMap(spot, width, height)) continue;
    paintOutline(ctx, spot.x * cell, spot.y * cell, cell, 0, 2, TARGET);
  }
  ctx.setTransform(1, 0, 0, 1, 0, 0);
}

function cellKey(cell: Cell | null): string | null {
  return cell ? `${cell.x},${cell.y}` : null;
}

export function Grid(props: GridProps) {
  const { width, height, cellPx = 48, onCellClick, onUnitClick, onCellHover } = props;
  // Sprites are pixel art: whole tile multiples, CSS size equal to the backing size. A flat-color
  // grid (no sprites) has no pixels to keep square and may be shown at any cellPx.
  const pixelArt = Boolean(props.tileSprite || props.unitSprite);
  const cell = pixelArt ? integerCell(cellPx) : backingCell(cellPx);
  const shownCell = pixelArt ? cell : cellPx;
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const hoverKey = useRef<string | null>(null);
  const [hovered, setHovered] = useState<Cell | null>(null);
  const hoveredChest = hovered ? props.chests?.find((c) => c.x === hovered.x && c.y === hovered.y) : undefined;
  const hoveredSecret = hovered ? props.secrets?.find((c) => c.x === hovered.x && c.y === hovered.y) : undefined;

  // Patch D: the window over a big map.
  const { cols, rows } = viewSize(width, height, props.viewport);
  const windowed = cols < width || rows < height;
  const [origin, setOrigin] = useState({ x: 0, y: 0 });
  const view = clampView(origin.x, origin.y, cols, rows, width, height);
  const lookAt = (c: Cell) => setOrigin({ x: c.x - Math.floor(cols / 2), y: c.y - Math.floor(rows / 2) });
  const followKey = props.follow ? `${props.follow.x},${props.follow.y}` : null;
  useEffect(() => {
    if (!props.follow || !windowed) return;
    const f = props.follow;
    // Only when the cell leaves the window's inner part.
    const margin = 2;
    if (f.x < view.x + margin || f.y < view.y + margin || f.x >= view.x + cols - margin || f.y >= view.y + rows - margin) lookAt(f);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [followKey, windowed]);
  const drag = useRef<{ x: number; y: number; ox: number; oy: number } | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = context2d(canvas);
    if (!ctx) return;
    drawGrid(ctx, props, cell, view);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props, cell, view.x, view.y, view.cols, view.rows]);

  const dots = useMemo(
    () => (windowed ? props.units.filter((u) => u.alive).map((u) => ({ x: u.x, y: u.y, kind: u.kind })) : []),
    [windowed, props.units],
  );

  /** The map cell under the pointer (the window's cell plus its corner). */
  function cellAt(event: MouseEvent<HTMLCanvasElement>): Cell | null {
    const c = cellFromPoint(event.clientX, event.clientY, event.currentTarget.getBoundingClientRect(), cols, rows);
    return c && { x: c.x + view.x, y: c.y + view.y };
  }

  function handleWheel(event: WheelEvent<HTMLCanvasElement>) {
    if (!windowed) return;
    const step = Math.sign(event.deltaY) * 2;
    if (event.shiftKey || Math.abs(event.deltaX) > Math.abs(event.deltaY)) setOrigin({ x: view.x + (step || Math.sign(event.deltaX) * 2), y: view.y });
    else setOrigin({ x: view.x, y: view.y + step });
  }

  function handleDown(event: MouseEvent<HTMLCanvasElement>) {
    // The middle button drags the window.
    if (!windowed || event.button !== 1) return;
    event.preventDefault();
    drag.current = { x: event.clientX, y: event.clientY, ox: view.x, oy: view.y };
  }

  function handleUp() {
    drag.current = null;
  }

  function handleClick(event: MouseEvent<HTMLCanvasElement>) {
    const cell = cellAt(event);
    if (!cell) return;
    const unit = unitOnCell(props.units, cell);
    if (unit) onUnitClick?.(unit);
    else onCellClick?.(cell);
  }

  function handleMove(event: MouseEvent<HTMLCanvasElement>) {
    if (drag.current) {
      const rect = event.currentTarget.getBoundingClientRect();
      const per = rect.width / cols;
      setOrigin({ x: drag.current.ox - (event.clientX - drag.current.x) / per, y: drag.current.oy - (event.clientY - drag.current.y) / per });
      return;
    }
    const cell = cellAt(event);
    const key = cellKey(cell);
    if (key === hoverKey.current) return;
    hoverKey.current = key;
    setHovered(cell);
    onCellHover?.(cell);
  }

  function handleLeave() {
    drag.current = null;
    hoverKey.current = null;
    setHovered(null);
    onCellHover?.(null);
  }

  const canvas = (
    <canvas
      ref={canvasRef}
      className="pixel grid-canvas"
      width={cols * cell}
      height={rows * cell}
      style={{ width: cols * shownCell, height: rows * shownCell }}
      role="img"
      aria-label={windowed ? `Battle map, ${width} by ${height} cells, showing ${cols} by ${rows} from ${view.x}, ${view.y}` : `Battle map, ${width} by ${height} cells`}
      title={hoveredChest?.label ?? hoveredSecret?.label}
      onClick={handleClick}
      onMouseMove={handleMove}
      onMouseLeave={handleLeave}
      onMouseDown={handleDown}
      onMouseUp={handleUp}
      onWheel={handleWheel}
    />
  );
  if (!windowed) return canvas;
  return (
    <div className="grid-view">
      {canvas}
      <ParchmentMinimap width={width} height={height} tiles={props.tiles} dots={dots} view={view} onJump={lookAt} />
    </div>
  );
}
