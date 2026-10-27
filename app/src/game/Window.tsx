// A window over the main screen (bag, skills, characters, the Tavern board...). The screen under
// it keeps running: a battle goes on updating while a window is open.
import { useEffect, type ReactNode } from "react";

export function GameWindow({ title, onClose, children, wide = false }: { title: string; onClose: () => void; children: ReactNode; wide?: boolean }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <div className="game-window-backdrop" onClick={onClose}>
      <section className={`game-window${wide ? " wide" : ""}`} role="dialog" aria-label={title} onClick={(e) => e.stopPropagation()}>
        <header className="game-window-head">
          <h2>{title}</h2>
          <button type="button" className="btn" onClick={onClose} aria-label="Close">
            ×
          </button>
        </header>
        <div className="game-window-body">{children}</div>
      </section>
    </div>
  );
}
