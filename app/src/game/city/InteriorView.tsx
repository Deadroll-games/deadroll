// A building's interior (docs/CITY_DESCRIPTION.md 7): site only. Furniture is decor; the
// interactive points open the building's windows (the Tavern: the board and the DM's table).
import { useState } from "react";
import { spriteUrl } from "../art";
import { CELL, floorSprite, wallSprite, type Interior, type InteriorItem } from "./prefabs";

export function InteriorView({ interior, onAction }: { interior: Interior; onAction: (action: NonNullable<InteriorItem["action"]>) => void }) {
  const [hover, setHover] = useState<string | null>(null);
  const floor = [];
  for (let y = 0; y < interior.h; y++) {
    for (let x = 0; x < interior.w; x++) {
      const url = spriteUrl(y < interior.wallRows ? wallSprite() : floorSprite(x, y));
      if (url) floor.push(<img key={`${x},${y}`} className="pixel city-cell" src={url} alt="" style={{ left: x * CELL, top: y * CELL }} />);
    }
  }
  return (
    <div className="city-scroll">
      <h2 className="interior-title">{interior.name}</h2>
      <div className="city interior" style={{ width: interior.w * CELL, height: interior.h * CELL }}>
        {floor}
        {interior.items.map((it, i) => {
          const url = spriteUrl(it.sprite);
          const style = { left: it.x * CELL, top: it.y * CELL, zIndex: it.y + 1 };
          if (!it.action) return url ? <img key={i} className="pixel city-cell" src={url} alt="" style={style} /> : null;
          return (
            <button
              key={i}
              type="button"
              className={`interior-point ${it.action}`}
              style={{ ...style, width: CELL, height: CELL }}
              aria-label={it.label}
              title={it.label}
              onMouseEnter={() => setHover(it.label ?? null)}
              onMouseLeave={() => setHover(null)}
              onClick={() => onAction(it.action!)}
            >
              {it.action === "exit" ? <span className="interior-exit">Exit</span> : url && <img className="pixel" src={url} alt="" draggable={false} />}
            </button>
          );
        })}
      </div>
      <p className="city-note muted" aria-live="polite">
        {hover ?? "Look around: the glowing spots open."}
      </p>
    </div>
  );
}
