// Cockatrice, 96, facing left: a rooster head with an inflamed crimson comb and wattle (R), a yellow
// beak and a petrifying eye (Y gold), dark ragged feathers on the chest and wings (H), a scaled
// serpent tail coiled on the ground (K), rooster legs with spurs. Coils on y = 91.
const fwing = require("./feather_wing");
module.exports = {
  id: 457, name: "cockatrice", size: 96,
  comment: [
    "Cockatrice, facing left: a rooster head with an inflamed comb and wattle, a petrifying eye,",
    "dark ragged feathers and wings, a scaled serpent tail coiled behind, spurred legs. Coils y = 91.",
  ],
  build(s) {
    s.curve([0.62, 0.34, -0.04]);
    // Serpent tail coiling back on the ground, rising at the end.
    s.mass([["c", [[56, 72, 6], [66, 82, 5.5], [78, 86, 5], [86, 80, 4], [84, 70, 3], [78, 66, 2]]]], "K", { k: 2, relief: 1.8 });
    // Far wing (darker), half raised.
    fwing(s, [54, 56], [64, 42], [80, 32], 9, "H", -1, { back: [0.6, 1] });
    // Legs: scaly rooster legs with spurs and claws.
    for (const [x, t] of [[50, -1], [44, 0]]) {
      s.cap([[x + 2, 74, 2], [x, 82, 1.4], [x, 88, 1.2]], "Y", { tone: t - 1 });
      for (const dx of [-4, -1, 2]) s.line(x, 89, x + dx, 91, "Y", 2 + t);
      s.px(x + 2, 84, "Y", 3 + t);
    }
    // Body: a feathered chest.
    s.mass([["e", 50, 62, 10, 11, { ragged: 0.25, tufts: 20 }]], "H", { k: 2, relief: 1.6 });
    // Neck and head.
    s.mass([["c", [[46, 56, 5], [40, 46, 4], [38, 38, 4]]]], "H", { k: 1.5, seam: true });
    s.mass([["e", 36, 36, 5, 4.6]], "H", { k: 1.2, seam: true });
    // Comb on the crown, wattle under the beak, the beak.
    s.poly([[34, 32], [35, 26], [37, 30], [39, 25], [40, 30], [42, 27], [42, 33]], "R", (x, y) => (y < 29 ? 4 : 2));
    s.mass([["e", 32, 42, 2.2, 3.2]], "R", {});
    s.poly([[33, 35], [27, 37], [29, 39], [33, 39]], "Y", (x, y) => (y < 37 ? 4 : 2));
    // Near wing, ragged, half raised.
    fwing(s, [48, 58], [56, 44], [70, 34], 10, "H", 0, { back: [0.6, 1] });
    s.seams();

    // The petrifying eye: gold with a black slit.
    s.px(36, 35, "Y", 5); s.px(37, 35, "Y", 4); s.px(37, 34, "D", 0);
    s.line(34, 33, 39, 33, "H", 0);
    // Scales on the tail: a sparse lattice.
    for (let y = 66; y < 92; y += 3) for (let x = 54; x < 90; x += 3) { const p = s.at(x + ((y / 3) % 2), y); if (p?.slot === "K" && p.c >= 2) p.c -= 1; }
    // Bald, sickly patches in the feathers.
    for (const [x, y] of [[52, 66], [46, 60], [54, 58]]) { s.px(x, y, "K", 3); s.px(x + 1, y, "K", 2); }
  },
};
