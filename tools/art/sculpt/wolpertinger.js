// Wolpertinger, 96, facing left: a rabid hare (K gray-brown fur) crouched to leap, deer antlers and
// long front fangs (R bone), small ragged feathered wings (H), red eyes (Y). Paws on y = 91.
const fwing = require("./feather_wing");
module.exports = {
  id: 460, name: "wolpertinger", size: 96,
  comment: [
    "Wolpertinger, facing left: a rabid hare crouched to leap, deer antlers, long front fangs, small",
    "ragged feathered wings, red eyes, matted fur. Paws on y = 91.",
  ],
  build(s) {
    s.curve([0.64, 0.36, -0.02]);
    // Far wing (darker), small, raised.
    fwing(s, [58, 68], [62, 60], [70, 54], 5, "H", -1, { back: [0.6, 1] });
    // Hind legs: the long hare foot flat on the ground, far one darker.
    s.mass([["e", 64, 80, 9, 9], ["c", [[66, 86, 3.2], [54, 90, 2.4]]]], "K", { tone: -1, seam: true });
    // Body: hunched, the rump high.
    s.mass([["e", 56, 76, 14, 11, { ragged: 0.15, tufts: 24 }], ["e", 44, 74, 8, 8]], "K", { k: 2, relief: 1.8 });
    s.mass([["e", 66, 82, 8, 8], ["c", [[68, 88, 3.2], [56, 91, 2.4]]]], "K", { seam: true });
    s.ell(72, 72, 3, 3, "K", { tone: 1 }); // the tail tuft
    // Front legs.
    s.cap([[44, 78, 2.6], [40, 86, 1.8], [38, 90, 1.6]], "K", { seam: true });
    // Head: a long hare head, ears laid back, the antlers.
    s.mass([["e", 38, 66, 6.5, 6], ["c", [[33, 69, 3.4], [28, 71, 2.4]]]], "K", { k: 1.4, seam: true });
    s.cap([[42, 63, 2], [52, 56, 1.8], [60, 56, 0.9]], "K", { tone: 0 });
    s.cap([[40, 63, 2.2], [48, 55, 2], [56, 52, 0.9]], "K", { tone: 1 });
    const antler = (x, y, dir, tone) => {
      s.cap([[x, y, 1.3], [x + dir * 2, y - 7, 1], [x + dir * 6, y - 13, 0.6]], "R", { tone });
      s.cap([[x + dir * 1, y - 4, 0.9], [x - dir * 3, y - 8, 0.5]], "R", { tone });
      s.cap([[x + dir * 3, y - 9, 0.9], [x + dir * 1, y - 14, 0.5]], "R", { tone });
    };
    antler(40, 61, 1, -1);
    antler(37, 61, -1, 0);
    // Near wing, small and ragged.
    fwing(s, [54, 70], [58, 62], [66, 56], 5, "H", 0, { back: [0.6, 1] });
    s.seams();

    // Face: red eye, nostril, the long fangs.
    s.px(36, 64, "Y", 4); s.px(37, 64, "Y", 5);
    s.line(34, 62, 39, 62, "K", 1);
    s.px(27, 70, "D", 1);
    s.line(28, 73, 31, 73, "D", 0);
    s.line(29, 73, 29, 75, "R", 5); s.line(30, 73, 30, 75, "R", 3);
    // Matted fur clumps.
    for (const [x, y] of [[50, 70], [58, 70], [62, 76], [46, 80], [54, 82]]) { s.px(x, y, "K", 4); s.px(x, y + 1, "K", 1); }
    for (const x of [36, 39, 54, 57]) s.px(x, 91, "R", 3);
  },
};
