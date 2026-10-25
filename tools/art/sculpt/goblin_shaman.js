// Goblin shaman: the goblin canon, a hooded cloak of stitched hides to the knees, a crooked staff
// topped with a small animal skull glowing green, a necklace of teeth. Feet on y = 91.
const G = require("./goblin_base");
module.exports = {
  id: 405, name: "goblin_shaman", size: 96,
  comment: [
    "Goblin shaman, facing left: hunched under a hooded cloak of stitched hides, ears out of the",
    "hood, a crooked staff with a glowing animal skull, a necklace of teeth. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.66, 0.36, -0.02]);
    // Staff behind the near hand: crooked shaft from the ground to above the head.
    G.back(s, { earDown: true });
    G.body(s, { noLoincloth: true });
    // Cloak of hides over the back and torso, ragged hem at the knees.
    s.poly([[41, 58], [47, 54], [55, 55], [60, 60], [62, 70], [61, 83], [57, 80], [55, 84], [51, 81],
      [48, 84], [45, 80], [42, 82], [41, 74], [40, 64]], "H", (x, y) => (y < 59 && x < 52 ? 3 : x < 45 ? 3 : x > 56 ? 1 : 2));
    G.head(s);
    // Hood over the skull: the face stays open at the front.
    // Close cowl on the skull: the face stays open, the tip droops to the shoulder.
    s.ell(42, 48, 9.5, 7.5, "H", { clip: (x) => x >= 35 });
    s.poly([[47, 42], [54, 45], [59, 53], [56, 55], [50, 50]], "H", 1);
    s.recolor((x, y, c, p) => (p.slot === "H" && y < 46 && x < 44 ? Math.min(3, c + 1) : c));
    s.line(35, 43, 35, 52, "H", 0); // the cowl edge over the brow and cheek
    // Near ear out of the cowl, in its usual place.
    s.poly([[42, 47], [52, 48], [64, 53], [53, 54], [44, 53]], "K", (x, y) => (y < 50 ? 4 : 3));
    s.poly([[46, 50], [53, 50], [59, 53], [52, 52]], "D", 2);
    // Near arm forward, the fist holding the staff.
    s.cap([[44, 60, 3.2], [37, 68, 2.6], [28, 67, 2.4]], "K");
    s.ell(26, 67, 3, 2.8, "K");
    s.seams();
    // Crooked staff: knotted wood from the ground to above the head.
    for (const [a, b] of [[[25, 90], [26, 77]], [[26, 77], [24, 62]], [[24, 62], [25, 46]], [[25, 46], [23, 36]]]) {
      s.line(a[0], a[1], b[0], b[1], "H", 2); s.line(a[0] + 1, a[1], b[0] + 1, b[1], "H", 3);
    }
    s.px(25, 77, "H", 4); s.px(25, 52, "H", 4); // knots
    // The fist over the shaft.
    s.ell(26, 67, 2.6, 2.6, "K", {});
    // Animal skull on top: cranium, snout forward, glowing sockets, a horn.
    s.ell(24, 32, 5, 4, "R", {});
    s.cap([[21, 34, 2.4], [16, 35, 1.8]], "R", {});
    s.poly([[26, 29], [30, 24], [28, 30]], "R", 4);
    s.px(22, 31, "Y", 5); s.px(23, 31, "Y", 4); s.px(23, 32, "Y", 3);
    s.px(17, 35, "D", 1); s.line(17, 36, 21, 36, "D", 1);
    // Bone charms dangling from the skull.
    s.line(27, 36, 27, 41, "H", 1); s.px(27, 42, "R", 4); s.px(27, 43, "R", 3);
    s.line(25, 36, 25, 39, "H", 1); s.px(25, 40, "R", 4);
    // Green glow specks rising from the skull.
    s.px(22, 26, "Y", 3); s.px(19, 28, "Y", 2);
    // Necklace of teeth under the chin.
    s.line(39, 62, 47, 62, "H", 1);
    for (const [x, y] of [[40, 63], [43, 64], [46, 63]]) s.px(x, y, "R", 4);
    // Warpaint stripes on the cheek and the arm.
    s.line(34, 53, 37, 54, "Y", 2); s.line(39, 64, 41, 66, "Y", 2);
    // Stitches on the cloak.
    for (const y of [64, 68, 72, 76]) s.px(53, y, "H", 4);
    s.line(57, 60, 58, 80, "H", 1); // a fold
    G.face(s, { noScars: true, teeth: 4 });
  },
};
