// The dungeon of the current party on the parchment, small: the whole map and the party's
// markers, live. Outside a party it is a blank parchment.
import { useEffect, useRef } from "react";
import { MONSTER_BASE } from "../chain/game";
import { MAP_MAX } from "../chain/types";
import { artUrl } from "./art";
import { useLiveParty } from "../routes/PartyChain";

/** The drawable area inside the parchment frame (screens/parchment.png is 384x256). */
const AREA = { x: 40, y: 34, w: 304, h: 188 };
/** `TileKind` (crates/types) by color: walls dark, water blue, the rest floor. */
const KIND_COLORS: Record<number, string> = { 0: "transparent", 4: "#3b2a1c", 5: "#8a5a2b", 6: "#5d7f8f", 7: "#3e5f73", 10: "#1c140e" };
const FLOOR = "#9c8358";

export function Minimap({ onOpen }: { onOpen?: () => void }) {
  const live = useLiveParty(null);
  const party = live?.party ?? null;
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const ctx = canvas.current?.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, AREA.w, AREA.h);
    if (!party?.tiles) return;
    const s = party.session;
    const cell = Math.max(1, Math.floor(Math.min(AREA.w / s.width, AREA.h / s.height)));
    const ox = Math.floor((AREA.w - cell * s.width) / 2);
    const oy = Math.floor((AREA.h - cell * s.height) / 2);
    for (let y = 0; y < s.height; y++) {
      for (let x = 0; x < s.width; x++) {
        const kind = (party.tiles[y * MAP_MAX + x] ?? 0) >> 3;
        const color = KIND_COLORS[kind] ?? FLOOR;
        if (color === "transparent") continue;
        ctx.fillStyle = color;
        ctx.fillRect(ox + x * cell, oy + y * cell, cell, cell);
      }
    }
    // The party's markers: each living hero on its cell.
    for (const p of party.players) {
      if (!(s.aliveMask & (1 << p.slot)) || p.slot >= MONSTER_BASE) continue;
      ctx.fillStyle = "#7a1f14";
      const r = Math.max(2, Math.floor(cell * 0.4));
      ctx.beginPath();
      ctx.arc(ox + p.part.core.x * cell + cell / 2, oy + p.part.core.y * cell + cell / 2, r, 0, Math.PI * 2);
      ctx.fill();
    }
  }, [party]);

  const frame = artUrl("screens/parchment");
  const active = !!party;
  return (
    <button
      type="button"
      className={`minimap${active ? "" : " inactive"}`}
      style={frame ? { backgroundImage: `url(${frame})` } : undefined}
      disabled={!active}
      onClick={onOpen}
      aria-label={active ? "Map of the dungeon: back to the party" : "Map of the dungeon: not in a party"}
      title={active ? "Back to the dungeon" : "Not in a party"}
    >
      <canvas ref={canvas} width={AREA.w} height={AREA.h} style={{ left: AREA.x, top: AREA.y }} />
      {!active && <span className="minimap-empty">Not in a party</span>}
    </button>
  );
}
