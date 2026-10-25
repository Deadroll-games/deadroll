// Minotaur, 2x2 on 128, facing left: a towering bull-headed brute (K brown hide), long horns, a
// nose ring and the double-bladed axe in R (steel), a leather kilt and belt (H), shaggy thighs,
// hooves, burning eyes. Hooves on y = 123.
module.exports = {
  id: 436, name: "minotaur", size: 128,
  comment: [
    "Minotaur, a 2x2 brute, facing left: a bull head with long horns and a nose ring, burning eyes,",
    "a massive torso, a leather kilt, shaggy beast legs with hooves, a double axe. Hooves on y = 123.",
  ],
  build(s) {
    s.curve([0.62, 0.34, -0.04]);
    // Far leg (darker): thigh, shin back, hoof.
    s.cap([[70, 82, 7], [66, 98, 5.5]], "K", { tone: -1 });
    s.cap([[66, 98, 5], [72, 110, 4]], "K", { tone: -1 });
    s.cap([[72, 110, 3.6], [70, 119, 3.2]], "K", { tone: -1 });
    s.poly([[65, 118], [75, 118], [75, 123], [64, 123]], "R", 1);
    // Far arm hanging behind.
    s.mass([["c", [[74, 46, 7], [80, 64, 5.5], [78, 80, 5]]], ["e", 77, 84, 5.5, 5]], "K", { tone: -1, seam: true });
    // Torso: a huge chest and shoulders, the belly; the bull neck.
    s.mass([
      ["e", 62, 52, 18, 14],
      ["c", [[62, 60, 14], [64, 76, 11]]],
      ["e", 52, 42, 10, 9],
    ], "K", { k: 3, relief: 2.2 });
    // Leather kilt with a belt.
    s.poly([[50, 76], [78, 76], [80, 92], [75, 89], [72, 95], [67, 89], [63, 95], [59, 89], [54, 94], [51, 88]], "H",
      (x, y) => (y < 79 ? 2 : x > 70 ? 0 : 1));
    for (const x of [56, 62, 68, 74]) s.line(x, 80, x - 1, 90, "H", 0, false);
    s.line(50, 77, 78, 77, "H", 0); s.line(50, 78, 78, 78, "H", 4);
    s.poly([[60, 76], [65, 76], [65, 80], [60, 80]], "R", 3); // buckle
    // Near leg: shaggy thigh, the shin back from the knee, hoof.
    s.cap([[60, 86, 8], [54, 100, 6]], "K");
    s.cap([[54, 100, 5.4], [61, 111, 4.2]], "K");
    s.cap([[61, 111, 3.8], [58, 119, 3.4]], "K");
    s.poly([[52, 118], [63, 118], [63, 123], [51, 123]], "R", (x) => (x < 57 ? 3 : 2));
    // Head: a bull skull low on the neck, a broad muzzle.
    s.mass([["e", 44, 38, 9, 9], ["c", [[38, 44, 7], [32, 48, 6]]]], "K", { k: 1.8, seam: true });
    // Horns: from the brow out and forward, tips up.
    s.cap([[48, 31, 3.2], [58, 26, 2.6], [64, 18, 1.8], [62, 12, 1]], "K", { tone: 2 });
    s.cap([[40, 31, 3.4], [30, 26, 2.8], [24, 18, 2], [26, 11, 1]], "K", { tone: 2 });
    // Near arm: the axe raised across the body.
    s.mass([["c", [[52, 50, 7], [42, 64, 5.5], [34, 72, 5]]], ["e", 32, 74, 5.5, 5]], "K", { seam: true });
    s.seams();

    // Double axe: the haft from the fist up to the head over the shoulder.
    s.line(36, 84, 22, 36, "H", 1); s.line(37, 84, 23, 36, "H", 3);
    s.poly([[22, 40], [12, 30], [10, 38], [12, 48], [22, 44]], "R", (x, y) => (y < 38 ? 4 : 2));
    s.poly([[24, 40], [34, 30], [36, 38], [34, 48], [24, 44]], "R", (x, y) => (y < 38 ? 3 : 1));
    s.line(12, 30, 10, 38, "R", 5); s.line(10, 38, 12, 48, "R", 5);
    s.ell(23, 42, 2.4, 3, "R", { tone: -1 });
    // The fist over the haft.
    s.ell(31, 74, 4.5, 4, "K", {});
    // Face: burning eyes under the brow, wide nostrils, the nose ring.
    s.line(36, 35, 48, 34, "K", 1);
    s.px(39, 37, "Y", 4); s.px(40, 37, "Y", 5); s.px(46, 37, "Y", 3);
    s.px(28, 47, "D", 1); s.px(29, 48, "D", 1); s.px(33, 47, "D", 1);
    s.ell(30, 52, 2.4, 2, "R", { tone: 1 }); s.px(30, 52, "D", 0); s.px(30, 51, "D", 0);
    // Horn rings, shaggy fur on the thighs and the chest.
    for (const [x, y] of [[54, 28], [58, 25], [34, 28], [30, 25]]) s.px(x, y, "K", 1);
    for (const [x, y] of [[56, 88], [60, 92], [64, 88], [58, 96], [62, 46], [66, 52], [58, 54]]) s.line(x, y, x - 1, y + 3, "K", 1);
    for (const x of [52, 55, 58, 66, 69]) s.px(x, 123, "R", 0);
  },
};
