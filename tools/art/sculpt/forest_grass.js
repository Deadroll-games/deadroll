// Grass with dead leaves: the Forest ground, reused by the city.
const T = require("./tiles_lib");

// Grass with dead leaves (A), mushrooms, a mossy stone; blades are short light strokes.
module.exports = (seed, v) => (s) => {
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
