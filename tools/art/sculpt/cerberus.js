// Cerberus, 2x2 on 128, facing left: a massive black mastiff with three heads on fanning necks
// (the near one low and forward, the far one high and darker), spiked iron collars (R steel),
// snarling jaws, burning eyes (Y). Paws on y = 123.
module.exports = {
  id: 435, name: "cerberus", size: 128,
  comment: [
    "Cerberus, a 2x2 beast, facing left: a massive black mastiff with three snarling heads on",
    "fanning necks, spiked iron collars, burning eyes, thick legs under chest and hips. Paws y = 123.",
  ],
  build(s) {
    s.curve([0.6, 0.32, -0.06]);
    // One head: skull, muzzle, open jaw hinged high at the cheek, ears; then face details.
    const head = (x, y, tone) => {
      s.mass([["e", x, y, 9, 8], ["c", [[x - 6, y + 3, 5.4], [x - 15, y + 5, 4]]]], "K", { k: 1.6, seam: true, tone });
      s.mass([["c", [[x - 4, y + 9, 3.4], [x - 13, y + 11, 2.6]]]], "K", { seam: true, tone: tone - 1 });
      s.poly([[x + 1, y - 6], [x + 6, y - 15], [x + 7, y - 4]], "K", (px) => Math.max(1, (px < x + 4 ? 3 : 1) + tone));
    };
    const face = (x, y) => {
      const gy = (px) => y + 9 - Math.round((px - (x - 16)) / 6);
      for (let px = x - 16; px <= x - 2; px++) { s.px(px, gy(px), "D", 1); s.px(px, gy(px) + 1, "D", 0); }
      for (let px = x - 15; px <= x - 4; px += 3) s.px(px, gy(px) - 1, "R", 5);
      for (let px = x - 14; px <= x - 5; px += 3) s.px(px, gy(px) + 2, "R", 4);
      s.px(x - 18, y + 4, "D", 2); s.px(x - 18, y + 5, "D", 1);
      s.line(x - 14, y, x - 4, y - 3, "K", 4); // lit snout top
      s.line(x - 8, y - 3, x - 1, y - 4, "K", 1); // brow
      s.px(x - 7, y - 2, "Y", 4); s.px(x - 6, y - 2, "Y", 5); s.px(x - 3, y - 2, "Y", 3);
    };
    // Far legs (darker), the far head high behind.
    s.mass([["c", [[56, 90, 8], [57, 106, 6.5], [56, 117, 6]]], ["e", 53, 120, 8, 3]], "K", { tone: -1, seam: true });
    s.mass([["c", [[92, 88, 9], [97, 102, 6.5], [95, 117, 6]]], ["e", 92, 120, 8, 3]], "K", { tone: -1, seam: true });
    s.mass([["c", [[66, 66, 10], [66, 44, 8]]]], "K", { tone: -1, seam: true });
    head(66, 34, -1);
    // Tail.
    s.cap([[104, 74, 4], [114, 70, 3], [118, 62, 2]], "K", { tone: -1 });
    // Body: deep chest, barrel, hips.
    s.mass([["e", 58, 78, 20, 17], ["c", [[66, 80, 15], [96, 80, 14]]]], "K", { k: 3, relief: 2 });
    // Middle neck and head.
    s.mass([["c", [[56, 70, 10], [44, 52, 8]]]], "K", { seam: true });
    head(43, 46, 0);
    // Near legs.
    s.mass([["c", [[100, 86, 10], [104, 100, 7], [101, 117, 6.5]]], ["e", 98, 120, 9, 3.2]], "K", { seam: true });
    s.mass([["c", [[50, 88, 9], [47, 104, 7.5], [46, 117, 7]]], ["e", 43, 120, 9.5, 3.2]], "K", { seam: true });
    // Near neck and head, low and forward.
    s.mass([["c", [[50, 78, 10], [34, 80, 8]]]], "K", { seam: true });
    head(28, 76, 0);
    s.seams();

    face(66, 34); face(43, 46); face(28, 76);
    // Spiked iron collars on the three necks.
    for (const [x0, y0, x1, y1] of [[72, 50, 60, 52], [52, 58, 44, 62], [42, 74, 38, 88]]) {
      s.line(x0, y0, x1, y1, "R", 2); s.line(x0, y0 + 1, x1, y1 + 1, "R", 1);
      for (let t = 0; t <= 1; t += 0.34) {
        const x = Math.round(x0 + (x1 - x0) * t), y = Math.round(y0 + (y1 - y0) * t);
        s.px(x - 1, y - 1, "R", 4); s.px(x - 1, y - 2, "R", 5);
      }
    }
    // Claws and muscle lines on the shoulder.
    for (const x of [35, 38, 41, 90, 93]) s.px(x, 123, "R", 3);
    s.line(52, 82, 58, 90, "K", 1);
  },
};
