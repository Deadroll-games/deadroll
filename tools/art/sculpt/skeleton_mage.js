// Skeleton mage, 96, facing left: the skeleton in a tattered dark robe to the ankles, a rope belt,
// bony feet under the hem, a raised bony hand with a flame over the palm (element ramp R); the
// sockets burn with the same fire (Y = fire). Feet on y = 91.
const B = require("./skeleton_base");
module.exports = {
  id: 422, name: "skeleton_mage", size: 96,
  comment: [
    "Skeleton mage, facing left: a tattered dark robe with a rope belt, bony feet under the hem,",
    "a raised bony hand with a flame (element ramp), burning sockets. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.6, 0.3, -0.06]);
    // Bony feet under the hem.
    s.poly([[39, 88], [45, 88], [45, 91], [38, 91]], "K", (x) => (x < 41 ? 4 : 3));
    s.poly([[48, 88], [53, 88], [53, 91], [47, 91]], "K", 1);
    // Robe: shoulders to a ragged hem, a hood fallen on the back.
    s.poly([[39, 34], [52, 33], [55, 44], [57, 62], [59, 80], [57, 87], [54, 84], [52, 89], [49, 85], [46, 89],
      [43, 85], [40, 88], [37, 84], [37, 70], [39, 52]], "H", (x, y) => (x < 42 ? 3 : x > 53 ? 1 : 2));
    s.ell(53, 34, 5, 4, "H", { tone: -1 });
    // Rope belt and folds.
    s.line(39, 56, 55, 55, "H", 4, false); s.line(39, 57, 55, 56, "H", 1, false);
    s.line(46, 57, 45, 66, "H", 4, false); s.line(47, 57, 47, 64, "H", 4, false);
    for (const x of [44, 49, 53]) s.line(x, 62, x + 1, 84, "H", 1, false);
    // Near arm: a wide sleeve to the elbow, the bony forearm raised, the hand open under the flame.
    s.poly([[40, 35], [45, 36], [43, 46], [35, 50], [33, 46]], "H", (x, y) => (y < 42 ? 3 : 2));
    s.cap([[35, 47, 1.4], [31, 40, 1.3]], "K");
    s.ell(30, 39, 2.2, 1.8, "K");
    B.skull(s, { eyes: [[39, 22, 5], [40, 22, 4]] });
    // The flame over the palm.
    s.patch(25, 24, [
      "...w..w..",
      "..wv..vw.",
      "..vw.wv..",
      ".wvxwxvw.",
      ".vwxxxwv.",
      "wvwxxxwvw",
      "vwxxxxxwv",
      ".vwxxxwv.",
      ".uvwxwvu.",
      "..uvvvu..",
      "...uuu...",
    ]);
    // Bony fingers curled up around it.
    s.px(28, 37, "K", 4); s.px(32, 37, "K", 3); s.px(29, 36, "K", 3);
    // Embers rising.
    for (const [x, y] of [[27, 21], [32, 19], [24, 17]]) s.px(x, y, "R", 4);
  },
};
