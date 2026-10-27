// Thunderbird, 96, facing left, hovering: a dark storm eagle (K and H feathers) with wings spread
// up, a hooked gold beak and talons (R), a burning eye and lightning crackling between the
// feathers (Y). The talons are the lowest point, about y = 84.
const fwing = require("./feather_wing");
module.exports = {
  id: 449, name: "thunderbird", size: 96,
  comment: [
    "Thunderbird, facing left, hovering: a dark storm eagle with wings spread up, a hooked beak,",
    "talons hanging, a burning eye, lightning crackling between the feathers.",
  ],
  build(s) {
    s.curve([0.62, 0.34, -0.04]);
    // Far wing (darker) up and back, the tail fan.
    fwing(s, [54, 44], [66, 26], [86, 8], 12, "H", -1, { back: [0.35, 1] });
    for (let i = 0; i < 5; i++) s.cap([[54, 64, 2.6], [60 + i * 2, 80 + (i % 2) * 2, 1.2]], "H", { tone: i % 2 ? -1 : 0 });
    // Body: chest and belly, the legs with talons hanging.
    s.mass([["e", 48, 52, 9, 12], ["e", 52, 64, 6, 6]], "K", { k: 2, relief: 1.8 });
    // Feathered thighs, short gold shanks, talons.
    s.ell(45, 66, 3.6, 4.4, "K", { tone: -1, ragged: 0.3, tufts: 8 }); s.ell(52, 67, 3.6, 4.4, "K", { ragged: 0.3, tufts: 8 });
    s.cap([[45, 70, 1.6], [44, 76, 1.4]], "R", { tone: -1 });
    s.cap([[52, 71, 1.6], [52, 77, 1.4]], "R", {});
    for (const [x, y] of [[44, 76], [52, 77]]) for (const dx of [-3, 0, 3]) { s.line(x, y, x + dx, y + 4, "R", 3); s.px(x + dx, y + 5, "R", 5); }
    // Near wing up and forward.
    fwing(s, [44, 46], [34, 26], [14, 8], 14, "H", 0, { back: [0.3, 1] });
    // Head: a fierce eagle head forward, the hooked beak.
    s.mass([["e", 40, 38, 6.5, 6]], "K", { k: 1.4, seam: true });
    s.poly([[35, 35], [29, 36], [26, 40], [27, 43], [29, 41], [33, 42], [35, 41]], "R", (x, y) => (y < 39 ? 4 : 2));
    s.seams();

    // Eye, brow, nostril; feather scallops on the chest.
    s.line(35, 35, 43, 34, "K", 1);
    s.px(37, 37, "Y", 4); s.px(38, 37, "Y", 5);
    s.px(31, 38, "D", 1);
    for (let y = 46; y < 64; y += 3) for (let x = 42; x < 56; x += 3) {
      const ox = x + ((y / 3) % 2);
      if (s.at(ox, y)?.slot === "K") s.px(ox, y, "K", 1);
    }
    // Lightning: jagged bolts crackling along the wings and down to the talons.
    const bolt = (pts) => { for (let i = 0; i + 1 < pts.length; i++) s.line(pts[i][0], pts[i][1], pts[i + 1][0], pts[i + 1][1], "Y", i % 2 ? 4 : 5); };
    bolt([[18, 14], [22, 20], [19, 23], [25, 30]]);
    bolt([[80, 12], [76, 18], [80, 21], [74, 28]]);
    bolt([[48, 82], [46, 86], [50, 88], [47, 92]]);
    for (const [x, y] of [[12, 24], [88, 22], [30, 18], [64, 16]]) s.px(x, y, "Y", 3);
  },
};
