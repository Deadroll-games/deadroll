// Wood fairy, 96, facing left, hovering: a thin eerie figure, torn insect wings (element ramp R),
// wild hair and a dress of withered leaves (H), an orb of magic (R) in the outstretched hand.
// The hem is the lowest point at y = 86; sparks fall below.
const insectWing = require("./wing");
module.exports = {
  id: 408, name: "wood_fairy", size: 96,
  comment: [
    "Wood fairy, facing left, hovering: thin and eerie, torn insect wings and the orb in her hand",
    "take the element ramp (R), wild hair and a dress of withered leaves, glowing eyes, a cruel smile.",
  ],
  build(s) {
    s.curve([0.6, 0.3, -0.06]);
    // Wings behind the back: far pair darker, then the near pair.
    insectWing(s, [[50, 45], [54, 30], [64, 16], [74, 10], [78, 16], [72, 32], [60, 46]], -1, [[56, 34], [66, 22]]);
    insectWing(s, [[50, 50], [62, 52], [72, 60], [72, 68], [64, 68], [55, 58]], -1, [[62, 60]]);
    insectWing(s, [[48, 45], [48, 28], [53, 12], [59, 5], [64, 10], [61, 28], [53, 46]], 0, [[54, 20], [57, 34]]);
    insectWing(s, [[48, 50], [56, 57], [61, 68], [57, 75], [51, 68], [47, 55]], 0, [[54, 64]]);
    // Back arm hanging behind the dress.
    s.cap([[48, 48, 1.6], [52, 56, 1.4], [53, 63, 1.2]], "K", { tone: -1 });
    // Torso and the slender neck.
    s.cap([[46, 47, 3.2], [47, 56, 2.4]], "K");
    s.cap([[44, 42, 1.5], [45, 46, 1.7]], "K");
    // Dress of withered leaves: narrow, a hem of pointed leaves.
    s.poly([[45, 49], [49, 49], [49, 57], [54, 70], [55, 79], [53, 76], [52, 84], [50, 78], [48, 86], [46, 78],
      [44, 84], [43, 77], [40, 81], [41, 70], [45, 57]], "H", (x, y) => (x < 45 ? 3 : x > 51 ? 1 : 2));
    s.line(44, 57, 50, 57, "H", 1); // the waist
    // Head in profile: skull, brow, nose, chin; a pointed ear.
    s.ell(42, 37, 5, 5.6, "K");
    s.cap([[39, 41, 2.6], [38, 43, 1.6]], "K");
    s.poly([[45, 35], [51, 29], [48, 37]], "K", 3);
    // Wild hair from the crown flowing back, the face left open.
    for (const [tx, ty, w] of [[60, 34, 2.4], [63, 42, 2.6], [61, 50, 2.4], [56, 53, 2], [52, 47, 1.8]])
      s.cap([[45, 33, 3.6], [(45 + tx) / 2 + 1, (33 + ty) / 2 - 1, w], [tx, ty, 0.6]], "H");
    // Near arm stretched forward, the hand under the orb.
    s.cap([[45, 49, 1.8], [39, 53, 1.5], [33, 51, 1.4]], "K");
    s.ell(31, 51, 1.8, 1.6, "K");
    s.ell(27, 48, 4.2, 4.2, "R", {}); // the orb
    s.seams();

    // Orb core and crackle.
    s.px(26, 47, "R", 5); s.px(27, 47, "R", 5); s.px(26, 46, "R", 4);
    for (const [x, y] of [[22, 44], [21, 49], [31, 43], [24, 53], [29, 54]]) s.px(x, y, "R", 4);
    // Leaf veins on the dress.
    for (const [x, y] of [[44, 64], [48, 70], [51, 64], [46, 76]]) s.line(x, y, x + 2, y - 3, "H", 1);
    // The head, pixel by pixel: profile to the left, hair swept back, a glowing eye under a dark
    // brow, a sharp nose, a hollow cheek, thin lips, a pointed chin.
    s.patch(36, 30, [
      "....cdddd...",
      "...ccddddd..",
      "..344cdddddd",
      "..4554cddddd",
      ".34bb44cdddd",
      ".3kl344cdddd",
      "..444433cddd",
      ".4444433cdd.",
      "4544433cdd..",
      ".444433c....",
      "..3n4433....",
      "..444333....",
      "...44433....",
      "....3333....",
    ]);
    // The slender neck, lit, without seams.
    s.recolor((x, y, c, p) => (p.slot === "K" && x >= 43 && x <= 47 && y >= 43 && y <= 48 ? Math.max(c, 3) : c));
    // Lighter strands along the hair.
    for (const [a, b] of [[[46, 32], [58, 35]], [[47, 34], [60, 42]], [[46, 36], [57, 48]]]) s.line(a[0], a[1], b[0], b[1], "H", 3, false);
    // Thin legs dangling under the hem, pointed toes.
    s.line(46, 84, 45, 88, "K", 2); s.line(49, 86, 49, 89, "K", 1);
    // Sparks falling from the orb and the hem.
    for (const [x, y] of [[26, 58], [29, 63], [24, 66], [44, 92], [50, 93]]) s.px(x, y, "R", 3);
  },
};
