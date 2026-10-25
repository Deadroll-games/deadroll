// Demon, 96, facing left: lean and sinewy, bruise-purple skin (K), curled ram horns and claws (R
// bone), burning eyes (Y), clawed hands reaching, hoofed beast legs, a barbed tail, a torn
// loincloth (H). Hooves on y = 91.
module.exports = {
  id: 431, name: "demon", size: 96,
  comment: [
    "Demon, facing left: lean and sinewy, bruise-purple skin, curled ram horns, burning eyes,",
    "clawed hands, hoofed beast legs, a barbed tail, a torn loincloth. Hooves on y = 91.",
  ],
  build(s) {
    s.curve([0.62, 0.32, -0.04]);
    // Barbed tail curling behind.
    s.cap([[54, 58, 2.4], [64, 62, 2], [70, 56, 1.6], [72, 48, 1.2]], "K", { tone: -1 });
    s.poly([[70, 46], [74, 41], [75, 48]], "R", 3);
    // Far leg (darker): thigh, knee forward, shin back, hoof.
    s.cap([[52, 58, 3.6], [49, 69, 2.8]], "K", { tone: -1 });
    s.cap([[49, 69, 2.4], [56, 79, 1.9]], "K", { tone: -1 });
    s.cap([[56, 79, 1.8], [53, 88, 1.6]], "K", { tone: -1 });
    s.poly([[50, 87], [56, 87], [56, 91], [49, 91]], "R", 1);
    // Far arm reaching down and back, claws.
    s.mass([["c", [[53, 38, 3], [58, 48, 2.4], [57, 57, 2]]]], "K", { tone: -1, seam: true });
    // Torso: chest, narrow waist, the ribs and abdomen.
    s.mass([["e", 47, 42, 8, 7.5], ["c", [[47, 46, 5.2], [48, 56, 4.2]]]], "K", { k: 2, relief: 1.8 });
    // Torn loincloth.
    s.poly([[42, 56], [54, 56], [55, 64], [52, 62], [50, 67], [48, 62], [45, 66], [43, 61]], "H", (x, y) => (y < 58 ? 4 : 3));
    // Near leg: thigh forward, the shin back from the knee, the long foot down to the hoof.
    s.cap([[45, 60, 4], [41, 70, 3]], "K");
    s.ell(41, 70, 2.8, 2.6, "K");
    s.cap([[41, 71, 2.6], [48, 80, 2]], "K");
    s.cap([[48, 80, 1.9], [45, 88, 1.7]], "K");
    s.poly([[41, 87], [48, 87], [48, 91], [40, 91]], "R", (x) => (x < 44 ? 3 : 2));
    // Head: a long skull, the jaw jutting forward.
    s.mass([["e", 44, 27, 5.5, 6], ["c", [[41, 31, 3], [38, 32, 2.2]]]], "K", { k: 1.3, seam: true });
    // Ram horns curling back and down around the ears.
    s.cap([[46, 22, 2.4], [52, 18, 2.2], [57, 21, 1.8], [56, 27, 1.4], [52, 27, 1]], "R", {});
    s.cap([[42, 22, 1.8], [44, 15, 1.4], [49, 12, 1], [51, 15, 0.7]], "R", { tone: -1 });
    // Near arm forward, claws spread.
    s.mass([["c", [[41, 38, 3.2], [36, 46, 2.6], [29, 48, 2.2]]]], "K", { seam: true });
    s.seams();

    // Claws.
    for (const [dx, dy] of [[-5, -3], [-6, 0], [-5, 3], [-2, 4]]) { s.line(27, 48, 27 + dx, 48 + dy, "K", 3); s.px(27 + dx, 48 + dy, "R", 5); }
    for (const [dx, dy] of [[-2, 4], [0, 5], [2, 4]]) s.px(57 + dx, 57 + dy, "R", 3);
    // Ribs and abdomen lines, the collarbone.
    for (const y of [43, 46]) s.line(43, y, 49, y + 1, "K", 1);
    s.line(46, 50, 46, 55, "K", 1); s.line(43, 52, 49, 52, "K", 1);
    s.line(40, 36, 50, 35, "K", 4, false);
    // Face: a heavy brow, burning eyes, a fanged grin.
    s.line(38, 25, 45, 24, "K", 1);
    s.px(39, 26, "Y", 4); s.px(40, 26, "Y", 5); s.px(44, 26, "Y", 3);
    s.patch(37, 31, [
      "4m4m4m.",
      "mmmmmm.",
      ".4..4..",
    ]);
    s.px(37, 29, "D", 1); // nostril
    // Horn ridges.
    for (const [x, y] of [[50, 18], [54, 19], [56, 23]]) s.px(x, y, "R", 1);
  },
};
