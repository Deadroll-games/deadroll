// Pegasus, 96, facing left: a pale ghostly horse (K) with great black raven wings raised (H), a
// dark mane and tail, cold glowing eyes (Y). Hooves on y = 91.
const Hb = require("./horse_base");
const fwing = require("./feather_wing");
module.exports = {
  id: 453, name: "pegasus", size: 96,
  comment: [
    "Pegasus, facing left: a pale ghostly horse with great black raven wings raised over the back,",
    "a dark mane and tail, cold glowing eyes. Hooves on y = 91.",
  ],
  build(s) {
    s.curve([0.6, 0.32, -0.06]);
    // Far wing raised behind (darker), the tail.
    fwing(s, [58, 50], [66, 28], [84, 6], 10, "H", -1, { back: [0.5, 1] });
    for (const [x, y, t] of [[88, 76, -1], [86, 82, 0]]) s.cap([[78, 52, 2.6], [84, 60, 2.2], [x, y, 0.8]], "H", { tone: t });
    Hb.far(s);
    Hb.body(s);
    // Mane: dark strands along the neck.
    for (let i = 0; i < 6; i++) {
      const x = 32 + i * 2.4, y = 34 + i * 3;
      s.cap([[x, y, 2], [x + 5, y + 5 + (i % 2) * 2, 0.6]], "H", { tone: i % 2 ? -1 : 0 });
    }
    // Near wing raised high over the back.
    fwing(s, [50, 50], [54, 26], [66, 4], 12, "H", 0, { back: [0.6, 1] });
    s.seams();
    Hb.face(s);
    s.px(24, 33, "Y", 5); s.px(25, 33, "Y", 4); s.px(23, 34, "Y", 2);
  },
};
