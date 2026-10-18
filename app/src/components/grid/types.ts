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
  /** Sprite size in cells: 1 for 32 px, 2 for 64 px monsters. Drawn bottom-center on its cell. */
  size?: 1 | 2;
};

export type GridProps = {
  /** Map size in cells, 1..=32 each. */
  width: number;
  height: number;
  /** MapLayout.tiles: 1024 bytes, index y * 32 + x (always stride 32), values are Tile. */
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
  /** Pixel size of one cell on screen. Default 48. Sprites scale by integers, never smoothed. */
  cellPx?: number;
  /**
   * Rendered sprite for a unit (RGBA from the on-chain rasterizer), or null to draw the fallback
   * marker (colored disc + first letter; players blue, monsters red, dead units gray with a cross).
   */
  unitSprite?: (unit: GridUnit) => ImageData | null;
  /** Rendered tile sprite (32x32 RGBA), or null to draw the flat tile color. */
  tileSprite?: (tile: Tile) => ImageData | null;
  onCellClick?: (cell: Cell) => void;
  onUnitClick?: (unit: GridUnit) => void;
  /** Hover changes, for path previews. null when the pointer leaves the grid. */
  onCellHover?: (cell: Cell | null) => void;
};
