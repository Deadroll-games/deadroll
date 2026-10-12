import { useRasterizer } from "./RasterizerProvider";

export function RasterizerStatus() {
  const s = useRasterizer();
  if (s.status === "loading") return <p className="muted">Loading rasterizer from chain...</p>;
  if (s.status === "error") return <p className="error">Rasterizer not loaded: {s.error}</p>;
  const r = s.render;
  return (
    <div className="panel">
      <p className="ok">
        Rasterizer v{r.config.rasterizerVersion} verified{r.fromCache ? " (IndexedDB cache, hash re-checked)" : ""}
        {r.source === "mock" ? " from local mock accounts" : " from chain"}
      </p>
      <p className="mono">sha256 {r.sha256Hex}</p>
      <p className="mono">account {r.config.rasterizer}</p>
      <p className="muted">
        Palette v{r.paletteVersion}, sprite books {r.books.map((b) => `#${b.id} (${b.body.length} B)`).join(", ")}
      </p>
    </div>
  );
}
