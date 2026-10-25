// Stone golem, 2x2 on 128, facing left: a hulk of chipped boulders (K stone), moss in the cracks
// (H), a rune heart glowing in the chest and glowing seams (Y), huge fists, short block legs.
// Feet on y = 123.
const rock = require("./rock");
module.exports = {
  id: 438, name: "golem", size: 128,
  comment: [
    "Stone golem, a 2x2 brute, facing left: chipped boulders bound by glowing seams, a rune heart",
    "in the chest, moss in the cracks, huge fists, short block legs. Feet on y = 123.",
  ],
  build(s) {
    s.curve([0.62, 0.34, -0.04]);
    const R = (x, y, w, h, seed, tone = 0, sides = 6) => rock(s, x, y, w, h, "K", { seed, tone, sides });
    // Far arm and far leg (darker).
    R(82, 50, 9, 9, 3, -1); R(90, 66, 7, 10, 5, -1); R(92, 84, 6, 8, 6, -1); R(93, 99, 8, 8, 7, -1, 7);
    R(74, 100, 8, 10, 9, -1); R(75, 113, 7, 6, 8, -1); R(75, 120, 10, 3, 2, -1, 5);
    // Torso: a broad chest wedge, the pelvis stone.
    s.poly([[42, 46], [80, 42], [88, 60], [78, 82], [50, 84], [40, 64]], "K", (x, y) => {
      const v = -(x - 64) / 24 - (y - 62) / 20;
      return v > 0.55 ? 4 : v > 0.1 ? 3 : v > -0.45 ? 2 : 1;
    });
    R(62, 90, 14, 7, 13, 0, 7);
    // Near leg: thigh, shin, a slab of a foot.
    R(55, 100, 9, 10, 15); R(53, 112, 8, 6, 14); R(51, 120, 12, 3, 4, 0, 5);
    // Head: a small block sunk at the front of the shoulders, a brow ledge.
    R(46, 38, 8, 8, 17);
    // Near arm: shoulder boulder, upper arm, forearm, a huge fist near the knee.
    R(42, 50, 11, 10, 19, 0, 7); R(36, 67, 7, 9, 20); R(32, 82, 7, 9, 21); R(29, 98, 11, 10, 23, 0, 7);
    // Dark gaps between the stones.
    for (let y = 1; y < 127; y++) for (let x = 1; x < 127; x++) {
      const p = s.at(x, y);
      if (!p || p.slot !== "K") continue;
      const r = s.at(x + 1, y), b = s.at(x, y + 1);
      if ((r && r.slot === "K" && r.sid !== p.sid) || (b && b.slot === "K" && b.sid !== p.sid)) p.c = 0;
    }
    // Glowing seams along some of the gaps, the rune heart in the chest.
    const seam = (pts) => {
      for (let i = 0; i + 1 < pts.length; i++) {
        const [a, b] = [pts[i], pts[i + 1]];
        const n = Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
        for (let k = 0; k <= n; k++) {
          const x = Math.round(a[0] + ((b[0] - a[0]) * k) / n), y = Math.round(a[1] + ((b[1] - a[1]) * k) / n);
          if (s.at(x, y)?.slot === "K") s.px(x, y, "Y", k % 4 === 0 ? 4 : 2);
        }
      }
    };
    seam([[56, 50], [62, 56], [60, 66], [66, 72]]);
    seam([[62, 56], [72, 52]]);
    seam([[40, 84], [36, 90]]);
    s.patch(56, 56, [
      "..jj..",
      ".jkkj.",
      "jklkkj",
      "jkllkj",
      ".jkkj.",
      "..jj..",
    ]);
    // Magic binding the floating stones: glowing knots in the joints.
    for (const [x, y] of [[39, 59], [34, 75], [31, 90], [86, 58], [91, 76], [92, 92], [56, 95], [54, 107], [73, 95], [74, 108]]) {
      if (!s.at(x, y) || s.at(x, y).c === 0 || s.at(x, y).slot !== "K") { s.px(x, y, "Y", 4); s.px(x, y - 1, "Y", 2); s.px(x, y + 1, "Y", 2); }
      else { s.px(x, y, "Y", 3); }
    }
    // Chips on the chest wedge.
    for (const [a, b] of [[[48, 50], [54, 56]], [[74, 48], [80, 56]], [[52, 74], [60, 78]]]) s.line(a[0], a[1], b[0], b[1], "K", 1);
    // Eyes: two glowing slits under the brow ledge.
    s.line(40, 38, 44, 38, "Y", 4); s.px(41, 38, "Y", 5);
    s.line(47, 38, 49, 38, "Y", 3);
    s.line(38, 35, 50, 34, "K", 4);
    // Moss in the cracks and on the shoulders.
    for (const [x, y, l] of [[48, 46, 5], [70, 42, 7], [58, 78, 4], [30, 88, 5], [54, 96, 4], [80, 66, 4]]) {
      for (let i = 0; i < l; i++) if (s.at(x + i, y)?.slot === "K") s.px(x + i, y, "H", i % 2 ? 2 : 3);
      if (s.at(x + 1, y + 1)?.slot === "K") s.px(x + 1, y + 1, "H", 1);
    }
    // Knuckles on the fist.
    for (const [x, y] of [[22, 100], [24, 104], [27, 107]]) s.line(x, y, x + 3, y - 1, "K", 0);
  },
};
