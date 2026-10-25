// Cave owl, 96, facing left (3/4, the face turned to the viewer as owls do): dark feathers (K, H),
// feather tufts like horns, a facial disc, huge glowing eyes (Y gold), a hooked beak and talons
// (R bone), wings half open. Talons on y = 91.
const fwing = require("./feather_wing");
module.exports = {
  id: 458, name: "owl", size: 96,
  comment: [
    "Cave owl, facing left: dark feathers, horn-like ear tufts, a facial disc around huge glowing",
    "eyes, a hooked beak, wings half open, talons spread. Talons on y = 91.",
  ],
  build(s) {
    s.curve([0.62, 0.34, -0.04]);
    // Far wing (darker) half open behind.
    fwing(s, [56, 52], [70, 40], [86, 36], 12, "H", -1, { back: [0.3, 1] });
    // Body: a heavy feathered body, the tail.
    for (let i = 0; i < 4; i++) s.cap([[52, 76, 2.4], [56 + i * 2, 88, 1]], "H", { tone: -1 });
    s.mass([["e", 48, 64, 13, 15, { ragged: 0.15, tufts: 24 }]], "K", { k: 2.5, relief: 1.6 });
    // Head: big and round, the facial disc, ear tufts.
    s.mass([["e", 44, 42, 12, 11]], "K", { k: 2, seam: true });
    s.poly([[36, 34], [32, 22], [40, 32]], "K", 3);
    s.poly([[50, 33], [56, 22], [54, 34]], "K", 1);
    s.ell(38, 44, 6.5, 6.5, "H", { tone: 1 });
    s.ell(50, 44, 5.4, 6, "H", {});
    // Legs: feathered, the talons spread on the ground.
    for (const [x, t] of [[50, -1], [42, 0]]) {
      s.cap([[x + 1, 76, 3], [x, 86, 2]], "K", { tone: t - 1 });
      for (const dx of [-4, -1, 2, 4]) { s.line(x, 87, x + dx, 91, "R", 3 + t); s.px(x + dx, 91, "R", 5 + t); }
    }
    // Near wing half open in front.
    fwing(s, [42, 54], [32, 44], [20, 46], 13, "H", 0, { back: [0.6, 1] });
    s.seams();

    // Eyes: huge, glowing, a black pupil, the rim dark.
    for (const [x, y, r] of [[38, 44, 3.6], [50, 44, 3]]) {
      s.ell(x, y, r, r, "Y", { tone: 1 });
      s.ell(x, y, r * 0.45, r * 0.45, "D", { tone: -3 });
      s.px(Math.round(x - 1), Math.round(y + 1), "Y", 5);
    }
    // Heavy brows slanting down to the beak, the upper lids half closed: a glare.
    for (const [x0, y0, x1, y1] of [[32, 38, 42, 42], [46, 42, 55, 38]]) { s.line(x0, y0, x1, y1, "K", 0); s.line(x0, y0 + 1, x1, y1 + 1, "K", 0); s.line(x0, y0 - 1, x1, y1 - 1, "K", 3); }
    for (let x = 34; x <= 42; x++) s.px(x, 42, "K", 1);
    for (let x = 47; x <= 53; x++) s.px(x, 42, "K", 1);
    // Hooked beak between the eyes.
    s.poly([[43, 46], [46, 46], [45, 52], [43, 50]], "R", (x) => (x < 45 ? 4 : 2));
    // Breast feather chevrons.
    for (let y = 58; y < 78; y += 4) for (let x = 40; x < 58; x += 4) {
      const ox = x + ((y / 4) % 2) * 2;
      if (s.at(ox, y)?.slot === "K") { s.px(ox, y, "K", 1); s.px(ox + 1, y + 1, "K", 1); s.px(ox - 1, y + 1, "K", 1); }
    }
  },
};
