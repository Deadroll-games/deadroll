// Stone gargoyle, 96, facing left: crouched on its haunches, front arms planted between the knees,
// bat wings half open behind, curled horns, a fanged snarl, glowing eyes, cracks in the stone.
// Feet on y = 91.
const wing = require("./bat_wing");
module.exports = {
  id: 429, name: "gargoyle", size: 96,
  comment: [
    "Stone gargoyle, facing left: crouched on its haunches, arms planted between the knees, bat",
    "wings half open behind, curled horns, a fanged snarl, glowing eyes, cracked stone. Feet y = 91.",
  ],
  build(s) {
    s.curve([0.64, 0.34, -0.04]);
    // Bat wings half open behind: the far one darker; stone bones on a stone membrane.
    wing(s, [58, 52], [72, 30], [[82, 18], [90, 28], [90, 42], [82, 54]], [62, 62], -1, { bone: "K", skin: "H" });
    wing(s, [54, 52], [60, 26], [[62, 12], [70, 14], [74, 26], [70, 40]], [58, 58], 0, { bone: "K", skin: "H" });
    // Far leg (haunch and foot) and the far arm, darker.
    s.mass([["e", 62, 78, 8, 9], ["c", [[62, 84, 4], [56, 90, 3]]]], "K", { tone: -1, seam: true });
    s.mass([["c", [[50, 60, 3.6], [44, 74, 3], [44, 88, 2.6]]]], "K", { tone: -1, seam: true });
    // Body: hunched torso leaning forward, the near haunch.
    s.mass([["c", [[58, 70, 9], [52, 58, 9.5], [46, 52, 7]]], ["e", 56, 80, 9, 9]], "K", { k: 2, relief: 1.6 });
    // Near leg: the foot forward, claws.
    s.mass([["c", [[54, 84, 4.5], [46, 89, 3.4]]]], "K", { seam: true });
    // Near arm planted between the knees.
    s.mass([["c", [[44, 56, 4], [38, 70, 3.2], [37, 86, 2.8]]], ["e", 36, 88, 3.6, 2.4]], "K", { seam: true });
    // Head forward and low: skull, muzzle, jaw.
    s.mass([["e", 38, 46, 7, 6.5], ["c", [[34, 50, 3.6], [28, 52, 2.8]]]], "K", { k: 1.6, seam: true });
    // Horns curling back.
    s.cap([[40, 41, 2.2], [44, 34, 1.6], [50, 32, 1.2], [53, 35, 0.8]], "R", {});
    s.cap([[36, 41, 1.8], [36, 35, 1.3], [39, 31, 0.9]], "R", { tone: -1 });
    s.seams();

    // Snarl: dark mouth, fangs; nostril; glowing eyes under a heavy brow.
    s.patch(27, 51, [
      "4m4m4m4m",
      "mmmmmmm.",
      ".4m.4m..",
    ]);
    s.px(27, 49, "D", 1);
    s.line(31, 44, 38, 43, "K", 1);
    s.px(32, 45, "Y", 4); s.px(33, 45, "Y", 5); s.px(37, 45, "Y", 3);
    // Claws.
    for (const [x, y] of [[32, 90], [34, 90], [36, 91], [42, 91], [44, 91]]) s.px(x, y, "R", 3);
    // Cracks and chips in the stone.
    for (const [a, b] of [[[52, 56], [56, 64]], [[56, 64], [55, 70]], [[60, 74], [64, 80]], [[40, 60], [39, 66]]]) s.line(a[0], a[1], b[0], b[1], "K", 0);
    const hash = (x, y) => (((x * 73856093) ^ (y * 19349663)) >>> 0) % 14;
    s.recolor((x, y, c) => (hash(x, y) === 0 && c >= 2 ? c - 1 : c), "K");
  },
};
