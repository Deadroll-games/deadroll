// Inventory: the equipment slots with what is worn, and every soulbound item with its bonus and
// a Wear / Take off button. Pure UI over InventoryProps.
import type { InventoryItemView, InventoryProps } from "./types";
import "./inventory.css";

function WearButtons({ item, busy, locked, onEquip, label }: { item: InventoryItemView; busy: boolean; locked: boolean; onEquip: InventoryProps["onEquip"]; label: (key: string) => string }) {
  const free = item.fits.filter((s) => s !== item.worn);
  return (
    <>
      {free.map((slot) => (
        <button key={slot} type="button" className="btn" disabled={busy || locked} onClick={() => onEquip(item.index, slot)}>
          {item.fits.length > 1 ? `Wear in ${label(slot)}` : "Wear"}
        </button>
      ))}
    </>
  );
}

export function Inventory({ slots, items, locked, busy, onEquip, onUnequip, onCreate, icon }: InventoryProps) {
  const label = (key: string) => slots.find((s) => s.key === key)?.label ?? key;
  return (
    <div className="inventory">
      {locked && <p className="muted">{locked}</p>}
      <ul className="equip-grid" aria-label="equipment">
        {slots.map((slot) => {
          const item = items?.find((i) => i.worn === slot.key) ?? null;
          return (
            <li key={slot.key} className={`equip-slot${item ? " worn" : ""}`} data-testid={`slot-${slot.key}`}>
              <span className="equip-slot-name">{slot.label}</span>
              {item ? (
                <>
                  <span className="equip-icon">{icon(item.base)}</span>
                  <span>{item.label}</span>
                  <button type="button" className="btn" disabled={busy || locked !== null} onClick={() => onUnequip(slot.key)}>
                    Take off
                  </button>
                </>
              ) : (
                <span className="muted">empty</span>
              )}
            </li>
          );
        })}
      </ul>
      {items === null ? (
        <p>
          <span className="muted">No items yet: this character was created before items existed. </span>
          {onCreate && (
            <button type="button" className="btn" disabled={busy || locked !== null} onClick={onCreate}>
              Get the class starter kit
            </button>
          )}
        </p>
      ) : (
        <table className="inventory-items">
          <thead>
            <tr>
              <th />
              <th>Item</th>
              <th>Level</th>
              <th>Gives</th>
              <th>Worn</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {items.map((item) => (
              <tr key={item.index}>
                <td className="equip-icon">{icon(item.base)}</td>
                <td>{item.label} <span className="muted">· soulbound</span></td>
                <td>{item.level}</td>
                <td>{item.bonus}</td>
                <td>{item.worn ? label(item.worn) : <span className="muted">in the bag</span>}</td>
                <td>
                  <WearButtons item={item} busy={busy} locked={locked !== null} onEquip={onEquip} label={label} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
