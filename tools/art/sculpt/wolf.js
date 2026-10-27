// Dire wolf, facing left, stalking, head low, snarling. Paws on y = 91.
module.exports = {
  id: 400, name: "wolf", size: 96,
  comment: [
    "Dire wolf of the caves, facing left: stalking, head low, snarling, a ragged ruff, a scar on",
    "the shoulder, far legs darker. Paws on y = 91.",
  ],
  build(s) {
    s.curve([0.62, 0.32, -0.05]);
    // Far legs (darker): front reaching back, hind under the body.
    s.cap([[44, 60, 4], [44, 72, 2.6], [43, 80, 2.2], [42, 88, 2]], "K", { tone: -1 });
    s.ell(40, 89.6, 3.2, 1.8, "K", { tone: -1 });
    s.cap([[66, 58, 5.5], [69, 69, 3.2], [72, 76, 2.4], [68, 88, 2]], "K", { tone: -1 });
    s.ell(67, 89.6, 3.2, 1.8, "K", { tone: -1 });
    // Tail, low and bushy.
    s.cap([[79, 49, 3], [87, 52, 3.6], [91, 61, 4.2], [90, 72, 2.6], [88, 77, 1.2]], "H", { tone: -1 });
    // Body: haunch, short barrel, deep chest.
    s.ell(70, 53, 10, 9.5, "K");
    s.ell(57, 52, 13, 8, "K");
    s.ell(42, 54, 12, 12, "K");
    // Near legs: front stepping forward (elbow, wrist, paw), hind with a clear hock.
    s.cap([[37, 59, 5], [35, 71, 3], [34, 80, 2.4], [33, 88, 2.2]], "K");
    s.ell(31, 89.6, 3.8, 2, "K");
    s.ell(73, 59, 7, 9, "K");
    s.cap([[75, 64, 4.6], [78, 72, 3], [80, 77, 2.4], [76, 88, 2.1]], "K");
    s.ell(74, 89.6, 3.8, 2, "K");
    // Ruff: a mane over the neck and shoulders with ragged tufts.
    s.ell(36, 47, 13, 11, "H", { ragged: 0.3, tufts: 16 });
    // Head: skull, stop, tapering muzzle, open lower jaw.
    s.ell(22, 50, 9.5, 8, "K");
    s.cap([[17, 53, 5], [5, 57, 3]], "K");
    s.cap([[18, 57, 2.6], [12, 59.5, 2.2], [7, 61, 1.8]], "K", { tone: -1 });
    // Ears, big, upright and laid back; the far one darker.
    s.poly([[25, 44], [32, 33], [33, 46]], "K", 1);
    s.poly([[19, 45], [23, 31], [29, 44]], "K", (x) => (x < 23 ? 4 : 3));
    s.poly([[22, 44], [23, 36], [26, 43]], "D", 2);
    s.seams();

    // Fur strands: short darker strokes; lighter tufts on the mane.
    s.recolor((x, y, c) => ((x + 2 * y) % 6 === 0 && c >= 2 && c <= 4 ? c - 1 : c), "K");
    s.recolor((x, y, c) => ((2 * x + y) % 5 === 0 && c >= 2 && c <= 3 ? c + 1 : c), "H");
    // Ribs on the flank.
    for (const rx of [52, 57, 62]) for (let y = 50; y <= 58; y++) {
      const x = rx + Math.round(((y - 54) * (y - 54)) / 9);
      const p = s.at(x, y);
      if (p && p.slot === "K") p.c = Math.max(1, p.c - 1);
    }
    // An old scar across the shoulder.
    s.line(44, 50, 49, 56, "K", 5);

    // Snarl: dark gap, teeth, the nose.
    // The gap rises toward the cheek.
    const gy = (x) => 59 - Math.round((x - 6) / 6);
    for (let x = 6; x <= 19; x++) { s.px(x, gy(x), "D", 1); if (x >= 8) s.px(x, gy(x) - 1, "D", 2); }
    for (const x of [7, 10, 13, 16]) s.px(x, gy(x) - 1, "R", 5);
    for (const x of [8, 11, 14]) s.px(x, gy(x) + 1, "R", 4);
    s.px(3, 56, "D", 2); s.px(3, 55, "D", 1); s.px(4, 55, "D", 1);
    s.line(7, 54, 15, 51, "K", 4); // lit top of the muzzle
    // Brow and the slanted glowing eyes.
    s.line(15, 46, 22, 45, "K", 1);
    s.px(16, 47, "Y", 3); s.px(17, 47, "Y", 5); s.px(18, 46, "Y", 4);
    s.px(17, 48, "D", 1);
    // Claws.
    for (const [x, y] of [[28, 91], [30, 91], [71, 91], [73, 91], [38, 91], [65, 91]]) s.px(x, y, "R", 3);
  },
};
