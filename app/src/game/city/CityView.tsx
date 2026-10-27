// The city on the main screen (docs/CITY_DESCRIPTION.md 1, 8): walking is interface only, a
// click on a building opens its interior without a transaction. A closed building is drawn with
// its door boarded up and lets nobody in.
import { useState } from "react";
import { BOARDS_SPRITE, spriteUrl } from "../art";
import { CITY_TILE_BASE, CITY_W_MAX } from "../../chain/city";
import { CELL, PREFABS, prefabUrl } from "./prefabs";
import { isOpen, type CityBuilding, type CityModel } from "./model";

const BUILDING_NAMES: Record<number, string> = { 0: "Tavern", 1: "Artisans' Guild", 2: "Storage" };
export const buildingName = (b: CityBuilding) => BUILDING_NAMES[b.kind] ?? "Building";

export function CityView({ city, onEnter }: { city: CityModel; onEnter: (b: CityBuilding) => void }) {
  const [note, setNote] = useState<string | null>(null);
  const tiles = [];
  for (let y = 0; y < city.height; y++) {
    for (let x = 0; x < city.width; x++) {
      const url = spriteUrl(CITY_TILE_BASE + (city.cells[y * CITY_W_MAX + x] ?? 0));
      if (url) tiles.push(<img key={`t${x},${y}`} className="pixel city-cell" src={url} alt="" style={{ left: x * CELL, top: y * CELL }} />);
    }
  }
  // Draw order of maps (docs/CITY_DESCRIPTION.md 8): by the bottom row, so lower things cover upper ones.
  const drawn = city.buildings
    .flatMap((b) => {
      const p = PREFABS[b.prefabId];
      return p && prefabUrl(p) ? [{ b, p }] : [];
    })
    .sort((a, b) => a.b.y + a.p.h - (b.b.y + b.p.h));
  const boards = spriteUrl(BOARDS_SPRITE);

  return (
    <div className="city-scroll">
      <div className="city" style={{ width: city.width * CELL, height: city.height * CELL }} onMouseLeave={() => setNote(null)}>
        {tiles}
        {city.objects.map((o, i) => {
          const url = spriteUrl(o.sprite);
          return url ? <img key={`o${i}`} className="pixel city-cell" src={url} alt="" style={{ left: o.x * CELL, top: o.y * CELL, zIndex: o.y + 1 }} /> : null;
        })}
        {drawn.map(({ b, p }) => {
          const open = isOpen(b);
          const name = buildingName(b);
          return (
            <button
              key={b.address}
              type="button"
              className={`city-building${open ? "" : " closed"}`}
              style={{ left: b.x * CELL, top: b.y * CELL, width: p.w * CELL, height: p.h * CELL, zIndex: b.y + p.h }}
              aria-label={open ? `Enter the ${name}` : `${name} (closed)`}
              title={open ? name : `${name}: closed`}
              onMouseEnter={() => setNote(open ? name : `${name}: closed for now`)}
              onClick={() => (open ? onEnter(b) : setNote(`${name}: the door is boarded up`))}
            >
              <img className="pixel" src={prefabUrl(p)} alt="" draggable={false} />
              {!open && boards && <img className="pixel city-boards" src={boards} alt="" draggable={false} style={{ left: p.boards.x, top: p.boards.y }} />}
            </button>
          );
        })}
      </div>
      <p className="city-note muted" aria-live="polite">
        {note ?? "Click a building to go in."}
      </p>
    </div>
  );
}
