// Ash phoenix, 96, facing left, hovering: a charred ash body (K) with smoldering seams, wings
// spread wide whose flight feathers burn (H fire), a crest of flame, long burning tail plumes
// trailing down, a burning eye (Y), a dark gold beak (R). Plume tips reach about y = 90.
const fwing = require("./feather_wing");
module.exports = {
  id: 450, name: "phoenix", size: 96,
  comment: [
    "Ash phoenix, facing left, hovering: a charred body with smoldering seams, wings spread wide",
    "with burning flight feathers, a crest of flame, long burning tail plumes trailing down.",
  ],
  build(s) {
    s.curve([0.62, 0.34, -0.04]);
    // Tail plumes trailing down, curling at the tips.
    // Flame tongues: thin wavy plumes, each swaying side to side as it falls.
    for (const [x, y, t, w] of [[72, 88, -1, 1], [62, 93, 0, -1], [50, 93, 0, 1], [38, 86, 0, -1], [70, 76, -1, 1]]) {
      const pts = [];
      for (let k = 0; k <= 6; k++) { const f = k / 6; pts.push([52 + (x - 52) * f + Math.sin(f * Math.PI * 2) * 3 * w, 60 + (y - 60) * f, 2 * (1 - f) + 0.6]); }
      s.cap(pts, "H", { tone: t });
    }
    // The tail darkens toward the body and burns brighter toward the tips.
    s.recolor((x, y, c, p) => (p.slot === "H" && y > 62 ? Math.max(1, Math.min(5, 1 + Math.floor((y - 62) / 7))) : c));
    // Far wing, wide.
    fwing(s, [54, 46], [70, 34], [90, 30], 12, "K", -1, { back: [0.2, 1] });
    // Body: a charred chest.
    s.mass([["e", 48, 52, 8, 10], ["e", 52, 60, 5, 5]], "K", { k: 2, relief: 1.8 });
    // Near wing, wide and up.
    fwing(s, [44, 46], [28, 32], [6, 26], 13, "K", 0, { back: [0.2, 1] });
    // Head: a small fierce head, the beak, a crest of flame.
    s.mass([["e", 40, 38, 5.6, 5]], "K", { k: 1.2, seam: true });
    s.poly([[35, 36], [30, 37], [28, 40], [30, 41], [34, 41]], "R", (x, y) => (y < 39 ? 4 : 2));
    for (const [dx, h] of [[0, 9], [3, 11], [6, 8], [8, 5]]) s.poly([[40 + dx, 34], [42 + dx + h * 0.4, 34 - h], [43 + dx, 35]], "H", (x, y) => (y < 30 ? 5 : 3));
    s.seams();

    // Smoldering seams on the charred body and wing arms.
    for (const [a, b] of [[[44, 46], [48, 56]], [[50, 48], [52, 58]], [[46, 58], [52, 62]]]) s.line(a[0], a[1], b[0], b[1], "Y", 3);
    // Burning tips: the trailing edges of the charred wings catch fire (two pixels deep), and the
    // flame tongues glow hottest at their edges.
    const burn = [];
    for (let y = 2; y < 94; y++) for (let x = 2; x < 94; x++) {
      const p = s.at(x, y);
      if (!p || (p.slot !== "K" && p.slot !== "H") || y < 30 || (x > 34 && x < 58 && y < 66)) continue;
      if (!s.at(x, y + 1)) burn.push([x, y, 4]);
      else if (!s.at(x, y + 2)) burn.push([x, y, 3]);
    }
    for (const [x, y, c] of burn) s.px(x, y, "H", c);
    // Eye and embers drifting up.
    s.px(37, 37, "Y", 5); s.px(38, 37, "Y", 4);
    for (const [x, y] of [[20, 18], [34, 14], [72, 20], [86, 24], [12, 40], [60, 92]]) s.px(x, y, "H", 4);
  },
};
