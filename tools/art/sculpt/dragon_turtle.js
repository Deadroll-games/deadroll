// Dragon turtle, 2x2 on 128, facing left: a squat swamp-scaled beast (K) under a high domed shell of
// stone plates (H) with bone spikes (R), a scaled neck stretching forward to a horned dragon head
// with glowing eyes (Y), pillar legs with claws, a short tail. Feet on y = 123.
const rock = require("./rock");
module.exports = {
  id: 445, name: "dragon_turtle", size: 128,
  comment: [
    "Dragon turtle, a 2x2 beast, facing left: a domed shell of stone plates with bone spikes, a",
    "scaled neck stretching to a horned dragon head, glowing eyes, pillar legs. Feet on y = 123.",
  ],
  build(s) {
    s.curve([0.66, 0.4, 0.02]);
    // Far legs and the tail (darker).
    s.mass([["c", [[56, 100, 8], [54, 116, 7]]], ["e", 51, 120, 9, 3]], "K", { tone: -1, seam: true });
    s.mass([["c", [[94, 100, 8], [96, 116, 7]]], ["e", 93, 120, 9, 3]], "K", { tone: -1, seam: true });
    s.cap([[108, 100, 5], [118, 104, 3], [123, 110, 1.5]], "K", { tone: -1 });
    // Belly under the shell, the shell dome of plates.
    s.mass([["e", 76, 100, 30, 10]], "K", { k: 2 });
    s.mass([["e", 76, 82, 34, 24]], "H", { k: 3, relief: 1.6 });
    // Shell plates: a grid of faceted scutes over the dome, a rim band.
    for (const [x, y, w, h, seed] of [[60, 72, 9, 8, 1], [76, 66, 10, 9, 2], [92, 72, 9, 8, 3], [52, 88, 8, 7, 4],
      [68, 84, 9, 8, 5], [84, 84, 9, 8, 6], [100, 88, 8, 7, 7]]) rock(s, x, y, w, h, "H", { seed, sides: 6 });
    // Dark seams between the scutes, then the rim band following the lower edge of the dome.
    for (let y = 1; y < 127; y++) for (let x = 1; x < 127; x++) {
      const p = s.at(x, y);
      if (!p || p.slot !== "H") continue;
      const q = s.at(x + 1, y), b = s.at(x, y + 1);
      if ((q?.slot === "H" && q.sid !== p.sid) || (b?.slot === "H" && b.sid !== p.sid)) p.c = 0;
    }
    for (let x = 42; x < 112; x++) {
      let bot = -1;
      for (let y = 60; y < 110; y++) if (s.at(x, y)?.slot === "H") bot = y;
      if (bot < 0) continue;
      s.px(x, bot, "H", 1); s.px(x, bot - 1, "H", 4); s.px(x, bot - 2, "H", 3);
    }
    // Bone spikes along the crest of the shell.
    for (const [x, h] of [[56, 8], [66, 11], [76, 13], [86, 11], [96, 8], [104, 5]]) {
      const top = [...Array(128).keys()].find((y) => s.at(x, y)?.slot === "H");
      if (top !== undefined) s.poly([[x - 2.6, top + 4], [x + 1, top - h], [x + 2.6, top + 4]], "R", (px) => (px < x ? 4 : 2));
    }
    // Near legs: thick pillars with claws.
    s.mass([["c", [[50, 102, 9], [46, 116, 8]]], ["e", 43, 120, 10.5, 3]], "K", { seam: true });
    s.mass([["c", [[100, 102, 9], [102, 116, 8]]], ["e", 100, 120, 10, 3]], "K", { seam: true });
    // Neck forward, the dragon head.
    s.mass([["c", [[50, 92, 9], [36, 84, 7], [28, 76, 6]]]], "K", { k: 2, relief: 1.8, seam: true });
    s.mass([["e", 24, 70, 8, 6.5], ["c", [[18, 73, 4.6], [9, 76, 3.2]]]], "K", { k: 1.4, seam: true });
    s.mass([["c", [[18, 80, 3], [10, 81, 2]]]], "K", { tone: -1, seam: true });
    s.cap([[27, 65, 2.2], [34, 58, 1.6], [40, 56, 0.8]], "R", {});
    s.cap([[23, 65, 1.8], [24, 57, 1.2], [28, 52, 0.6]], "R", { tone: -1 });
    s.seams();

    // Face: eye, nostril, jaw with fangs.
    s.line(18, 66, 27, 65, "K", 1);
    s.px(21, 68, "Y", 4); s.px(22, 68, "Y", 5);
    s.px(8, 74, "D", 1);
    s.line(9, 78, 22, 78, "D", 0);
    for (const x of [10, 13, 16, 19]) s.px(x, 77, "R", 5);
    for (const x of [11, 14, 17]) s.px(x, 79, "R", 4);
    // Neck scales: a belly band under the neck.
    for (let k = 0; k < 18; k += 2) { const x = 46 - k * 0.9 | 0, y = 96 - k * 0.8 | 0; if (s.at(x, y)?.slot === "K") s.px(x, y, "K", 4); }
    for (const x of [36, 39, 42, 46, 96, 99, 102]) s.px(x, 123, "R", 3);
  },
};
