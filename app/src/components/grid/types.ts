// Props contract for the battle grid (L9.3). Written by the lane owner; the component must not change it.
import type { Cell, Tile } from "../../chain/types";

export type UnitKind = "player" | "monster";

export type GridUnit = {
  /** Turn-order id: 0..4 player slot, 4 + i monster i. Unique per grid. */
  id: number;
  kind: UnitKind;
  x: number;
  y: number;
  hp: number;
  hpMax: number;
  alive: boolean;
  /** Short label for the fallback marker and the tooltip, e.g. "Wolf" or "Aria". */
  label: string;
  /** Footprint in cells (1, or 2 for big monsters). The sprite is drawn bottom-center on its cell at the tile pixel scale. */
  size?: 1 | 2;
};

/**
 * Patch 11: a visible chest (docs/MAP_DESCRIPTION.md 7.1). A closed chest occupies its cell; an
 * opened one is drawn as an outline. `label` is the hover tooltip (e.g. the table's levels).
 * Placeholder look until the chest sprite exists.
 */
export type GridChest = { x: number; y: number; opened: boolean; label: string };

/** Patch 13: the entrance and the exit points (docs/MAP_DESCRIPTION.md 7.1), drawn over their cell. */
export type GridPointKind = "entrance" | "exit";
export type GridPoint = { x: number; y: number; kind: GridPointKind };

/** An aura on the map: its bearer's cell, its reach, and a short label for the tooltip. */
export type GridAura = { center: Cell; radius: number; hostile: boolean; label: string };

export type GridProps = {
  /** Map size in cells, 1..=32 each. */
  width: number;
  height: number;
  /** MapLayout.cells: 1024 cell bytes (tile type in bits 7-3, `cellKind`), index y * 32 + x (always stride 32). */
  tiles: Uint8Array;
  units: GridUnit[];
  /** Unit whose turn it is; gets a highlight ring. */
  activeUnitId?: number | null;
  /** Currently selected unit (clicked); gets a selection ring. */
  selectedUnitId?: number | null;
  /** Cells the active unit can move to; tinted. */
  reachable?: Cell[];
  /** Cells/units that can be targeted; outlined in red. */
  targets?: Cell[];
  /** Path preview, drawn as dots in order. */
  path?: Cell[];
  /** Cells on fire (a resonance's burning cells); tinted orange under the units. */
  burning?: Cell[];
  /**
   * Patch 5: auras around their bearers (docs/SKILL_BUILDER.md 3.5): a square of `radius` cells
   * (Chebyshev, no line of sight) around `center`; `hostile` = a debuff aura on the enemies.
   */
  auras?: GridAura[];
  /** Patch 5: the cells of a line target under the pointer, tinted. */
  line?: Cell[];
  /** Patch 11: visible chests, drawn under the units; the canvas title shows the hovered one's label. */
  chests?: GridChest[];
  /** Patch 13: the entrance and the exit. */
  points?: GridPoint[];
  /**
   * CSS pixel size of one cell on screen. Default 48. With sprites it is rounded down to a whole
   * multiple of the 64 px tile (minimum 64), so the canvas is never resized by CSS and sprites scale
   * by integers, never smoothed. A grid without sprites is shown at exactly this size.
   */
  cellPx?: number;
  /**
   * Rendered sprite for a unit (RGBA from the on-chain rasterizer), or null to draw the fallback
   * marker (colored disc + first letter; players blue, monsters red, dead units gray with a cross).
   */
  unitSprite?: (unit: GridUnit) => ImageData | null;
  /** Rendered tile sprite of a tile type (64x64 RGBA), or null to draw the flat tile color. */
  tileSprite?: (tile: Tile) => ImageData | null;
  /** Rendered sprite of a point, or null to draw its flat marker. */
  pointSprite?: (kind: GridPointKind) => ImageData | null;
  onCellClick?: (cell: Cell) => void;
  onUnitClick?: (unit: GridUnit) => void;
  /** Hover changes, for path previews. null when the pointer leaves the grid. */
  onCellHover?: (cell: Cell | null) => void;
};
