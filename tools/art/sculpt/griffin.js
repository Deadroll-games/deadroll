// Griffin, 2x2 on 128, facing left: a lion body in dark tawny fur (K) with a tufted tail, an eagle
// head and neck of dark feathers (H), a hooked beak and talons (R gold), feathered wings raised,
// a burning eye (Y). Feet on y = 123.
const fwing = require("./feather_wing");
module.exports = {
  id: 443, name: "griffin", size: 128,
  comment: [
    "Griffin, a 2x2 beast, facing left: a lion body with a tufted tail, an eagle head with a hooked",
    "beak, feathered wings raised, eagle talons in front, lion paws behind. Feet on y = 123.",
  ],
  build(s) {
    s.curve([0.64, 0.36, -0.02]);
    // Far wing (darker), raised high behind.
    fwing(s, [74, 64], [80, 38], [92, 14], 12, "H", -1, { back: [1, 0.35] });
    // Tail with a tuft, far legs.
    s.cap([[100, 84, 3], [112, 80, 2.4], [118, 70, 1.8]], "K", { tone: -1 });
    s.ell(119, 66, 4, 5, "H", { tone: -1, ragged: 0.4, tufts: 9 });
    s.mass([["c", [[56, 92, 6.5], [54, 106, 5], [54, 117, 4]]]], "H", { tone: -1, seam: true });
    s.mass([["c", [[94, 92, 8], [98, 106, 6], [94, 117, 5]]], ["e", 92, 120, 7, 3]], "K", { tone: -1, seam: true });
    // Lion body, the feathered chest.
    s.mass([["e", 78, 86, 22, 13], ["e", 98, 86, 11, 11]], "K", { k: 3, relief: 1.8 });
    s.mass([["e", 56, 80, 12, 14, { ragged: 0.25, tufts: 20 }], ["c", [[52, 72, 9], [46, 62, 7.5]]]], "H", { k: 2, relief: 1.6 });
    // Eagle head: the skull, a hooked beak.
    s.mass([["e", 42, 56, 8, 7.5]], "H", { k: 1.4, seam: true });
    s.poly([[35, 52], [28, 53], [24, 57], [25, 61], [28, 59], [33, 60], [36, 59]], "R", (x, y) => (y < 56 ? 4 : 2));
    s.line(24, 57, 25, 61, "R", 1);
    // Near legs: eagle foreleg with talons, the lion hind leg.
    s.mass([["c", [[54, 92, 6.5], [48, 106, 4.6], [47, 116, 3.6]]]], "H", { seam: true });
    s.mass([["e", 96, 92, 10, 11], ["c", [[100, 100, 7], [96, 110, 5.5], [100, 117, 4.6]]], ["e", 98, 120, 7, 3]], "K", { seam: true });
    // Near wing, raised over the back.
    fwing(s, [64, 66], [68, 36], [80, 10], 14, "H", 0, { back: [1, 0.4] });
    s.seams();

    // Talons on the eagle foot.
    for (const [x, y, dx] of [[44, 118, -3], [47, 119, -1], [50, 119, 1]]) { s.line(x, y, x + dx, y + 4, "R", 3); s.px(x + dx, y + 4, "R", 5); }
    for (const x of [56, 58]) s.px(x, 123, "R", 3);
    for (const x of [92, 95, 98]) s.px(x, 123, "K", 1);
    // Eye and brow, the beak's nostril.
    s.line(36, 52, 44, 51, "H", 0);
    s.px(38, 54, "Y", 4); s.px(39, 54, "Y", 5);
    s.px(32, 55, "D", 1);
    // Feather scallops on the chest and neck.
    for (let y = 56; y < 90; y += 4) for (let x = 44; x < 66; x += 4) {
      const ox = x + ((y / 4) % 2) * 2;
      if (s.at(ox, y)?.slot === "H" && s.at(ox, y + 1)?.slot === "H") { s.px(ox, y + 1, "H", 1); s.px(ox + 1, y + 1, "H", 1); }
    }
  },
};
