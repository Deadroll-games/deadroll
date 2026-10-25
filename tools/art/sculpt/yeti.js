// Yeti, 2x2 on 128, facing left: a shaggy ape-beast with matted dirty-white fur (K), a dark bare
// face (H dark), a fanged roar (R bone), cold glowing eyes (Y), huge arms on the knuckles.
// Feet on y = 123.
module.exports = {
  id: 439, name: "yeti", size: 128,
  comment: [
    "Yeti, a 2x2 beast, facing left: a shaggy ape-beast in matted dirty-white fur, a dark bare face",
    "roaring with fangs, cold eyes, huge arms on the knuckles, short legs. Feet on y = 123.",
  ],
  build(s) {
    s.curve([0.66, 0.4, 0.02]);
    // Far arm on its knuckles and the far leg (darker).
    s.mass([["c", [[78, 54, 10], [88, 76, 8], [88, 104, 7]]], ["e", 87, 112, 8, 7, { ragged: 0.3, tufts: 14 }]], "K", { tone: -1, seam: true });
    s.mass([["c", [[72, 96, 9], [74, 110, 8], [74, 118, 7.5]]], ["e", 71, 121, 10, 3]], "K", { tone: -1, seam: true });
    // Body: a great hunched back and shoulders, the belly; shaggy edges.
    s.mass([
      ["e", 64, 62, 24, 22, { ragged: 0.22, tufts: 44 }],
      ["e", 62, 88, 18, 14, { ragged: 0.3, tufts: 30 }],
    ], "K", { k: 3, relief: 1.8 });
    // Near leg: short and thick.
    s.mass([["c", [[54, 98, 10], [52, 110, 9], [51, 118, 8.5]]], ["e", 47, 121, 11, 3]], "K", { seam: true });
    // Head low and forward, the dark face.
    s.mass([["e", 40, 54, 13, 13, { ragged: 0.3, tufts: 22 }]], "K", { k: 1.6, seam: true });
    s.mass([["e", 34, 58, 7, 7.5]], "H", { k: 1.2, tone: 1 });
    // Near arm: a huge arm, the fist on the knuckles ahead.
    s.mass([["c", [[50, 62, 10], [40, 80, 8]]], ["c", [[40, 80, 9.5], [34, 104, 8]]], ["e", 32, 112, 9, 8, { ragged: 0.25, tufts: 16 }], ["e", 41, 72, 9, 11, { ragged: 0.35, tufts: 14 }]], "K", { seam: true });
    s.seams();

    // Roar: the open mouth with fangs, nostrils, the brow and cold eyes.
    s.patch(28, 59, [
      ".xwxwx.",
      "mmmmmmm",
      "mmmmmmm",
      "mnnnnm.",
      ".wxwx..",
    ]);
    s.px(30, 56, "D", 1); s.px(32, 56, "D", 1);
    s.line(29, 52, 38, 51, "H", 0);
    s.px(31, 53, "Y", 4); s.px(32, 53, "Y", 5); s.px(36, 53, "Y", 3);
    // Shaggy fur: locks hanging from every lower edge, tufts up along the back.
    const hash = (x, y) => (((x * 73856093) ^ (y * 19349663)) >>> 0) % 7;
    const edges = [];
    for (let x = 2; x < 126; x++) for (let y = 30; y < 114; y++) {
      const p = s.at(x, y), b = s.at(x, y + 1), t = s.at(x, y - 1);
      if (p?.slot === "K" && !b) edges.push([x, y, 1]);
      if (p?.slot === "K" && !t && x > 50) edges.push([x, y, -1]);
    }
    // Left edges (toward the viewer side of the arms): short locks pointing down and out.
    for (let y = 40; y < 112; y++) for (let x = 2; x < 126; x++) {
      const p = s.at(x, y);
      if (p?.slot === "K" && !s.at(x - 1, y) && hash(x, y) < 3) {
        s.px(x - 1, y, "K", 2, false); if (hash(y, x) < 2 && !s.at(x - 2, y + 1)) s.px(x - 2, y + 1, "K", 1, false);
      }
    }
    for (const [x, y, d] of edges) {
      const l = hash(x, y) % (d > 0 ? 4 : 3);
      for (let i = 1; i <= l; i++) if (!s.at(x, y + d * i)) s.px(x, y + d * i, "K", d > 0 ? (i === l ? 1 : 2) : (i === l ? 3 : 4), false);
    }
    // Clumps inside the fur: sparse short strokes, a lit top pixel over a shadowed tail.
    for (let y = 44; y < 114; y += 3) for (let x = 24; x < 104; x += 3) {
      const h = (((x * 2654435761) ^ (y * 40503)) >>> 0) % 5;
      if (h !== 0) continue;
      const ox = x + (y % 2), p0 = s.at(ox, y), p1 = s.at(ox, y + 1), p2 = s.at(ox - 1, y + 2);
      if (p0?.slot !== "K" || p1?.slot !== "K" || p2?.slot !== "K") continue;
      p0.c = Math.min(4, p0.c + 1); p1.c = Math.max(1, p1.c - 1); p2.c = Math.max(1, p2.c - 1);
    }
    // Matted fur: hanging locks along the arms and the belly.
    const lock = (x, y, l) => { for (let i = 0; i < l; i++) s.px(x + (i > 2 ? 1 : 0), y + i, "K", i < 2 ? 4 : i < l - 1 ? 2 : 1); };
    for (const [x, y, l] of [[42, 70, 6], [46, 78, 5], [38, 88, 6], [60, 98, 5], [66, 100, 4], [74, 76, 5], [56, 74, 5], [84, 70, 6]]) lock(x, y, l);
    // Knuckles and claws on the fists, toes.
    for (const [x, y] of [[24, 116], [27, 118], [30, 119], [82, 117], [85, 118]]) s.px(x, y, "R", 3);
    for (const x of [38, 41, 44, 62, 65]) s.px(x, 123, "R", 2);
  },
};
