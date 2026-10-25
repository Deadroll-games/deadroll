// Blighted unicorn, 96, facing left: a gaunt black horse with ribs showing (K), a cracked spiral horn
// of bone (R), a mane and tail of tattered shadow (H), red eyes (Y). Hooves on y = 91.
const H = require("./horse_base");
module.exports = {
  id: 451, name: "unicorn", size: 96,
  comment: [
    "Blighted unicorn, facing left: a gaunt black horse with ribs showing, a cracked spiral bone",
    "horn, a mane and tail of tattered shadow, red eyes. Hooves on y = 91.",
  ],
  build(s) {
    s.curve([0.6, 0.32, -0.06]);
    // Tail of tattered shadow.
    for (const [x, y, t] of [[88, 74, -1], [86, 82, 0], [84, 70, -1]]) s.cap([[78, 52, 2.6], [84, 58, 2.2], [x, y, 0.8]], "H", { tone: t });
    H.far(s);
    H.body(s, { lean: true });
    // Mane: tattered strands along the neck, a forelock.
    for (let i = 0; i < 7; i++) {
      const x = 32 + i * 2.4, y = 34 + i * 3;
      s.cap([[x, y, 2], [x + 6, y + 6 + (i % 2) * 3, 0.6]], "H", { tone: i % 2 ? -1 : 0 });
    }
    s.cap([[27, 30, 1.6], [23, 34, 0.6]], "H", {});
    // The horn: a spiral of bone from the forehead, cracked.
    s.cap([[26, 29, 2], [22, 20, 1.4], [19, 12, 0.6]], "R", {});
    for (const y of [26, 22, 18, 15]) { const x = Math.round(26 - (29 - y) * 0.42); s.px(x, y, "R", 1); s.px(x + 1, y - 1, "R", 4); }
    s.seams();
    H.face(s, { lean: true });
    // Blight: dark veins on the neck and shoulder.
    s.line(36, 44, 38, 54, "K", 0); s.line(38, 54, 42, 58, "K", 0);
  },
};
