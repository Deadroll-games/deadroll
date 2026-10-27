// Mimic, 96, facing left: an iron-bound wooden chest (K wood, H iron) with the lid gaping, rows of
// crooked teeth (Y bone), a lolling tongue and gums (R flesh), pale dead eyes inside the dark,
// clawed legs under the bottom. Feet on y = 91.
module.exports = {
  id: 428, name: "mimic", size: 96,
  comment: [
    "Mimic, facing left: an iron-bound chest with the lid gaping, crooked teeth, a lolling tongue,",
    "pale eyes in the dark of the box, clawed legs under the bottom. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.66, 0.36, -0.04]);
    // Clawed legs under the box (far pair darker).
    for (const [x0, tone] of [[58, -1], [36, 0]]) {
      s.cap([[x0, 80, 2.6], [x0 - 4, 86, 2], [x0 - 2, 90, 1.6]], "R", { tone: tone - 1 });
      for (const dx of [-5, -3, -1]) s.px(x0 + dx, 91, "Y", 3);
    }
    // The box: front face (lit left), side face (darker), planks and iron bands.
    s.poly([[28, 62], [58, 62], [58, 84], [28, 84]], "K", (x, y) => (y < 66 ? 3 : 2));
    s.poly([[58, 62], [68, 57], [68, 79], [58, 84]], "K", 1);
    for (const y of [69, 76]) { s.line(28, y, 57, y, "K", 0); s.line(59, y - 3, 67, y - 7, "K", 0); }
    for (const x of [30, 44, 56]) { s.line(x, 62, x, 84, "H", 3); s.line(x + 1, 62, x + 1, 84, "H", 1); }
    s.line(59, 62, 67, 58, "H", 2); s.line(59, 83, 67, 79, "H", 2);
    for (const [x, y] of [[30, 65], [30, 80], [44, 65], [44, 80], [56, 65], [56, 80]]) s.px(x, y, "H", 5); // rivets
    // The lock plate hanging open, bent.
    s.poly([[40, 62], [46, 62], [46, 67], [43, 69], [40, 67]], "H", (x, y) => (y < 64 ? 4 : 2));
    s.px(43, 65, "D", 1);
    // Inside: the dark maw between the box and the lid.
    s.poly([[28, 62], [58, 62], [68, 57], [64, 46], [52, 40], [30, 46]], "D", (x, y) => (y < 50 ? 1 : 0));
    // Lid gaping up and back: underside planks, iron edge.
    s.poly([[30, 46], [52, 40], [64, 46], [62, 30], [50, 24], [28, 32]], "K", (x, y) => (x < 40 ? 2 : 1));
    // The arched end of the lid on the right, and its iron edge.
    s.poly([[62, 30], [66, 32], [69, 38], [68, 45], [64, 46]], "K", 2);
    s.line(62, 30, 66, 32, "H", 3); s.line(66, 32, 69, 38, "H", 3); s.line(69, 38, 68, 45, "H", 2);
    s.line(28, 32, 50, 24, "H", 4); s.line(50, 24, 62, 30, "H", 2); s.line(30, 46, 28, 32, "H", 3);
    for (const [a, b] of [[[36, 44], [34, 30]], [[44, 42], [42, 27]]]) s.line(a[0], a[1], b[0], b[1], "K", 0);
    // Teeth: crooked rows on the box rim (pointing up) and the lid edge (pointing down).
    for (let i = 0; i < 7; i++) {
      const x = 29 + i * 4, h = 5 + ((i * 7) % 3), lean = (i % 2) - 0.5;
      s.poly([[x, 62], [x + 1.5 + lean, 62 - h], [x + 3.2, 62]], "Y", (px) => (px <= x + 1 ? 4 : 2));
    }
    for (let i = 0; i < 7; i++) {
      const x = 31 + i * 4, y = 46 - Math.round(i * 0.9), h = 5 + ((i * 5) % 3), lean = 0.5 - (i % 2);
      s.poly([[x, y], [x + 1.5 + lean, y + h], [x + 3.2, y - 0.7]], "Y", (px) => (px <= x + 1 ? 4 : 3));
    }
    // Gums and the lolling tongue over the front.
    s.line(29, 61, 57, 61, "R", 2);
    s.cap([[44, 58, 3.2], [36, 64, 3], [30, 72, 2.6], [32, 78, 2]], "R", {});
    s.line(42, 58, 31, 72, "R", 1, false); // the tongue's groove
    // Pale dead eyes in the dark.
    for (const [x, y, r] of [[45, 52, 2.2], [55, 50, 1.6], [50, 55, 1.2]]) {
      s.ell(x, y, r * 1.1, r * 0.8, "Y", { tone: 1 });
      s.px(x, y, "D", 0); if (r > 2) s.px(x, y - 1, "D", 0);
    }
    // Drool.
    s.px(34, 79, "R", 4); s.px(34, 80, "R", 3);
  },
};
