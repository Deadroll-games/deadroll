// Kitsune, 96, facing left: a pale nine-tailed fox (K) crouched to spring, nine tails fanned
// behind with dark tips (H), red markings around burning eyes (Y), ghostly foxfire flames
// floating around it (R). Paws on y = 91.
module.exports = {
  id: 454, name: "kitsune", size: 96,
  comment: [
    "Kitsune, facing left: a pale nine-tailed fox crouched to spring, nine tails fanned behind with",
    "dark tips, red markings around burning eyes, ghostly foxfire floating around. Paws on y = 91.",
  ],
  build(s) {
    s.curve([0.66, 0.4, 0.02]);
    // Nine tails fanned from the rump, back ones darker; each bushy, with a dark tip.
    // Each tail arcs up and back from the rump and curls at the tip like a flame; the last third
    // darkens to the tip (H).
    for (let i = 0; i < 9; i++) {
      const a = -Math.PI * 0.08 - (i / 8) * Math.PI * 0.62, len = 28 + ((i * 5) % 3) * 3, tone = i % 2 ? -1 : 0;
      const P = (f, bend) => [68 + Math.cos(a) * len * f + Math.cos(a - Math.PI / 2) * bend, 64 + Math.sin(a) * len * f + Math.sin(a - Math.PI / 2) * bend];
      const pts = [[...P(0, 0), 3], [...P(0.35, 2), 5], [...P(0.65, 3), 4.6], [...P(0.85, 1), 2.8], [...P(1, -3), 0.8]];
      s.cap(pts.slice(0, 4), "K", { tone });
      s.cap(pts.slice(2), "H", { tone: tone + 1 });
    }
    // Far legs (darker).
    s.cap([[48, 76, 2.6], [42, 84, 1.8], [44, 89, 1.6]], "K", { tone: -1 });
    s.cap([[62, 70, 3.6], [68, 78, 2.2], [62, 89, 1.6]], "K", { tone: -1 });
    // Body crouched low, the chest ruff, the neck forward.
    s.mass([["c", [[62, 68, 7], [50, 74, 6.5]]], ["e", 44, 76, 6.5, 6.5, { ragged: 0.3, tufts: 12 }]], "K", { k: 2, relief: 1.8 });
    // Head low and forward: a narrow fox head, long ears laid back.
    s.mass([["e", 36, 70, 6, 5], ["c", [[32, 73, 3.2], [24, 76, 1.8]]]], "K", { k: 1.3, seam: true });
    s.poly([[37, 66], [42, 57], [43, 67]], "K", (x) => (x < 40 ? 4 : 2));
    s.poly([[41, 66], [47, 58], [46, 68]], "K", 1);
    s.line(39, 64, 41, 60, "H", 1);
    // Near legs: crouched, ready to spring.
    s.cap([[44, 78, 2.8], [36, 84, 2], [34, 89, 1.7]], "K", { seam: true });
    s.ell(32, 90, 2.8, 1.4, "K", { seam: true });
    s.cap([[66, 72, 5], [72, 80, 2.8], [68, 89, 1.8]], "K", { seam: true });
    s.ell(66, 90, 2.8, 1.4, "K", { seam: true });
    s.seams();

    // Face: red markings sweeping back from the eyes, the burning eye, a sly snarl.
    s.line(31, 69, 39, 67, "Y", 2); s.line(32, 71, 38, 72, "Y", 2);
    s.px(33, 69, "Y", 4); s.px(34, 69, "Y", 5);
    s.px(23, 75, "D", 1);
    s.line(25, 77, 33, 76, "D", 1);
    s.px(27, 76, "K", 5); s.px(30, 76, "K", 5);
    // Foxfire: ghostly flames floating around.
    const fire = (x, y) => s.patch(x - 2, y - 6, [
      "...x.",
      "..x..",
      "..wx.",
      ".xwx.",
      ".wvw.",
      "xwvwx",
      ".wvw.",
      "..v..",
    ]);
    fire(20, 44); fire(30, 34); fire(84, 30); fire(16, 76);
  },
};
