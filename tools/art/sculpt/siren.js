// Drowned siren, 96, facing left: a pale corpse torso (K) rising from a scaled fish tail coiled on
// the ground (H), long wet dark hair clinging to the shoulders (R cloth), clawed hands reaching,
// a mouth of needle teeth, cold glowing eyes (Y). Coils on y = 91.
module.exports = {
  id: 455, name: "siren", size: 96,
  comment: [
    "Drowned siren, facing left: a pale corpse torso rising from a scaled fish tail coiled on the",
    "ground, long wet hair clinging, clawed hands reaching, needle teeth, cold eyes. Coils y = 91.",
  ],
  build(s) {
    s.curve([0.62, 0.34, -0.04]);
    // Long hair falling down the back (behind the torso).
    for (const [x, y, t] of [[62, 70, -1], [58, 74, 0], [64, 62, -1], [55, 66, 0]]) s.cap([[48, 34, 3], [54, 46, 2.6], [x, y, 0.8]], "R", { tone: t });
    // The tail: a thick coil on the ground, curling back, a torn fan fin at the end.
    s.mass([["c", [[50, 70, 6.5], [58, 80, 7], [70, 86, 6], [80, 82, 4.6], [84, 74, 3.4]]]], "H", { k: 2, relief: 1.8 });
    s.poly([[84, 72], [92, 62], [90, 70], [94, 72], [88, 76]], "H", (x, y) => (y < 68 ? 3 : 1));
    for (const [x, y] of [[90, 64], [92, 70]]) s.erase(x, y);
    s.mass([["c", [[48, 76, 6], [40, 86, 5.5], [52, 90, 5], [64, 90, 4]]]], "H", { k: 2, tone: -1 });
    // Torso: ribs under the skin, narrow waist into the scales.
    s.mass([["e", 46, 46, 7, 6.5], ["c", [[46, 48, 5.6], [48, 62, 5], [50, 70, 6]]]], "K", { k: 2, relief: 1.8 });
    // Head: gaunt, the jaw long.
    s.mass([["e", 42, 32, 5, 5.6], ["c", [[39, 36, 3], [37, 38, 2]]]], "K", { k: 1.2, seam: true });
    // Hair over the crown and clinging down the near shoulder.
    s.poly([[42, 27], [46, 25], [50, 28], [53, 34], [52, 46], [48, 40], [46, 32]], "R", (x, y) => (y < 30 ? 3 : 1));
    for (const [x, l] of [[44, 14], [47, 16]]) for (let i = 0; i < l; i++) s.px(x + (i >> 3), 34 + i, "R", i < 3 ? 2 : 1);
    // Arms reaching forward, long clawed fingers.
    s.cap([[50, 40, 2], [54, 48, 1.7], [50, 54, 1.5]], "K", { tone: -1, seam: true });
    s.cap([[42, 40, 2.2], [34, 46, 1.8], [26, 46, 1.5]], "K", { seam: true });
    s.seams();

    for (const [dx, dy] of [[-4, -2], [-5, 0], [-4, 2]]) { s.line(25, 46, 25 + dx, 46 + dy, "K", 3); s.px(25 + dx, 46 + dy, "Y", 2); }
    // Face, pixel by pixel: a gaunt profile, the sunken glowing eye, a hollow cheek, the long jaw
    // open on needle teeth; wet hair swept back.
    s.patch(35, 26, [
      "......tuuu..",
      ".....3tuuuu.",
      "....34ttuuu.",
      "...3443tuuu.",
      "..34kl3ttuu.",
      "..3444322tu.",
      ".44443212tu.",
      "45444321.tu.",
      ".m4m4m21.t..",
      ".mmmmm32....",
      "..4m4m2.....",
      "...333......",
    ]);
    // Ribs, scales on the tail, the seam where skin turns to scale.
    for (const y of [48, 51, 54]) s.line(44, y, 49, y + 1, "K", 1);
    for (let y = 70; y < 92; y += 3) for (let x = 40; x < 88; x += 4) {
      const ox = x + ((y / 3) % 2) * 2, p = s.at(ox, y);
      if (p?.slot === "H" && p.c >= 2) { p.c -= 1; const q = s.at(ox + 1, y); if (q?.slot === "H") q.c = Math.min(4, q.c + 1); }
    }
    s.line(43, 66, 55, 65, "H", 4);
  },
};
