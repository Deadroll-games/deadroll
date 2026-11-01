// Patch D: the whole of a big map on the parchment (docs/MAP_DESCRIPTION.md 17.6), next to the grid
// that shows a window of it: the tiles in flat colors, the units as dots, the window as a frame.
// A click moves the window there.
import { useEffect, useRef, type MouseEvent } from "react";
import { cellKind, type Cell } from "../../chain/types";
import { artUrl } from "../../game/art";

/** The drawable area inside the parchment frame (screens/parchment.png is 384x256). */
const AREA = { x: 40, y: 34, w: 304, h: 188 };
/** `TileKind` (crates/types) by color: walls dark, water blue, the rest floor. */
const KIND_COLORS: Record<number, string> = { 0: "transparent", 4: "#3b2a1c", 5: "#8a5a2b", 6: "#5d7f8f", 7: "#3e5f73", 10: "#1c140e" };
const FLOOR = "#9c8358";
const PLAYER = "#7a1f14";
const MONSTER = "#2a1a40";
const FRAME = "#7a1f14";

export type MinimapDot = { x: number; y: number; kind: "player" | "monster" };

/** Pixels per cell and the offset that center a `width` x `height` map in the area. */
export function minimapScale(width: number, height: number): { cell: number; ox: number; oy: number } {
  const cell = Math.max(1, Math.floor(Math.min(AREA.w / width, AREA.h / height)));
  return { cell, ox: Math.floor((AREA.w - cell * width) / 2), oy: Math.floor((AREA.h - cell * height) / 2) };
}

export function ParchmentMinimap(props: {
  width: number;
  height: number;
  /** `width` per row, as every tile array. */
  tiles: Uint8Array;
  dots: MinimapDot[];
  /** The window the grid shows, in cells. */
  view: { x: number; y: number; cols: number; rows: number };
  onJump: (center: Cell) => void;
}) {
  const { width, height, tiles, dots, view, onJump } = props;
  const canvas = useRef<HTMLCanvasElement>(null);
  const { cell, ox, oy } = minimapScale(width, height);

  // The tiles change rarely: drawn on their own layer, the frame and the dots over a copy.
  const base = useRef<HTMLCanvasElement | null>(null);
  useEffect(() => {
    const layer = document.createElement("canvas");
    layer.width = AREA.w;
    layer.height = AREA.h;
    const ctx = layer.getContext("2d");
    if (!ctx) return;
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const color = KIND_COLORS[cellKind(tiles[y * width + x] ?? 0)] ?? FLOOR;
        if (color === "transparent") continue;
        ctx.fillStyle = color;
        ctx.fillRect(ox + x * cell, oy + y * cell, cell, cell);
      }
    }
    base.current = layer;
  }, [tiles, width, height, cell, ox, oy]);

  useEffect(() => {
    const ctx = canvas.current?.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, AREA.w, AREA.h);
    if (base.current) ctx.drawImage(base.current, 0, 0);
    for (const d of dots) {
      ctx.fillStyle = d.kind === "player" ? PLAYER : MONSTER;
      const r = Math.max(1.5, cell * 0.6);
      ctx.beginPath();
      ctx.arc(ox + d.x * cell + cell / 2, oy + d.y * cell + cell / 2, r, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.strokeStyle = FRAME;
    ctx.lineWidth = 1.5;
    ctx.strokeRect(ox + view.x * cell + 0.5, oy + view.y * cell + 0.5, view.cols * cell - 1, view.rows * cell - 1);
  }, [dots, view, cell, ox, oy, tiles]);

  function click(e: MouseEvent<HTMLCanvasElement>) {
    const r = e.currentTarget.getBoundingClientRect();
    const px = ((e.clientX - r.left) / r.width) * AREA.w;
    const py = ((e.clientY - r.top) / r.height) * AREA.h;
    const x = Math.floor((px - ox) / cell);
    const y = Math.floor((py - oy) / cell);
    if (x >= 0 && y >= 0 && x < width && y < height) onJump({ x, y });
  }

  const frame = artUrl("screens/parchment");
  return (
    <div className="minimap parchment-minimap" style={frame ? { backgroundImage: `url(${frame})` } : undefined}>
      <canvas
        ref={canvas}
        width={AREA.w}
        height={AREA.h}
        style={{ left: AREA.x, top: AREA.y }}
        role="img"
        aria-label={`Map overview, ${width} by ${height} cells: click to look there`}
        onClick={click}
      />
    </div>
  );
}
