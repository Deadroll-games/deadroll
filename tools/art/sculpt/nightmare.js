// Nightmare, 96, facing left: a black horse (K) with a mane and tail of living flame (H fire),
// smoldering hooves (R fire), burning eyes (Y), embers trailing. Hooves on y = 91.
const H = require("./horse_base");
module.exports = {
  id: 452, name: "nightmare", size: 96,
  comment: [
    "Nightmare, facing left: a black horse with a mane and tail of living flame, smoldering hooves,",
    "burning eyes, embers trailing. Hooves on y = 91.",
  ],
  build(s) {
    s.curve([0.6, 0.32, -0.06]);
    // A flame tongue: a wavy tapering plume, hot at the tip.
    const flame = (x0, y0, x1, y1, w, sway) => {
      const pts = [];
      for (let k = 0; k <= 5; k++) { const f = k / 5; pts.push([x0 + (x1 - x0) * f + Math.sin(f * Math.PI * 2) * sway, y0 + (y1 - y0) * f, w * (1 - f) + 0.5]); }
      s.cap(pts, "H", {});
    };
    // Tail of flame streaming back and up.
    for (const [x, y, sw] of [[92, 44, 2], [90, 56, -2], [88, 36, 2]]) flame(78, 52, x, y, 2.6, sw);
    H.far(s);
    H.body(s);
    // Mane of flame streaming back from the neck and the poll.
    for (let i = 0; i < 7; i++) {
      const x = 31 + i * 2.2, y = 33 + i * 3;
      flame(x, y, x + 10 + (i % 2) * 3, y - 8 - (i % 3) * 2, 2.2, i % 2 ? 1.5 : -1.5);
    }
    s.seams();
    H.face(s);
    // The flames burn darkest at the roots and white-hot at the tips.
    s.recolor((x, y, c, p) => {
      if (p.slot !== "H") return c;
      const open = (!s.at(x, y - 1)) + (!s.at(x + 1, y)) + (!s.at(x - 1, y));
      return open >= 2 ? 5 : open === 1 ? 4 : Math.max(2, c);
    });
    // Hooves smolder; embers trail behind them.
    s.recolor((x, y, c, p) => (p.slot === "R" ? (y < 89 ? 4 : 2) : c));
    for (const [x, y] of [[44, 92], [50, 93], [76, 92], [82, 93], [60, 30], [70, 26]]) s.px(x, y, "H", 4);
    // Burning eyes instead of the plain eye.
    s.px(24, 33, "Y", 5); s.px(23, 33, "Y", 4); s.px(25, 32, "Y", 3);
  },
};
