// Hydra, 2x2 on 128, facing left: a heavy swamp-scaled body low to the ground on short clawed legs,
// five serpent necks rising at different heights (far ones darker), each head with open jaws,
// fangs (R bone) and toxic eyes (Y). Feet on y = 123.
module.exports = {
  id: 442, name: "hydra", size: 128,
  comment: [
    "Hydra, a 2x2 boss-sized beast, facing left: a heavy swamp-scaled body on short clawed legs,",
    "five serpent necks rising at different heights, open jaws with fangs, toxic eyes. Feet y = 123.",
  ],
  build(s) {
    s.curve([0.66, 0.4, 0.02]);
    // One neck and head: the neck from the body to the head, then a snake head facing left.
    const neck = (base, mid, head, r, tone) => {
      // An S-curve: sway sideways at a third and two thirds of the way.
      const at = (t) => [base[0] + (head[0] + 4 - base[0]) * t, base[1] + (head[1] + 3 - base[1]) * t];
      const dx = head[1] - base[1], dy = -(head[0] - base[0]), n = Math.hypot(dx, dy) || 1, sw = 5;
      const p1 = at(0.33), p2 = at(0.66);
      s.mass([["c", [[base[0], base[1], r], [p1[0] + (dx / n) * sw, p1[1] + (dy / n) * sw, r * 0.85],
        [p2[0] - (dx / n) * sw, p2[1] - (dy / n) * sw, r * 0.72], [head[0] + 4, head[1] + 3, r * 0.65]]]], "K", { tone, seam: true, k: 2 });
    };
    const snake = (x, y, tone) => {
      s.mass([["e", x, y, 6, 4.6], ["c", [[x - 4, y + 1, 3.6], [x - 11, y + 2, 2.4]]]], "K", { tone, seam: true, k: 1.4 });
      s.mass([["c", [[x - 3, y + 6, 2.2], [x - 10, y + 8, 1.6]]]], "K", { tone: tone - 1, seam: true });
    };
    const face = (x, y) => {
      s.px(x - 3, y - 1, "Y", 4); s.px(x - 2, y - 1, "Y", 5);
      s.line(x - 5, y - 3, x, y - 3, "K", 1);
      s.px(x - 12, y + 1, "D", 1);
      for (let px = x - 11; px <= x; px++) s.px(px, y + 4 + Math.round((px - x) / 6), "D", 0);
      s.px(x - 10, y + 3, "R", 5); s.px(x - 10, y + 4, "R", 3); s.px(x - 7, y + 3, "R", 4);
      s.px(x - 9, y + 6, "R", 4);
    };
    // Necks and heads, back to front.
    const heads = [
      [[84, 78], [96, 52], [92, 26], -2],
      [[76, 76], [80, 46], [70, 20], -1],
      [[66, 78], [58, 52], [48, 30], -1],
      [[60, 82], [44, 66], [28, 50], 0],
      [[56, 88], [38, 86], [18, 74], 0],
    ];
    for (const [i, [b, m, h, t]] of heads.entries()) {
      if (i === 3) {
        // The body goes in front of the three far necks and behind the two near ones.
        s.mass([["c", [[96, 104, 10], [104, 112, 7], [116, 118, 4]]]], "K", { tone: -1 });
        s.mass([["c", [[80, 98, 6], [84, 112, 5]]], ["e", 82, 120, 7, 3]], "K", { tone: -1, seam: true });
        s.mass([["e", 70, 98, 26, 16], ["e", 56, 96, 14, 12]], "K", { k: 3, relief: 1.8 });
        s.mass([["c", [[50, 104, 6], [46, 114, 5]]], ["e", 43, 120, 8, 3]], "K", { seam: true });
        s.mass([["c", [[86, 104, 6], [90, 114, 5]]], ["e", 88, 120, 7, 3]], "K", { seam: true });
      }
      neck(b, m, h, 6.5 - Math.abs(t), t);
      snake(h[0], h[1], t);
    }
    s.seams();
    for (const [, , h] of heads) face(h[0], h[1]);
    // Belly scales and claws.
    for (let x = 46; x < 90; x += 3) if (s.at(x, 108)?.slot === "K") { s.px(x, 108, "K", 3); s.px(x + 1, 109, "K", 1); }
    for (const x of [37, 40, 43, 84, 87]) s.px(x, 123, "R", 3);
  },
};
