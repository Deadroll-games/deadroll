// Dark knight, 96, facing left: undead in blackened plate (K) on the human canon, spiked
// pauldrons, a horned helm with a glowing visor slit (Y), a torn crimson cape (H), a greatsword
// (R) held two-handed, raised forward. Feet on y = 91.
module.exports = {
  id: 430, name: "dark_knight", size: 96,
  comment: [
    "Dark knight, facing left: blackened plate, spiked pauldrons, a horned helm with a glowing",
    "visor slit, a torn crimson cape, a greatsword raised two-handed. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.62, 0.34, -0.04]);
    // Torn cape behind, flaring back.
    s.poly([[44, 34], [56, 34], [60, 50], [66, 70], [68, 86], [64, 83], [62, 89], [58, 84], [55, 88], [52, 83],
      [49, 86], [48, 70]], "H", (x, y) => (x > 60 ? 1 : 2));
    for (const [a, b] of [[[54, 40], [58, 84]], [[58, 46], [64, 80]]]) s.line(a[0], a[1], b[0], b[1], "H", 0, false);
    // Far leg (darker): greave and sabaton.
    s.mass([["c", [[51, 60, 4], [53, 74, 3.4], [52, 88, 3]]]], "K", { tone: -1, seam: true });
    s.poly([[49, 87], [56, 87], [57, 91], [47, 91]], "K", 1);
    // Far pauldron.
    s.mass([["e", 55, 37, 5, 4.5]], "K", { tone: -1, seam: true });
    // Breastplate and the faulds over the hips.
    s.mass([["e", 47, 45, 10, 11], ["e", 47, 56, 8, 5]], "K", { k: 2, relief: 1.8 });
    s.poly([[39, 56], [55, 56], [56, 64], [38, 64]], "K", (x, y) => (y < 59 ? 3 : 2));
    for (const y of [59, 62]) s.line(38, y, 56, y, "K", 0);
    // Near leg: thigh plate, knee cop, greave, sabaton.
    s.mass([["c", [[44, 62, 4.4], [42, 74, 3.8], [43, 88, 3.4]]]], "K", { seam: true });
    s.ell(42, 74, 3.6, 3, "K", { seam: true });
    s.poly([[37, 87], [46, 87], [47, 91], [35, 91]], "K", (x) => (x < 40 ? 4 : 3));
    // Helm: a rounded great helm, the face plate forward, horns.
    s.mass([["e", 46, 25, 7, 8], ["c", [[42, 28, 4.5], [39, 29, 3.4]]]], "K", { k: 1.4, seam: true });
    s.cap([[50, 20, 2], [55, 15, 1.4], [56, 9, 0.9]], "K", { tone: -1 });
    s.cap([[43, 20, 2.2], [39, 13, 1.6], [41, 7, 1]], "K", {});
    // Near pauldron with spikes.
    s.mass([["e", 41, 37, 6.5, 5.5]], "K", { seam: true });
    for (const [x, y, dx, dy] of [[38, 33, -2, -5], [42, 32, 0, -6], [46, 33, 2, -4]]) {
      s.poly([[x - 1.4, y], [x + dx, y + dy], [x + 1.4, y]], "K", (px) => (px <= x + dx / 2 ? 5 : 3));
    }
    // Near arm: down to the elbow, forward to the grip; gauntlets.
    s.mass([["c", [[40, 41, 3.2], [37, 50, 2.8], [35, 57, 2.6]]]], "K", { seam: true });
    s.seams();

    // Greatsword: pommel under the hands, the grip, a cross guard, a long blade raised forward.
    s.ell(38, 64, 1.6, 1.6, "R", {});
    s.line(37, 63, 34, 55, "H", 0); s.line(38, 62, 35, 55, "H", 1);
    s.line(29, 54, 38, 51, "R", 3); s.line(29, 55, 38, 52, "R", 1);
    s.poly([[36, 52], [16, 15], [13, 13], [10, 16], [11, 20], [31, 56]], "R", (x, y) => (2 * x - y < 0 ? 3 : 2));
    s.line(10, 17, 30, 55, "R", 5, false);
    s.line(14, 17, 33, 53, "R", 1, false); // the fuller
    // Gauntlets on the grip.
    s.ell(35, 57, 2.6, 2.4, "K", {}); s.ell(36, 60, 2.4, 2.2, "K", { tone: -1 });
    // Visor slit glowing, the breathing holes.
    s.line(39, 26, 45, 26, "Y", 4); s.px(40, 26, "Y", 5); s.px(41, 26, "Y", 5);
    for (const [x, y] of [[40, 30], [42, 30], [41, 32]]) s.px(x, y, "D", 0);
    // Plate edges: a lit rim on the breastplate, rivets.
    s.line(40, 40, 44, 36, "K", 5, false);
    for (const [x, y] of [[44, 48], [50, 48], [47, 53]]) s.px(x, y, "K", 5);
  },
};
