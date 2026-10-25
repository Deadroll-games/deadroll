// Frost wolf, 96, facing left: the dire wolf's body in pale fur (K, H), ice spikes along the spine
// (R cold), frost breath from the snarl and cold glowing eyes (Y). Paws on y = 91.
const wolf = require("./wolf");
module.exports = {
  id: 461, name: "frost_wolf", size: 96,
  comment: [
    "Frost wolf, facing left: the dire wolf in pale fur, ice spikes along the spine, frost breath",
    "from the snarl, cold glowing eyes. Paws on y = 91.",
  ],
  build(s) {
    wolf.build(s);
    // Ice spikes along the neck and back, lit on their left faces.
    for (const [x, h] of [[38, 6], [43, 8], [48, 7], [53, 6], [58, 5], [63, 4], [68, 4]]) {
      const top = [...Array(96).keys()].find((y) => s.at(x, y));
      if (top !== undefined) s.poly([[x - 1.8, top + 2], [x - 0.5, top - h], [x + 1.8, top + 2]], "R", (px) => (px < x ? 5 : 3));
    }
    // Frost breath curling from the jaws, ice on the paws.
    for (const [x, y, c] of [[2, 60, 4], [4, 62, 3], [1, 64, 2], [6, 58, 3], [3, 66, 2], [7, 64, 1]]) s.px(x, y, "R", c);
    for (const x of [28, 30, 71, 73]) s.px(x, 90, "R", 4);
  },
};
