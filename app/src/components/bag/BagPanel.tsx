// The bag and the settled cache side by side (patch 11): drag a reward onto a slot, Shift + click
// to send it to the first fitting slots, click a cache reward then a bag slot to replace, split a
// stack, throw a slot away (asked first), lock a slot in this browser. Placeholder look until the
// item icons exist.
import { useState, type DragEvent } from "react";
import { slotKey, type SlotRef } from "../../chain/bagLayout";
import type { Reward } from "../../chain/loot";
import type { BagPanelProps } from "./types";
import "./bag.css";

const DRAG = "text/plain";

export function BagPanel(p: BagPanelProps) {
  const [selected, setSelected] = useState<SlotRef | null>(null);
  const [confirm, setConfirm] = useState<SlotRef | null>(null);
  const locked = (at: SlotRef) => p.locks.has(slotKey(at));
  const drop = (to: SlotRef) => (e: DragEvent) => {
    e.preventDefault();
    const what = e.dataTransfer.getData(DRAG);
    if (what.startsWith("cache:")) p.onPlace(Number(what.slice(6)), to);
    else if (what.startsWith("slot:")) {
      const [section, index] = what.slice(5).split(":");
      p.onMove({ section: section === "r" ? "resources" : "consumables", index: Number(index) }, to);
    }
  };
  const clickSlot = (at: SlotRef) => {
    if (!p.editable) return;
    if (p.picked !== null) {
      p.onPlace(p.picked, at);
      p.onPick(null);
    } else {
      setSelected(selected && selected.section === at.section && selected.index === at.index ? null : at);
    }
  };
  const slot = (r: Reward, at: SlotRef) => (
    <button
      key={slotKey(at)}
      type="button"
      className={`bag-slot${r.qty > 0 ? " full" : ""}${locked(at) ? " locked" : ""}${selected?.section === at.section && selected.index === at.index ? " selected" : ""}`}
      aria-label={`${at.section} slot ${at.index + 1}${r.qty > 0 ? `: ${p.label(r)}` : ""}${locked(at) ? " (locked)" : ""}`}
      title={r.qty > 0 ? p.label(r) : undefined}
      draggable={p.editable && r.qty > 0}
      onDragStart={(e) => e.dataTransfer.setData(DRAG, `slot:${slotKey(at)}`)}
      onDragOver={(e) => p.editable && e.preventDefault()}
      onDrop={drop(at)}
      onClick={() => clickSlot(at)}
    >
      {r.qty > 0 ? <span className="mono">{p.label(r)}</span> : null}
    </button>
  );
  return (
    <div className="bag-panel">
      <div className="bag-grids">
        <div>
          <h3>Resources</h3>
          <div className="bag-grid">{p.bag.resources.map((r, index) => slot(r, { section: "resources", index }))}</div>
          <h3>Consumables</h3>
          <div className="bag-grid">{p.bag.consumables.map((r, index) => slot(r, { section: "consumables", index }))}</div>
        </div>
        {p.cache.length > 0 && (
          <div className="bag-cache">
            <h3>Cache</h3>
            <p className="muted">Drag onto a slot, Shift + click for the first free one, or click and then a slot to replace.</p>
            <ul>
              {p.cache.map((r, i) => (
                <li key={`${r.kind}-${r.id}`}>
                  <button
                    type="button"
                    className="btn"
                    aria-pressed={p.picked === i}
                    draggable={p.editable}
                    onDragStart={(e) => e.dataTransfer.setData(DRAG, `cache:${i}`)}
                    disabled={!p.editable || p.busy}
                    onClick={(e) => (e.shiftKey ? p.onTakeAll(i) : p.onPick(p.picked === i ? null : i))}
                  >
                    {p.label(r)}
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
      {p.editable && selected && (
        <div className="forms-row">
          <button type="button" className="btn" onClick={() => p.onSplit(selected)}>
            Split
          </button>
          <button type="button" className="btn" onClick={() => p.onToggleLock(selected)}>
            {locked(selected) ? "Unlock" : "Lock"}
          </button>
          <button type="button" className="btn" disabled={locked(selected)} onClick={() => setConfirm(selected)}>
            Throw away
          </button>
        </div>
      )}
      {confirm && (
        <div className="forms-confirm" role="alertdialog" aria-label="Confirm throw away">
          <p>Throw this slot away? It burns.</p>
          <div className="forms-row">
            <button
              type="button"
              className="btn primary"
              onClick={() => {
                p.onDiscard(confirm);
                setConfirm(null);
                setSelected(null);
              }}
            >
              Throw away
            </button>
            <button type="button" className="btn" onClick={() => setConfirm(null)}>
              Cancel
            </button>
          </div>
        </div>
      )}
      {p.error && <p className="error">{p.error}</p>}
      {p.editable && (
        <div className="forms-row">
          <button type="button" className="btn" disabled={p.busy} onClick={p.onReset}>
            Start over
          </button>
          <button type="button" className="btn primary" disabled={p.busy || p.error !== null} onClick={p.onDone}>
            Done
          </button>
          <span className="muted">What stays in the cache burns with Done.</span>
        </div>
      )}
    </div>
  );
}
