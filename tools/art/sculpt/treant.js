// Treant, 2x2 on 128, facing left: a twisted dead oak walking on its roots, a hollow face with
// glowing eye holes, the near branch-arm raised with a knotted thorny club, the far arm hanging with
// twig fingers, a crown of bare branches, moss and shelf fungi. Roots on y = 123.
module.exports = {
  id: 407, name: "treant", size: 128,
  comment: [
    "Treant, a 2x2 tank, facing left: a twisted dead oak on its roots, a hollow face with glowing",
    "eye holes and a split mouth, a raised branch-arm ending in a thorny club, the far arm hanging",
    "with twig fingers, bare crooked crown, hanging moss, shelf fungi. Roots on y = 123.",
  ],
  build(s) {
    s.curve([0.62, 0.3, -0.06]);
    // Far arm hanging behind (darker), twig fingers.
    s.mass([["c", [[76, 58, 7], [88, 72, 5], [93, 88, 3.5], [94, 96, 2.4]]]], "K", { tone: -1, seam: true });
    for (const [dx, dy] of [[-3, 6], [0, 7], [3, 5]]) s.cap([[94, 95, 1.6], [94 + dx, 95 + dy, 0.9]], "K", { tone: -1 });
    // Crown: bare crooked branches.
    s.cap([[60, 34, 6], [54, 22, 3.5], [47, 12, 2], [44, 6, 1]], "K");
    s.cap([[54, 22, 2.5], [60, 14, 1.5], [62, 8, 0.8]], "K");
    s.cap([[68, 34, 6], [76, 22, 3.5], [86, 14, 2], [92, 10, 1]], "K");
    s.cap([[78, 21, 2.2], [80, 12, 1.2]], "K");
    s.cap([[64, 32, 5], [67, 18, 3], [66, 6, 1.2]], "K");
    // Roots: legs spreading on the ground.
    s.mass([["c", [[56, 110, 7], [44, 119, 4], [33, 122, 2]]]], "K", { tone: -1, seam: true });
    s.mass([["c", [[74, 110, 7], [86, 119, 4], [96, 122, 2]]]], "K", { tone: -1, seam: true });
    // Trunk: one mass, thick at the base, twisting up.
    s.mass([["c", [[64, 116, 18], [61, 98, 13], [64, 78, 12], [66, 56, 14], [64, 40, 11]]]], "K", { k: 3, relief: 1.8 });
    // Near roots in front.
    s.mass([["c", [[58, 114, 7], [50, 120, 4.5], [42, 122, 2.5]]]], "K", { seam: true });
    s.mass([["c", [[68, 116, 6.5], [72, 121, 4], [78, 122, 2]]]], "K", { seam: true });
    // Near arm raised forward: the branch ends in a knotted club.
    s.mass([["c", [[56, 58, 7], [44, 56, 5], [34, 44, 4.5]]], ["e", 30, 38, 6, 5.5], ["e", 24, 34, 5, 5], ["e", 28, 30, 4.5, 4]], "K", { k: 1.5, seam: true });
    s.seams();

    // Thorns on the club.
    for (const [x, y, dx, dy] of [[20, 32, -4, -2], [24, 29, -1, -4], [31, 30, 3, -3], [19, 39, -4, 1], [26, 43, -1, 4]]) {
      s.line(x, y, x + dx, y + dy, "R", 3); s.px(x + dx, y + dy, "R", 5);
    }
    // Bark: cracks spiralling up the trunk (the oak is twisted).
    for (const x0 of [50, 58, 66, 74]) {
      let x = x0;
      for (let y = 114; y > 40; y--) {
        if (y % 4 === 0) x += 1;
        if (x > 78) x -= 30;
        const p = s.at(x, y);
        if (p && p.slot === "K") { p.c = 0; }
      }
    }
    // Knots on the bark.
    for (const [x, y] of [[58, 98], [70, 84], [62, 46]]) { s.ell(x, y, 2.5, 2, "K", { tone: -3 }); s.px(x - 1, y - 2, "K", 4); s.px(x, y - 2, "K", 3); }
    // Hollow face: eye holes with a toxic glow, a split jagged mouth.
    for (const [x, y] of [[51, 64], [61, 63]]) {
      for (let yy = y - 3; yy <= y + 3; yy++) for (let xx = x - 4; xx <= x + 4; xx++)
        if (((xx - x) / 3.6) ** 2 + ((yy - y) / 3) ** 2 <= 1) s.px(xx, yy, "K", 0);
      s.px(x - 1, y, "Y", 4); s.px(x, y, "Y", 5); s.px(x - 1, y + 1, "Y", 3); s.px(x, y + 1, "Y", 4); s.px(x + 1, y, "Y", 2);
    }
    s.line(48, 61, 54, 60, "K", 4); s.line(58, 60, 64, 59, "K", 4); // brow ridges
    const mouth = [[45, 75], [48, 73], [51, 77], [54, 73], [57, 78], [60, 73], [63, 77], [66, 74]];
    for (let i = 0; i + 1 < mouth.length; i++) {
      const [a, b] = [mouth[i], mouth[i + 1]];
      // Only over the bark: the mouth never leaves the trunk outline.
      const n = Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
      for (let t = 0; t <= n; t++) for (let k = 0; k < 3; k++) {
        const x = Math.round(a[0] + ((b[0] - a[0]) * t) / n), y = Math.round(a[1] + ((b[1] - a[1]) * t) / n) + k;
        if (s.at(x, y) && s.at(x - 1, y)) s.px(x, y, "K", 0);
      }
    }
    s.px(55, 76, "Y", 2); s.px(56, 76, "Y", 3); s.px(52, 77, "Y", 1); // glow deep inside
    // Shelf fungi on the trunk.
    for (const [x, y] of [[74, 88], [76, 94], [52, 100]]) { s.ell(x, y, 4, 1.6, "R", {}); s.line(x - 3, y + 1, x + 3, y + 1, "R", 1); }
    // Hanging moss from the arms and the crown.
    for (const [x, y, l] of [[38, 50, 9], [44, 54, 6], [33, 46, 5], [84, 66, 8], [90, 76, 6], [58, 40, 7], [70, 38, 9], [50, 18, 5], [80, 18, 6]]) {
      for (let i = 0; i < l; i++) s.px(x + (i > l / 2 ? 1 : 0), y + i, "H", i < 2 ? 3 : i < l - 2 ? 2 : 1);
    }
    // A few dark leaves left on the crown.
    for (const [x, y] of [[46, 10], [62, 8], [88, 12], [56, 20], [74, 22]]) { s.ell(x, y, 2, 1.4, "H", {}); }
  },
};
