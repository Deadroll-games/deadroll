// Cave troll, 2x2 on 128, facing left: a hunched giant, a small head sunk below the shoulders, an
// underbite with tusks, long arms (the near one drags a log club, the far one on its knuckles), a
// pot belly, short thick legs. Feet on y = 123.
module.exports = {
  id: 409, name: "troll", size: 128,
  comment: [
    "Cave troll, a 2x2 tank, facing left: hunched, a small head sunk under huge shoulders, tusks",
    "in an underbite, a log club studded with bone dragged in the near hand, the far hand on its",
    "knuckles, pot belly, ragged loincloth. Feet on y = 123.",
  ],
  build(s) {
    s.curve([0.64, 0.34, -0.04]);
    // Far leg and far arm (on its knuckles), darker.
    s.mass([["c", [[76, 96, 9.5], [77, 110, 8.5], [77, 118, 8]]], ["e", 73, 120.5, 11, 3]], "K", { tone: -1, seam: true });
    s.mass([["c", [[84, 60, 10], [98, 78, 7.5], [95, 98, 6.5]]], ["e", 94, 105, 8, 6.5]], "K", { tone: -1, seam: true });
    // One body: hunched back and shoulders, chest, pot belly, hips, the head low in front.
    s.mass([
      ["e", 70, 56, 23, 15],
      ["c", [[62, 66, 19], [70, 80, 20], [68, 92, 16]]],
      ["e", 54, 86, 16, 15],
    ], "K", { k: 3, relief: 2 });
    // Head low and forward with its own outline: skull, heavy brow, the jutting lower jaw.
    s.mass([["e", 37, 59, 10, 9], ["c", [[42, 53, 4], [29, 54, 4]]], ["e", 32, 68, 10, 6.5]], "K", { k: 2, relief: 1.6, seam: true });
    // Near leg: thick and short, a flat foot.
    s.mass([["c", [[58, 98, 10.5], [56, 110, 9.5], [55, 118, 9]]], ["e", 51, 120.5, 13, 3]], "K", { seam: true });
    // Loincloth with a ragged hem.
    s.poly([[50, 98], [60, 95], [72, 94], [81, 95], [81, 105], [77, 103], [74, 108], [70, 103], [66, 109],
      [62, 103], [57, 108], [53, 103], [50, 105]], "H", (x, y) => (y < 98 ? 3 : x > 72 ? 1 : 2));
    // Near arm: a long arm hanging forward, the fist around the club.
    s.mass([["c", [[58, 66, 10], [46, 82, 8], [34, 92, 7]]], ["e", 31, 94, 7, 6.5]], "K", { seam: true });
    s.seams();

    // Log club: from the fist down to the ground ahead, bark, rope bands, bone spikes.
    s.cap([[30, 98, 4.5], [22, 108, 7], [16, 117, 8.5]], "H");
    for (const [x, y] of [[26, 103], [19, 112]]) { s.line(x - 6, y + 4, x + 4, y - 3, "H", 1); s.line(x - 5, y + 4, x + 5, y - 3, "H", 4); }
    for (const [x, y, dx, dy] of [[12, 112, -4, -2], [16, 106, -3, -4], [24, 114, 2, 4], [11, 119, -4, 1], [21, 101, -3, -3]]) {
      s.line(x, y, x + dx, y + dy, "R", 4); s.px(x + dx, y + dy, "R", 5);
    }
    // The fist over the haft.
    s.ell(31, 94, 5.5, 5, "K", { seam: false });

    // Warty hide.
    const hash = (x, y) => (((x * 73856093) ^ (y * 19349663)) >>> 0) % 11;
    s.recolor((x, y, c) => (hash(x >> 1, y >> 1) === 0 && c >= 2 ? c - 1 : c), "K");
    // Belly button and belly folds, chest crease, back knuckles.
    s.line(46, 92, 60, 94, "K", 1);
    s.px(52, 88, "D", 1);
    for (const x of [89, 92, 95]) s.px(x, 111, "R", 3);
    // Face: heavy brow, two small glowing eyes under it, a broad short nose, the jutting jaw
    // with a mouth line and two tusks rising from the lower lip.
    s.line(28, 55, 44, 54, "K", 4); s.line(28, 56, 44, 55, "K", 1);
    s.px(32, 58, "Y", 4); s.px(33, 58, "Y", 5); s.px(38, 58, "Y", 3); s.px(39, 58, "Y", 2);
    s.px(32, 57, "D", 1); s.px(38, 57, "D", 1);
    s.mass([["e", 27, 62, 3.5, 3]], "K", {});
    s.px(25, 63, "D", 1); s.px(27, 64, "D", 1);
    s.line(24, 69, 40, 70, "D", 1); s.line(25, 70, 39, 71, "D", 2);
    s.line(27, 69, 27, 65, "R", 4); s.px(27, 64, "R", 5); s.px(28, 68, "R", 3);
    s.line(35, 70, 35, 66, "R", 4); s.px(35, 65, "R", 5); s.px(36, 69, "R", 3);
    // Scars and hair strands on the hump.
    s.line(64, 58, 72, 64, "K", 5);
    // Toes.
    for (const x of [40, 43, 46, 62, 65]) s.px(x, 123, "R", 3);
  },
};
