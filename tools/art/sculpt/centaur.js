// Centaur, 2x2 on 128, facing left: a dark horse body (K) on four legs with hooves and a tail, a
// pale human torso (H) with long tangled hair, burning eyes (Y), drawing a recurve bow (R wood)
// with an arrow aimed left. Hooves on y = 123.
module.exports = {
  id: 447, name: "centaur", size: 128,
  comment: [
    "Centaur, a 2x2 beast, facing left: a dark horse body with a tail, a pale human torso with long",
    "tangled hair and burning eyes, drawing a recurve bow with an arrow aimed left. Hooves y = 123.",
  ],
  build(s) {
    s.curve([0.64, 0.36, -0.02]);
    // Horse tail, far legs (darker).
    s.cap([[104, 80, 3.4], [112, 86, 3], [116, 98, 2.4], [114, 108, 1.4]], "K", { tone: -1 });
    for (const dx of [-2, 0, 2]) s.line(114 + dx, 96, 113 + dx * 2, 110, "K", 1, false);
    const leg = (pts, tone) => {
      s.mass([["c", pts]], "K", { tone, seam: true });
      const [x, y] = pts[pts.length - 1];
      s.poly([[x - 3, y], [x + 3, y], [x + 3.5, 123], [x - 3.5, 123]], "R", Math.max(0, 1 + tone));
    };
    leg([[56, 88, 5.5], [54, 102, 3.6], [56, 110, 3], [54, 119, 2.6]], -1);
    leg([[94, 88, 6], [98, 100, 4], [96, 110, 3], [97, 119, 2.6]], -1);
    // Horse body.
    s.mass([["e", 78, 82, 26, 12], ["e", 100, 82, 10, 11], ["e", 56, 80, 11, 12]], "K", { k: 3, relief: 1.8 });
    // Near legs.
    leg([[52, 90, 5.5], [46, 102, 3.6], [44, 110, 3], [46, 119, 2.6]], 0);
    leg([[98, 90, 6], [102, 102, 4], [100, 110, 3], [102, 119, 2.6]], 0);
    // Human torso rising from the horse's chest, the head, long hair falling back.
    s.mass([["c", [[56, 72, 7], [52, 56, 7.5], [50, 46, 6]]], ["e", 51, 45, 8.5, 5]], "H", { k: 2, relief: 1.8, seam: true });
    for (const [tx, ty] of [[62, 50], [60, 56], [58, 60]]) s.cap([[52, 32, 3], [tx, ty, 0.8]], "K", { tone: -1 });
    s.mass([["e", 48, 34, 6, 6.6], ["c", [[45, 38, 3.2], [43, 40, 2.2]]]], "H", { k: 1.2, seam: true });
    s.poly([[46, 28], [52, 27], [56, 32], [54, 36], [50, 31]], "K", 2); // hair on the crown
    // Far arm drawing the string back to the cheek, the near arm holding the bow forward.
    s.cap([[54, 44, 2.6], [58, 40, 2.2], [52, 36, 1.8]], "H", { tone: -1, seam: true });
    s.cap([[48, 44, 2.8], [38, 42, 2.2], [30, 41, 2]], "H", { seam: true });
    s.seams();

    // Recurve bow held vertically in the near hand, the string to the far hand, the arrow.
    const bow = [[34, 22], [30, 26], [28, 34], [29, 41], [28, 48], [30, 56], [34, 60]];
    for (let i = 0; i + 1 < bow.length; i++) { s.line(bow[i][0], bow[i][1], bow[i + 1][0], bow[i + 1][1], "R", 3); s.line(bow[i][0] + 1, bow[i][1], bow[i + 1][0] + 1, bow[i + 1][1], "R", 1); }
    s.line(34, 22, 51, 37, "R", 4, false); s.line(51, 37, 34, 60, "R", 4, false);
    s.line(51, 37, 14, 40, "R", 2); s.poly([[16, 38], [10, 40], [16, 42]], "R", 5); // shaft and head
    for (const [x, y] of [[48, 36], [48, 38]]) s.px(x, y, "Y", 3); // fletching
    s.ell(30, 41, 2.2, 2, "H", {}); // the fist on the grip
    // Face: brow, burning eye, a grim mouth.
    s.line(44, 32, 49, 31, "K", 1);
    s.px(45, 33, "Y", 4); s.px(46, 33, "Y", 5);
    s.line(43, 38, 46, 38, "D", 1);
    // Chest and stomach shading on the torso, the horse's chest muscle.
    s.line(48, 52, 54, 52, "H", 1); s.line(50, 58, 56, 58, "H", 1);
    s.line(58, 76, 62, 86, "K", 1);
  },
};
