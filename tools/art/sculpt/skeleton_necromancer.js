// Skeleton necromancer, 96, facing left: a hooded robe sewn with bones, a tall staff (R = wood)
// topped with a skull holding a toxic green crystal (Y), the same glow in the sockets.
const B = require("./skeleton_base");
module.exports = {
  id: 423, name: "skeleton_necromancer", size: 96,
  comment: [
    "Skeleton necromancer, facing left: a hooded robe sewn with bones, a tall staff planted ahead",
    "with a skull and a toxic green crystal, green glow in the sockets. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.6, 0.3, -0.06]);
    s.poly([[39, 88], [45, 88], [45, 91], [38, 91]], "K", (x) => (x < 41 ? 4 : 3));
    s.poly([[48, 88], [53, 88], [53, 91], [47, 91]], "K", 1);
    // Staff behind the near hand, planted on the ground ahead.
    s.line(29, 30, 31, 91, "R", 3, false); s.line(30, 30, 32, 91, "R", 1, false);
    // Robe to a ragged hem.
    s.poly([[38, 32], [53, 31], [56, 44], [58, 62], [60, 80], [58, 87], [55, 84], [53, 89], [50, 85], [47, 89],
      [44, 85], [41, 88], [38, 84], [37, 70], [38, 50]], "H", (x, y) => (x < 41 ? 3 : x > 54 ? 1 : 2));
    // Near arm: sleeve to the staff, a bony hand gripping it.
    s.poly([[40, 34], [45, 35], [42, 46], [34, 53], [32, 49]], "H", (x, y) => (y < 42 ? 3 : 2));
    s.ell(31, 51, 2.2, 2, "K");
    B.skull(s, { eyes: [[39, 22, 5], [40, 22, 4]] });
    // Hood up: framing the skull, the front edge dark.
    s.poly([[37, 18], [40, 13], [47, 11], [53, 14], [56, 22], [56, 32], [50, 34], [48, 26], [46, 18], [40, 17]], "H",
      (x, y) => (y < 15 && x < 48 ? 3 : x > 52 ? 1 : 2));
    s.line(37, 18, 41, 16, "H", 0); s.line(46, 18, 48, 27, "H", 0);
    // Bones sewn on the robe: a rib-like row and small finger bones.
    for (const y of [40, 44, 48]) s.line(41, y, 46, y + 1, "K", 4, false);
    for (const [x, y] of [[50, 66], [44, 72], [53, 76]]) { s.line(x, y, x + 1, y + 3, "K", 3, false); s.px(x, y, "K", 5); }
    // Staff head: a small skull holding a crystal, green glow.
    s.ell(31, 25, 3.6, 3.4, "K", {});
    s.patch(26, 24, [
      ".mm.....",
      "4mn4....",
      "44m4....",
      ".4m4m4..",
      "..4444..",
    ]);
    s.patch(28, 17, [
      "..l..",
      ".kjk.",
      ".jlj.",
      "kjljk",
      ".jkj.",
      ".ihi.",
      "..h..",
    ]);
    s.px(27, 26, "Y", 4); // a spark in the skull's socket
    for (const [x, y] of [[25, 14], [34, 12], [24, 20]]) s.px(x, y, "Y", 3);
  },
};
