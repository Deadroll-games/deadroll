// Ghoul, 96, facing left: a gaunt rotting corpse, the spine bent in a C so the shoulders and head
// hang in front of the hips, skin stretched over the ribs, long arms reaching forward with claws,
// a hanging jaw, stringy hair, rags, long thin bent legs. Feet on y = 91.
module.exports = {
  id: 424, name: "ghoul", size: 96,
  comment: [
    "Ghoul, facing left: gaunt rotting corpse, the spine bent forward, ribs under the skin, long",
    "arms reaching with claws, a hanging jaw, stringy hair, rags, thin bent legs. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.62, 0.32, -0.04]);
    // Far arm reaching (darker) and the far leg.
    s.cap([[50, 41, 2], [40, 49, 1.7], [28, 52, 1.5]], "K", { tone: -1 });
    s.cap([[58, 62, 2.6], [61, 76, 2], [58, 89, 1.6]], "K", { tone: -1 });
    s.poly([[55, 88], [60, 88], [61, 91], [53, 91]], "K", 1);
    // Torso bent in a C: hips low at the back, the chest and shoulders forward.
    s.mass([["c", [[57, 62, 5], [59, 53, 5.5], [56, 45, 6], [49, 40, 5.5]]]], "K", { k: 2, relief: 1.6 });
    // Near leg: thin, the knee bent forward, a bony foot.
    s.cap([[55, 63, 2.8], [49, 76, 2.1]], "K");
    s.ell(49, 77, 2.4, 2.1, "K");
    s.cap([[49, 78, 1.8], [52, 89, 1.6]], "K");
    s.poly([[46, 88], [54, 88], [54, 91], [45, 91]], "K", (x) => (x < 49 ? 4 : 3));
    // Rags at the hips.
    s.poly([[52, 60], [63, 60], [63, 69], [60, 67], [58, 71], [55, 67], [53, 70]], "H", (x, y) => (y < 62 ? 3 : 2));
    // Head low in front of the shoulders, the jaw hanging open.
    s.mass([["e", 39, 41, 5.6, 5.4], ["c", [[36, 44, 2.6], [33, 45, 2]]]], "K", { k: 1.5, seam: true });
    s.cap([[39, 49, 1.8], [34, 51, 1.4]], "K", { tone: -1, seam: true });
    // Near arm reaching forward from the shoulder, long bony fingers.
    s.cap([[48, 43, 2.4], [36, 51, 1.9], [22, 52, 1.6]], "K", { seam: true });
    s.seams();

    // Claws.
    for (const [dx, dy] of [[-6, -3], [-7, 0], [-6, 3], [-3, 4]]) { s.line(21, 52, 21 + dx, 52 + dy, "K", 3); s.px(21 + dx, 52 + dy, "R", 4); }
    for (const [dx, dy] of [[-5, -2], [-6, 1], [-4, 3]]) { s.line(27, 53, 27 + dx, 53 + dy, "K", 1); s.px(27 + dx, 53 + dy, "R", 2); }
    // Ribs under the skin on the flank, the spine ridge along the back.
    for (const y of [45, 48, 51, 54]) for (let i = 0; i < 7; i++) {
      const x = 58 - i, yy = y + Math.round((i * i) / 16);
      const p = s.at(x, yy), q = s.at(x, yy + 1);
      if (p && p.slot === "K") p.c = 4;
      if (q && q.slot === "K") q.c = 1;
    }
    for (const [x, y] of [[54, 38], [58, 41], [61, 45], [63, 50], [63, 55]]) { s.px(x, y, "K", 5); s.px(x + 1, y + 1, "K", 1); }
    // Face: sunken socket with a cold glow, nose hole, the hanging jaw with teeth.
    s.patch(34, 38, [
      ".mmm.",
      "mnklm",
      ".mmm.",
      ".....",
      "m....",
    ]);
    s.patch(33, 46, [
      "4m4m4m.",
      "mmmmmmm",
      "mmmmmm.",
      ".4m4m..",
    ]);
    // Stringy hair from the scalp down the back of the head.
    for (const [x, l] of [[42, 13], [44, 15], [46, 11], [40, 8]]) for (let i = 0; i < l; i++) s.px(x + (i >> 2), 36 + i, "H", i < 3 ? 4 : i < l - 3 ? 3 : 2);
    // Rot blotches.
    const hash = (x, y) => (((x * 73856093) ^ (y * 19349663)) >>> 0) % 12;
    s.recolor((x, y, c) => (hash(x >> 1, y >> 1) === 0 && c >= 2 ? c - 1 : c), "K");
    for (const x of [46, 48, 54]) s.px(x, 91, "R", 3);
  },
};
