import { Link } from "react-router-dom";
import { DEV_TOOLS } from "../config";
import { RasterizerStatus } from "../raster/RasterizerStatus";
import { useRasterizer, useRenderImage } from "../raster/RasterizerProvider";
import { SpriteCanvas } from "../raster/SpriteCanvas";

export function Home() {
  const state = useRasterizer();
  const sample = state.status === "ready" ? state.sample : null;
  const alive = useRenderImage(sample);
  const dead = useRenderImage(sample ? { ...sample, flags: 1 } : null);
  return (
    <section>
      <h1>Deadroll</h1>
      <p className="muted">Every dice roll is public. Every death leaves a mark.</p>
      <RasterizerStatus />
      {sample && (
        <div style={{ display: "flex", gap: 24, marginTop: 16, alignItems: "end" }}>
          <figure>
            <SpriteCanvas image={alive.image} scale={3} label="reference warrior" />
            <figcaption className="muted">Human warrior</figcaption>
          </figure>
          <figure>
            <SpriteCanvas image={dead.image} scale={3} label="reference warrior, dead" />
            <figcaption className="muted">GRAY flag (dead)</figcaption>
          </figure>
          {(alive.error || dead.error) && <p className="error">{alive.error ?? dead.error}</p>}
        </div>
      )}
      {DEV_TOOLS && (
        <p className="muted" style={{ marginTop: 24 }}>
          Component demos on mock data: <Link to="/dev/grid">grid</Link> · <Link to="/dev/dice">dice</Link> ·{" "}
          <Link to="/dev/forms">forms</Link>
        </p>
      )}
    </section>
  );
}
