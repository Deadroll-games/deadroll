// Lizardman, 96, facing left: a hunched scaled warrior (K swamp scales), a crocodile head full of
// teeth, yellow eyes (Y gold), a heavy tail, a bone crest, a hide loincloth (H), a spear with a
// bone head (R). Feet on y = 91.
module.exports = {
  id: 456, name: "lizardman", size: 96,
  comment: [
    "Lizardman, facing left: a hunched scaled warrior with a crocodile head full of teeth, yellow",
    "eyes, a heavy tail, a bone crest, a hide loincloth, a bone-tipped spear. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.62, 0.34, -0.04]);
    // Tail sweeping back along the ground, far leg and far arm (darker).
    s.cap([[54, 64, 5], [66, 74, 4], [78, 84, 2.8], [88, 88, 1.4]], "K", { tone: -1 });
    s.cap([[52, 66, 3.6], [55, 77, 2.6], [52, 88, 2.2]], "K", { tone: -1 });
    s.poly([[48, 88], [56, 88], [57, 91], [46, 91]], "K", 1);
    s.cap([[54, 42, 2.8], [58, 52, 2.3], [56, 60, 2]], "K", { tone: -1 });
    // Torso hunched forward, the belly plates.
    s.mass([["c", [[52, 44, 7], [50, 56, 6.5], [50, 66, 5.6]]], ["e", 46, 46, 6, 6]], "K", { k: 2, relief: 1.8 });
    // Loincloth of hide.
    s.poly([[44, 63], [56, 62], [57, 72], [54, 70], [51, 74], [48, 70], [45, 72]], "H", (x, y) => (y < 65 ? 3 : 2));
    // Near leg: thigh, the shin back, a clawed foot.
    s.cap([[47, 66, 4.2], [43, 76, 3], [45, 88, 2.4]], "K");
    s.ell(43, 76, 2.8, 2.4, "K");
    s.poly([[38, 88], [47, 88], [47, 91], [36, 91]], "K", (x) => (x < 41 ? 4 : 3));
    // Head: a long crocodile head forward, the jaw.
    s.mass([["e", 42, 36, 5.5, 5], ["c", [[38, 37, 4], [26, 39, 2.8]]]], "K", { k: 1.4, seam: true });
    s.mass([["c", [[38, 42, 3], [27, 43, 2]]]], "K", { tone: -1, seam: true });
    // Near arm forward, gripping the spear.
    s.cap([[44, 44, 3], [38, 52, 2.4], [32, 54, 2.2]], "K", { seam: true });
    s.seams();

    // Spear: shaft through the fist, a bone head forward.
    s.line(46, 62, 16, 46, "H", 2); s.line(46, 61, 16, 45, "H", 4);
    s.poly([[17, 45], [8, 42], [6, 44], [9, 47], [17, 48]], "R", (x, y) => (y < 45 ? 4 : 2));
    s.line(18, 44, 20, 49, "H", 1); s.line(19, 44, 21, 49, "H", 1); // binding
    s.ell(32, 54, 2.4, 2.2, "K", {});
    // Teeth along both jaws, the eye, the nostril, the bone crest on the head and back.
    for (let x = 27; x < 38; x += 2) { s.px(x, 40, "R", 5); s.px(x + 1, 41, "R", 4); }
    s.line(26, 40, 38, 41, "D", 0);
    s.px(41, 34, "Y", 4); s.px(42, 34, "Y", 5); s.px(42, 33, "D", 0);
    s.line(38, 32, 45, 31, "K", 1);
    s.px(25, 37, "D", 1);
    for (const [x, y, h] of [[45, 31, 4], [49, 33, 5], [53, 37, 5], [56, 42, 4], [57, 48, 3]]) s.poly([[x - 1.4, y + 1], [x + 1, y - h], [x + 1.6, y + 1]], "R", 3);
    // Scales: a lattice on the back, lighter belly plates.
    for (let y = 44; y < 66; y += 3) for (let x = 46; x < 60; x += 3) { const p = s.at(x + ((y / 3) % 2), y); if (p?.slot === "K" && p.c >= 2) p.c -= 1; }
    for (const y of [48, 52, 56, 60]) s.line(44, y, 48, y, "K", 4);
  },
};
