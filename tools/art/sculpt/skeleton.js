// Skeleton, 96, facing left: the human canon in bare bone, a notched rusty sword held forward,
// a cold glow in the sockets. Feet on y = 91.
const B = require("./skeleton_base");
module.exports = {
  id: 420, name: "skeleton", size: 96,
  comment: [
    "Skeleton, facing left: bare bones on the human canon, a notched rusty sword held forward,",
    "a cold glow in the eye sockets, cracks on the skull. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.6, 0.3, -0.06]);
    B.far(s);
    B.body(s);
    B.arm(s, [33, 56], [38, 47]);
    B.skull(s);
    // Sword: grip in the fist, a short cross guard, a notched blade forward and up.
    s.line(34, 59, 32, 53, "H", 2); s.line(29, 56, 36, 53, "R", 2);
    s.poly([[32, 54], [17, 40], [15, 40], [16, 43], [30, 56]], "R", (x, y) => (x + y < 74 ? 3 : 2));
    s.line(15, 40, 30, 55, "R", 4, false);
    s.erase(20, 46); s.erase(24, 49);
    for (const [x, y] of [[22, 46], [26, 50], [19, 43]]) s.px(x, y, "H", 3); // rust
    // Bony fingers around the grip.
    s.px(32, 57, "K", 4); s.px(31, 56, "K", 3); s.px(34, 58, "K", 2);
  },
};
