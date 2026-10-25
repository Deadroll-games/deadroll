// Forest tiles (ids 540..574): walls of old trunks under a dark canopy, overgrown path slabs, grass
// with dead leaves, swamp water, a gate of poles, earthen stairs. Materials: forest_wall (P bark,
// S dark leaves), forest_floor, forest_grass, forest_door (tools/art TILE_LOOKS).
const T = require("./tiles_lib");
const biome = require("./biome");

const style = {
  capH: 20,
  // The top: the canopy seen from above, dark leaf clumps (S) with gaps of shadow.
  cap(s, r, x0, y0, x1, y1) {
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) s.px(x, y, "S", 1, false);
    const n = Math.round(((x1 - x0) * (y1 - y0)) / 160);
    for (let i = 0; i < n; i++) {
      const cx = x0 + r() * (x1 - x0), cy = y0 + r() * Math.max(1, y1 - y0), w = 6 + r() * 6, h = 4 + r() * 4;
      s.ell(cx, cy, w, Math.min(h, (y1 - y0) / 2 + 2), "S", { ragged: 0.4, tufts: 10 });
    }
    T.scatter(s, x0, y0, x1, y1, r, 14, "S", 4, "S");
  },
  // The face: old trunks side by side (P bark, vertical grain), roots spreading at the foot.
  face(s, r, x0, y0, x1, y1) {
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) s.px(x, y, "O", 0, false);
    let x = x0 - Math.floor(r() * 6);
    while (x < x1) {
      const w = 9 + Math.floor(r() * 8), cx = x + w / 2;
      for (let y = y0; y < y1 - 2; y++) for (let px = Math.max(x0, x + 1); px < Math.min(x1, x + w - 1); px++) {
        const d = (px - cx) / (w / 2);
        s.px(px, y, "P", d < -0.4 ? 3 : d < 0.3 ? 2 : 1, false);
      }
      for (let k = 0; k < 3; k++) { const gx = Math.floor(x + 2 + r() * (w - 4)), gy = y0 + Math.floor(r() * (y1 - y0 - 10)); s.line(gx, gy, gx, gy + 5 + Math.floor(r() * 6), "P", 0, false); }
      // Roots at the foot.
      s.line(Math.floor(cx), y1 - 6, Math.floor(cx - 4), y1 - 1, "P", 2, false);
      s.line(Math.floor(cx), y1 - 6, Math.floor(cx + 4), y1 - 1, "P", 1, false);
      x += w;
    }
    for (let y = y1 - 2; y < y1; y++) for (let px = x0; px < x1; px++) if (!s.at(px, y) || s.at(px, y).slot === "O") s.px(px, y, "O", 0, false);
    T.scatter(s, x0, y0, x1, y1 - 2, r, 12, "S", () => 2 + Math.floor(r() * 2), "P");
  },
  floorRowH: [14, 22], floorW: [16, 28],
  // Overgrown path slabs: grass (S) creeping over the joints and edges.
  floorDetail(s, r, v) {
    T.scatter(s, 0, 0, 64, 64, r, 40 + v * 20, "S", () => 2 + Math.floor(r() * 3), "S");
    if (v === 2) T.scatter(s, 0, 0, 64, 64, r, 18, "S", 3, "P");
  },
};

// Grass with dead leaves (A), mushrooms, a mossy stone; blades are short light strokes.
const grass = (seed, v) => (s) => {
  const r = T.rnd(seed);
  for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) s.px(x, y, "S", ((x * 5 + y * 11) % 7) === 0 ? 1 : 2, false);
  for (let i = 0; i < 90; i++) {
    const x = Math.floor(r() * 64), y = 2 + Math.floor(r() * 62);
    s.px(x, y, "S", 3, false); s.px(x, y - 1, "S", 4, false); if (r() < 0.5) s.px(x + 1, y - 2, "S", 3, false);
  }
  for (let i = 0; i < 6 + v * 4; i++) { const x = Math.floor(r() * 62), y = Math.floor(r() * 62); s.px(x, y, "A", 3, false); s.px(x + 1, y, "A", 2, false); }
  if (v === 1) for (const [x, y] of [[20, 40], [24, 42], [44, 22]]) { s.ell(x, y, 2.4, 1.4, "E", {}); s.line(x, y + 1, x, y + 3, "P", 4); }
  if (v === 2) { s.poly([[34, 30], [46, 28], [50, 36], [40, 40], [32, 36]], "P", T.facet(41, 34, 9, 6, 0)); T.scatter(s, 32, 28, 50, 34, r, 8, "S", 3, "P"); }
};

// Gate: the canopy over a gap between two trunks closed by a gate of lashed poles.
const door = (s) => {
  const r = T.rnd(566);
  style.cap(s, r, 0, 0, 64, 20);
  for (let x = 0; x < 64; x++) s.px(x, 19, "S", 4, false);
  style.face(s, r, 0, 20, 64, 64);
  for (let y = 22; y < 62; y++) for (let x = 14; x < 50; x++) s.px(x, y, "O", 0, false);
  for (let x = 15; x < 49; x += 5) for (let y = 24; y < 61; y++) { s.px(x, y, "P", 3, false); s.px(x + 1, y, "P", 2, false); s.px(x + 2, y, "P", 1, false); }
  for (const y of [30, 50]) for (let x = 14; x < 50; x++) { s.px(x, y, "P", 4, false); s.px(x, y + 1, "P", 1, false); }
  s.line(15, 50, 48, 31, "P", 3);
  for (const [x, y] of [[16, 30], [46, 30], [16, 50], [46, 50]]) s.px(x, y, "A", 4, false);
};

module.exports = biome({ base: 540, prefix: "forest", title: "Forest", style, door, ground: grass });
