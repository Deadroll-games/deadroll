// Ogre, 2x2 on 128, facing left: obese, a huge sagging belly, a small bald head sunk in the
// shoulders, grimy ochre skin (K), a sackcloth kilt (H), a hammer of
// a stone block lashed to a log (R gray stone). Feet on y = 123.
module.exports = {
  id: 437, name: "ogre", size: 128,
  comment: [
    "Ogre, a 2x2 brute, facing left: obese with a sagging belly, a small bald head sunk in the",
    "shoulders, grimy skin, a sackcloth kilt, a stone-block hammer dragged. Feet on y = 123.",
  ],
  build(s) {
    s.curve([0.62, 0.34, -0.04]);
    // Far leg and far arm (darker).
    s.mass([["c", [[74, 96, 9], [76, 110, 8], [75, 118, 7.5]]], ["e", 72, 121, 10, 3]], "K", { tone: -1, seam: true });
    s.mass([["c", [[80, 52, 8], [88, 70, 6.5], [86, 86, 6]]], ["e", 85, 90, 6.5, 6]], "K", { tone: -1, seam: true });
    // Body: rounded shoulders, a huge belly sagging over the kilt.
    s.mass([
      ["e", 66, 56, 20, 15],
      ["e", 58, 82, 24, 20],
      ["c", [[70, 70, 16], [72, 92, 14]]],
    ], "K", { k: 3, relief: 2 });
    // Sackcloth kilt under the belly.
    s.poly([[46, 96], [86, 94], [88, 104], [82, 102], [78, 108], [72, 102], [64, 107], [58, 101], [50, 106], [46, 102]], "H",
      (x, y) => (x > 76 ? 1 : 2));
    // Near leg: stout, under the belly.
    s.mass([["c", [[58, 100, 10], [56, 112, 9], [55, 118, 8.5]]], ["e", 51, 121, 11.5, 3]], "K", { seam: true });
    // Head: small and bald, low between the shoulders; jowls, a heavy jaw.
    s.mass([["e", 46, 42, 9, 9], ["e", 42, 50, 8, 5]], "K", { k: 1.8, seam: true });
    s.ell(52, 40, 2.6, 3.4, "K", { tone: -1 }); // ear
    // Near arm hanging forward, the fist low.
    s.mass([["c", [[58, 54, 8], [48, 72, 6.5], [38, 86, 6]]], ["e", 35, 89, 6, 5.5]], "K", { seam: true });
    s.seams();

    // Hammer dragged: the log haft from the fist down to a stone block resting on the ground ahead.
    s.line(37, 86, 22, 108, "H", 1); s.line(38, 86, 23, 108, "H", 3); s.line(38, 87, 23, 109, "H", 2);
    s.poly([[6, 104], [28, 99], [32, 120], [9, 123]], "R", (x, y) => (y < 104 ? 4 : x < 12 ? 3 : 2));
    s.line(28, 99, 32, 120, "R", 1); s.line(9, 122, 32, 120, "R", 0);
    for (const [x0, x1] of [[16, 19], [22, 25]]) { s.line(x0, 102, x1, 122, "H", 4); s.line(x0 + 1, 102, x1 + 1, 122, "H", 1); } // rope lashing
    for (const [x, y] of [[12, 110], [26, 108], [18, 116]]) s.px(x, y, "R", 1); // chips
    // The fist on the haft.
    s.ell(35, 89, 5, 4.5, "K", {});
    // Face: a heavy brow, small mean eyes, a flat nose, an underbite with a jutting tooth.
    s.line(37, 39, 48, 38, "K", 1);
    s.px(40, 41, "Y", 4); s.px(41, 41, "Y", 5); s.px(46, 41, "Y", 3);
    s.px(37, 44, "D", 1); s.px(38, 45, "D", 1);
    s.line(36, 50, 46, 51, "D", 1);
    s.px(39, 49, "R", 4); s.px(39, 48, "R", 5);
    // Belly button, fat folds, grime blotches.
    s.px(62, 84, "D", 1); s.line(56, 96, 70, 96, "K", 1); s.line(58, 74, 68, 72, "K", 1);
    const hash = (x, y) => (((x * 73856093) ^ (y * 19349663)) >>> 0) % 13;
    s.recolor((x, y, c) => (hash(x >> 1, y >> 1) === 0 && c >= 2 ? c - 1 : c), "K");
    for (const x of [42, 45, 48, 66, 69]) s.px(x, 123, "K", 1);
  },
};
