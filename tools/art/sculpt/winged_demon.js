// Winged demon, 96, facing left: the demon body with great leathery bat wings spread behind and a
// jagged spear (R) in the clawed hand. Hooves on y = 91.
const demon = require("./demon");
const wing = require("./bat_wing");
module.exports = {
  id: 432, name: "winged_demon", size: 96,
  comment: [
    "Winged demon, facing left: the demon with great leathery bat wings spread behind and a jagged",
    "spear in the clawed hand. Hooves on y = 91.",
  ],
  build(s) {
    // Wings behind the body: the far one higher and darker, the near one wide; membrane in K.
    wing(s, [46, 36], [34, 14], [[24, 5], [14, 10], [10, 22], [18, 32]], [42, 46], -1, { bone: "R", skin: "K", arm: 2, finger: 2, armTone: -1 });
    wing(s, [48, 38], [62, 22], [[72, 8], [78, 16], [80, 30], [72, 46]], [54, 52], 0, { bone: "R", skin: "K", arm: 2.2, finger: 2, armTone: -1 });
    demon.build(s);
    // Jagged spear through the claws: shaft, a barbed head forward and up.
    s.line(44, 58, 14, 42, "H", 2); s.line(44, 57, 14, 41, "H", 4);
    s.poly([[15, 42], [5, 36], [4, 39], [2, 38], [5, 43], [12, 45]], "R", (x, y) => (y < 40 ? 4 : 2));
    s.line(5, 36, 4, 42, "R", 5, false);
    s.poly([[13, 44], [15, 39], [17, 45]], "R", 3); // barb
    // Claws closing over the shaft.
    s.px(27, 48, "K", 4); s.px(28, 50, "K", 3);
  },
};
