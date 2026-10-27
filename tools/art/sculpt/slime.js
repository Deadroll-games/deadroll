// Acid slime, 96: a heavy dark dome of ooze sagging into a puddle, a skull and ribs half dissolved
// inside (H = bone, seen through the ooze), bubbles and a wet highlight. Rim on y = 91.
module.exports = {
  id: 426, name: "slime", size: 96,
  comment: [
    "Acid slime: a heavy dark dome of ooze sagging into a puddle, a skull and ribs half dissolved",
    "inside, bubbles, a wet highlight, acid fumes. Rim on y = 91.",
  ],
  build(s) {
    s.curve([0.76, 0.52, 0.14]);
    // The dome sagging into a wide puddle; the top leans forward (left).
    s.mass([["e", 46, 78, 24, 13], ["e", 42, 70, 15, 13], ["e", 48, 88, 30, 4]], "K", { k: 3, relief: 1.4 });
    for (let x = 0; x < 96; x++) for (let y = 92; y < 96; y++) s.erase(x, y);
    // A skull inside, seen through the ooze.
    s.patch(35, 64, [
      "...bccb...",
      "..bcccccb.",
      ".bccccccb.",
      ".bmmcbmmb.",
      ".bmmcbmmb.",
      "..bcmcbb..",
      "..bcbcb...",
      "..b.b.b...",
    ]);
    // A thigh bone sinking deeper (darker), knobbed ends.
    s.line(54, 80, 64, 85, "H", 2); s.line(54, 81, 64, 86, "H", 1);
    s.ell(53.5, 80, 1.6, 1.6, "H", { tone: -1 }); s.ell(64.5, 86, 1.6, 1.6, "H", { tone: -1 });
    // Wet highlight along the lit top, bubbles.
    s.line(33, 60, 41, 57, "K", 5); s.line(32, 62, 34, 61, "K", 4);
    for (const [x, y, r] of [[56, 70, 1.6], [62, 76, 1.2], [42, 84, 1.4], [28, 78, 1.1], [66, 84, 1]]) {
      s.ell(x, y, r, r, "K", { tone: 1 }); s.px(Math.round(x - 0.5), Math.round(y - 0.5), "K", 5);
    }
    // Acid fumes rising.
    for (const [x, y] of [[38, 52], [44, 49], [52, 53], [41, 46]]) s.px(x, y, "K", 2);
  },
};
