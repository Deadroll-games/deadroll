// Error and transaction toasts (L9.6). One provider for the app; screens call `useToast()`.
import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";
import { explorerUrl } from "../../config";
import "./toast.css";

export type Toast = { id: number; kind: "ok" | "error" | "info"; text: string; signature?: string };

type ToastApi = {
  push: (t: Omit<Toast, "id">) => void;
  error: (e: unknown, what?: string) => void;
};

const Ctx = createContext<ToastApi | null>(null);
let nextId = 1;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const dismiss = useCallback((id: number) => setToasts((ts) => ts.filter((t) => t.id !== id)), []);
  const push = useCallback(
    (t: Omit<Toast, "id">) => {
      const id = nextId++;
      setToasts((ts) => [...ts.slice(-4), { ...t, id }]);
      setTimeout(() => dismiss(id), t.kind === "error" ? 12_000 : 5_000);
    },
    [dismiss],
  );
  const error = useCallback(
    (e: unknown, what?: string) => push({ kind: "error", text: `${what ? `${what}: ` : ""}${e instanceof Error ? e.message : String(e)}` }),
    [push],
  );
  const api = useMemo(() => ({ push, error }), [push, error]);
  return (
    <Ctx.Provider value={api}>
      {children}
      <div className="toasts" role="status" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className={`toast ${t.kind}`} onClick={() => dismiss(t.id)}>
            <span>{t.text}</span>
            {t.signature && (
              <a href={explorerUrl("tx", t.signature)} target="_blank" rel="noreferrer" onClick={(e) => e.stopPropagation()}>
                tx
              </a>
            )}
          </div>
        ))}
      </div>
    </Ctx.Provider>
  );
}

export function useToast(): ToastApi {
  const v = useContext(Ctx);
  if (!v) throw new Error("useToast outside ToastProvider");
  return v;
}
