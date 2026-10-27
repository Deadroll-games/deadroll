// Jackal lord, 96, facing left: a tall black-furred warrior (K) with a jackal head, long upright
// ears, glowing eyes (Y), a broad collar and belt of tarnished gold (H), a linen kilt, a sickle
// sword (khopesh, R steel). Feet on y = 91.
module.exports = {
  id: 448, name: "jackal_lord", size: 96,
  comment: [
    "Jackal lord, facing left: tall and black-furred, a jackal head with long upright ears and",
    "glowing eyes, a broad collar and belt of tarnished gold, a sickle sword. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.62, 0.34, -0.04]);
    // Far leg and far arm (darker).
    s.cap([[50, 58, 3.6], [52, 73, 2.8], [51, 88, 2.4]], "K", { tone: -1 });
    s.poly([[48, 88], [54, 88], [55, 91], [46, 91]], "K", 1);
    s.cap([[52, 36, 2.8], [56, 48, 2.3], [56, 58, 2]], "K", { tone: -1 });
    // Torso: broad shoulders, narrow waist.
    s.mass([["e", 47, 40, 9, 6], ["c", [[47, 42, 6], [47, 54, 4.6]]]], "K", { k: 2, relief: 1.8 });
    // Gold belt and the kilt with a front flap.
    s.poly([[41, 55], [53, 55], [55, 66], [39, 66]], "H", (x, y) => (y < 57 ? 3 : 1));
    s.poly([[44, 56], [49, 56], [48, 70], [45, 70]], "H", (x) => (x < 46 ? 4 : 2));
    // Near leg.
    s.cap([[45, 62, 3.8], [42, 74, 3], [43, 88, 2.6]], "K");
    s.ell(42, 74, 2.8, 2.4, "K");
    s.poly([[38, 88], [46, 88], [46, 91], [36, 91]], "K", (x) => (x < 41 ? 4 : 3));
    // Head: a jackal skull, a long snout, tall upright ears.
    s.mass([["e", 44, 26, 5, 5], ["c", [[41, 28, 2.8], [33, 31, 1.7]]]], "K", { k: 1.2, seam: true });
    s.poly([[44, 22], [44, 10], [48, 22]], "K", (x) => (x < 46 ? 4 : 2));
    s.poly([[47, 22], [49, 11], [51, 23]], "K", 1);
    s.line(45, 20, 45, 13, "H", 2); // inner ear trim
    // Broad collar of gold over the shoulders.
    s.poly([[38, 34], [56, 34], [55, 40], [47, 43], [39, 40]], "H", (x, y) => (y < 36 ? 4 : y < 39 ? 3 : 2));
    for (const x of [41, 45, 49, 53]) s.px(x, 38, "H", 1);
    // Near arm forward, the hand on the khopesh.
    s.cap([[40, 38, 2.8], [35, 47, 2.3], [30, 50, 2]], "K", { seam: true });
    s.seams();

    // Khopesh: the hilt in the fist, the straight neck, the sickle blade hooking forward.
    s.line(29, 53, 31, 47, "H", 3);
    s.line(29, 50, 21, 43, "R", 3); s.line(29, 51, 21, 44, "R", 1);
    // The sickle: the outer edge arcs up, forward and down into a hook; the inner edge returns.
    s.poly([[22, 43], [18, 37], [12, 34], [7, 36], [5, 42], [7, 49], [10, 50], [9, 44], [11, 40], [15, 40], [19, 45]], "R",
      (x, y) => (y < 39 ? 4 : x < 9 ? 3 : 2));
    s.line(12, 34, 7, 36, "R", 5); s.line(7, 36, 5, 42, "R", 5); s.line(5, 42, 7, 49, "R", 4);
    s.ell(30, 50, 2.2, 2, "K", {});
    // Gold bands on the arm, the face: eye glow, nose, a grim mouth line.
    s.line(37, 44, 39, 42, "H", 4); s.line(37, 45, 39, 43, "H", 2);
    s.px(41, 25, "Y", 4); s.px(42, 25, "Y", 5);
    s.line(39, 23, 45, 22, "K", 1);
    s.px(32, 30, "D", 1);
    s.line(34, 32, 40, 31, "D", 1);
  },
};
