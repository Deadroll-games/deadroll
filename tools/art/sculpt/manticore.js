// Manticore, 2x2 on 128, facing left: a lion body (K brown), a thick dark mane (H) around a
// near-human face with a maw of fangs (R bone), leathery bat wings, a segmented scorpion tail arched
// over the back with a sting (R bone, venom in Y). Feet on y = 123.
const wing = require("./bat_wing");
module.exports = {
  id: 444, name: "manticore", size: 128,
  comment: [
    "Manticore, a 2x2 beast, facing left: a lion body, a dark mane around a near-human face with a",
    "maw of fangs, leathery bat wings, a scorpion tail arched over the back with a sting.",
  ],
  build(s) {
    s.curve([0.64, 0.36, -0.02]);
    // Scorpion tail: segments rising from the rump, arching forward over the back, the sting.
    const seg = [[100, 82, 5.5], [112, 76, 5], [120, 64, 4.5], [122, 50, 4], [118, 38, 3.6], [110, 30, 3.2], [100, 28, 2.8]];
    s.cap(seg, "R", { tone: -1 });
    // Segment rings across the tail.
    for (let i = 0; i + 1 < seg.length; i++) for (const t of [0.5]) {
      const [a, b] = [seg[i], seg[i + 1]], x = a[0] + (b[0] - a[0]) * t, y = a[1] + (b[1] - a[1]) * t;
      const nx = -(b[1] - a[1]), ny = b[0] - a[0], n = Math.hypot(nx, ny), w = a[2];
      s.line(Math.round(x - (nx / n) * w), Math.round(y - (ny / n) * w), Math.round(x + (nx / n) * w), Math.round(y + (ny / n) * w), "R", 0, false);
    }
    // Far wing (darker), far legs.
    wing(s, [74, 62], [86, 36], [[92, 16], [104, 20], [108, 34], [100, 48]], [80, 70], -1, { bone: "K", skin: "H", arm: 2.6 });
    s.mass([["c", [[58, 92, 7.5], [56, 106, 6], [56, 117, 5]]], ["e", 53, 120, 7.5, 3]], "K", { tone: -1, seam: true });
    s.mass([["c", [[94, 92, 8], [98, 106, 6], [94, 117, 5]]], ["e", 92, 120, 7, 3]], "K", { tone: -1, seam: true });
    // Lion body.
    s.mass([["e", 78, 86, 22, 13], ["e", 98, 84, 11, 11], ["e", 58, 82, 13, 14]], "K", { k: 3, relief: 1.8 });
    // Near legs.
    s.mass([["c", [[54, 92, 8], [48, 106, 6], [47, 117, 5]]], ["e", 44, 120, 8, 3]], "K", { seam: true });
    s.mass([["e", 96, 92, 10, 11], ["c", [[100, 100, 7], [96, 110, 5.5], [100, 117, 4.6]]], ["e", 98, 120, 7, 3]], "K", { seam: true });
    // Mane around the head, then the face.
    s.ell(46, 66, 16, 17, "H", { ragged: 0.35, tufts: 22 });
    s.mass([["e", 40, 66, 9, 10.5], ["e", 38, 74, 6.5, 4.5]], "K", { k: 1.6, seam: true });
    // Near wing, raised.
    wing(s, [66, 66], [74, 32], [[72, 10], [86, 6], [98, 16], [98, 34], [88, 48]], [72, 72], 0, { bone: "K", skin: "H", arm: 2.8 });
    s.seams();

    // Face: a heavy brow, sunken burning eyes, a broad nose, the wide maw with rows of fangs.
    s.patch(31, 59, [
      "..11111111.",
      ".1jlk11kj1.",
      "..1111111..",
      "....22.....",
      "...m22m....",
      "..........",
      "xwxwxwxwx..",
      "mmmmmmmmm..",
      "nnnnnnnnn..",
      "wxwxwxwx...",
    ]);
    // Talons and paws.
    for (const x of [38, 41, 44, 92, 95, 98]) s.px(x, 123, "R", 3);
    // Mane locks over the shoulders.
    for (const [x, y, l] of [[54, 76, 6], [58, 72, 5], [50, 80, 5], [60, 64, 5]]) for (let i = 0; i < l; i++) s.px(x + (i >> 1), y + i, "H", i < 2 ? 3 : 1);
    // The sting curving down and forward from the tip.
    s.poly([[100, 25], [92, 28], [86, 34], [92, 31], [100, 31]], "R", (x) => (x < 94 ? 4 : 2));
    s.px(87, 33, "Y", 5); s.px(88, 33, "Y", 3); s.px(86, 35, "Y", 2);
  },
};
