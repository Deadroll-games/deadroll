// The panel to the right of the main screen, the same in the city and in battle: the minimap on
// parchment, then one button per row (bag, skills, characters, back to the dungeon, the city,
// settings).
import { ICONS, spriteUrl } from "./art";
import { Minimap } from "./Minimap";

export type PanelAction = "bag" | "skills" | "characters" | "dungeon" | "city" | "settings";

const ROWS: { action: PanelAction; icon: number; label: string }[] = [
  { action: "bag", icon: ICONS.bag, label: "Bag" },
  { action: "skills", icon: ICONS.skills, label: "Skills" },
  { action: "characters", icon: ICONS.characters, label: "Characters" },
  { action: "dungeon", icon: ICONS.map, label: "Dungeon" },
  { action: "city", icon: ICONS.city, label: "City" },
  { action: "settings", icon: ICONS.settings, label: "Settings" },
];

export function SidePanel({
  current,
  disabled,
  activeName,
  onAction,
}: {
  /** The open window or screen, highlighted. */
  current: PanelAction | null;
  /** Buttons that do nothing now (no party for the dungeon). */
  disabled: Partial<Record<PanelAction, string>>;
  activeName: string | null;
  onAction: (a: PanelAction) => void;
}) {
  return (
    <aside className="side-panel" aria-label="Game panel">
      <Minimap onOpen={() => onAction("dungeon")} />
      <p className="side-active muted">{activeName ? `Active: ${activeName}` : "No active character"}</p>
      <nav className="side-buttons">
        {ROWS.map((r) => {
          const why = disabled[r.action];
          return (
            <button
              key={r.action}
              type="button"
              className="side-button"
              aria-pressed={current === r.action}
              disabled={!!why}
              title={why ?? r.label}
              onClick={() => onAction(r.action)}
            >
              <img className="pixel" src={spriteUrl(r.icon)} alt="" width={64} height={64} draggable={false} />
              <span>{r.label}</span>
            </button>
          );
        })}
      </nav>
    </aside>
  );
}
