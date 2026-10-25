// Behemoth, 2x2 on 128, facing left: a massive demon beast on four limbs, charred hide (K ash)
// split by glowing magma cracks (Y fire), heavy horns curving forward (R bone), spikes along the
// spine, thick forelegs like pillars, a club tail. Feet on y = 123.
module.exports = {
  id: 433, name: "behemoth", size: 128,
  comment: [
    "Behemoth, a 2x2 boss-sized beast, facing left: charred hide split by glowing magma cracks,",
    "heavy horns curving forward, spikes along the spine, pillar forelegs, a club tail. Feet y = 123.",
  ],
  build(s) {
    s.curve([0.64, 0.36, -0.02]);
    // Club tail behind.
    s.cap([[100, 86, 6], [110, 92, 4.5], [116, 100, 3.5]], "K", { tone: -1 });
    s.ell(118, 104, 5, 5.5, "R", { tone: -1 });
    for (const [x, y, dx, dy] of [[118, 98, 0, -4], [123, 103, 4, 0], [119, 109, 1, 4], [113, 104, -4, 0]])
      s.poly([[x - 1.5 * (dy !== 0), y - 1.5 * (dx !== 0)], [x + dx, y + dy], [x + 1.5 * (dy !== 0), y + 1.5 * (dx !== 0)]], "R", 3);
    // Far legs (darker).
    s.mass([["c", [[50, 88, 9], [50, 104, 8], [50, 117, 7.5]]], ["e", 48, 120, 9.5, 3.4]], "K", { tone: -1, seam: true });
    s.mass([["c", [[92, 92, 9], [94, 106, 7], [92, 117, 6.5]]], ["e", 90, 120, 9, 3.4]], "K", { tone: -1, seam: true });
    // Body: the shoulder hump towering over the low hindquarters.
    s.mass([
      ["e", 56, 66, 26, 22],
      ["c", [[62, 78, 20], [86, 84, 16], [100, 84, 14]]],
      ["e", 40, 72, 12, 12],
    ], "K", { k: 3, relief: 1.8 });
    // Near legs: a pillar foreleg with a broad clawed foot, the hind leg bent.
    s.mass([["c", [[42, 82, 11], [40, 100, 9.5], [40, 116, 9]]], ["e", 37, 120, 12, 3.6]], "K", { seam: true });
    s.mass([["c", [[100, 90, 10], [104, 104, 7.5], [100, 117, 7]]], ["e", 97, 120, 10, 3.4]], "K", { seam: true });
    // Head low between the shoulders: a broad brow, a heavy jaw.
    s.mass([["e", 26, 70, 11, 10], ["c", [[20, 78, 7], [12, 80, 5.5]]]], "K", { k: 2, seam: true });
    s.mass([["c", [[24, 86, 5], [14, 87, 3.8]]]], "K", { tone: -1, seam: true });
    // Horns curving forward from the brow.
    s.cap([[34, 62, 4], [30, 52, 3], [22, 47, 2], [17, 49, 1.2], [16, 52, 0.6]], "R", { tone: -1 });
    s.cap([[28, 64, 4.6], [20, 56, 3.6], [12, 54, 2.6], [7, 58, 1.6], [6, 62, 0.7]], "R", {});
    // Growth rings on the near horn.
    for (const [x, y] of [[23, 57], [17, 55], [12, 55]]) { s.px(x, y, "R", 1); s.px(x, y + 1, "R", 1); s.px(x, y + 2, "R", 1); }
    s.seams();

    // Spikes along the spine.
    for (const [x, h] of [[40, 8], [48, 11], [56, 12], [64, 10], [72, 8], [80, 6], [88, 5]]) {
      const top = [...Array(128).keys()].find((y) => s.at(x, y)?.slot === "K");
      if (top !== undefined) s.poly([[x - 2.5, top + 3], [x + 1, top - h], [x + 2.5, top + 3]], "R", (px) => (px < x ? 4 : 2));
    }
    // Magma cracks: a branching glowing network over the hide.
    const crack = (pts) => {
      for (let i = 0; i + 1 < pts.length; i++) {
        const [a, b] = [pts[i], pts[i + 1]];
        const n = Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
        for (let k = 0; k <= n; k++) {
          const x = Math.round(a[0] + ((b[0] - a[0]) * k) / n), y = Math.round(a[1] + ((b[1] - a[1]) * k) / n);
          if (s.at(x, y)?.slot === "K") s.px(x, y, "Y", k % 3 === 0 ? 5 : 3);
        }
      }
    };
    crack([[46, 56], [52, 64], [50, 72], [56, 80]]);
    crack([[52, 64], [60, 62], [66, 70]]);
    crack([[70, 72], [76, 80], [84, 78], [90, 86]]);
    crack([[40, 90], [42, 100], [39, 108]]);
    crack([[100, 96], [104, 106]]);
    crack([[30, 66], [26, 72]]);
    // Face: burning eyes under the brow, nostrils, the maw with fangs and a glow inside.
    s.line(14, 70, 30, 67, "K", 1);
    s.px(18, 72, "Y", 4); s.px(19, 72, "Y", 5); s.px(20, 71, "Y", 5); s.px(26, 71, "Y", 3);
    s.px(8, 78, "D", 1); s.px(9, 79, "D", 1);
    const gy = (x) => 84 - Math.round((x - 9) / 6);
    for (let x = 9; x <= 28; x++) { s.px(x, gy(x), "D", 1); s.px(x, gy(x) + 1, "Y", x % 4 === 0 ? 3 : 1); }
    for (const x of [10, 14, 19, 24]) s.px(x, gy(x) - 1, "R", 5);
    for (const x of [12, 17, 22]) s.px(x, gy(x) + 2, "R", 4);
    // Claws.
    for (const x of [27, 30, 33, 36, 86, 89, 92]) s.px(x, 123, "R", 3);
  },
};
