// Reaper, 96, facing left, hovering: a tall tattered hooded cloak, a skull in the shadow of the
// hood with a cold glow, bony hands on a long scythe (dark snath in D, the blade in R). The ragged
// hem trails to the right; the lowest tatter at y = 89.
const B = require("./skeleton_base");
module.exports = {
  id: 425, name: "reaper", size: 96,
  comment: [
    "Reaper, facing left, hovering: a tall tattered hooded cloak, a skull in the hood's shadow",
    "with a cold glow, bony hands on a long scythe with a notched blade. Lowest tatter at y = 89.",
  ],
  build(s) {
    s.curve([0.6, 0.3, -0.06]);
    // Cloak: hood peak, shoulders, a long body flaring to tatters trailing back.
    s.poly([[44, 13], [52, 15], [57, 24], [58, 36], [62, 52], [66, 68], [70, 82], [64, 79], [62, 87], [57, 80],
      [54, 89], [50, 81], [46, 87], [43, 79], [39, 85], [37, 74], [38, 56], [37, 40], [38, 26], [40, 18]], "H",
      (x, y) => (x < 42 ? 3 : x > 58 ? 1 : 2));
    // Fold lines.
    for (const [a, b] of [[[47, 40], [45, 80]], [[53, 42], [55, 78]], [[58, 50], [64, 76]]]) s.line(a[0], a[1], b[0], b[1], "H", 1, false);
    // Hood opening: a deep shadow with the skull inside.
    s.ell(41, 28, 6, 7.5, "D", { tone: -3 });
    s.ell(40, 29, 4.2, 5, "K", {});
    s.patch(37, 27, [
      "mmm.",
      "nlkm",
      "mmm.",
      ".4m4",
      "4m4m",
    ]);
    s.line(37, 22, 44, 21, "H", 3); // the lit rim of the hood
    // Near arm: a sleeve to the snath.
    s.poly([[39, 38], [45, 40], [40, 52], [35, 56], [34, 51]], "H", (x, y) => (y < 45 ? 3 : 2));
    // Snath in front of the cloak, left of the face: from the blade down through the fist.
    s.line(30, 13, 33, 88, "D", 3, false); s.line(31, 13, 34, 88, "D", 1, false);
    s.ell(33, 55, 2.4, 2, "K"); s.px(31, 55, "K", 4); s.px(32, 57, "K", 3);
    // Scythe blade: from the top of the snath curving forward and down, a notched edge.
    s.poly([[32, 12], [26, 9], [18, 10], [10, 14], [5, 21], [11, 18], [18, 15], [25, 14], [31, 15]], "R",
      (x, y) => (y < 12 ? 4 : 2));
    s.line(5, 21, 11, 18, "R", 5); s.line(11, 18, 18, 15, "R", 5);
    s.erase(15, 16); s.erase(22, 15);
    s.ell(31, 13, 1.8, 1.8, "R", { tone: -1 }); // the tang ring
    // Wisps under the hem.
    for (const [x, y] of [[48, 92], [56, 91], [44, 93]]) s.px(x, y, "H", 2);
  },
};
