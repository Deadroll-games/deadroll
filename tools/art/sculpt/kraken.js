// Kraken, 2x2 on 128, facing left: a bulbous mantle (K bruise) with a huge yellow eye and a slit
// pupil (Y), a hooked beak (R), tentacles sprawling on the ground and curling up, pale suckers
// (R) on their undersides. Lowest tentacles on y = 123.
module.exports = {
  id: 446, name: "kraken", size: 128,
  comment: [
    "Kraken, a 2x2 beast, facing left: a bulbous mantle with a huge slit-pupil eye and a hooked",
    "beak, tentacles sprawling on the ground and curling up, pale suckers. Lowest on y = 123.",
  ],
  build(s) {
    s.curve([0.64, 0.38, -0.02]);
    // A tentacle: a tapering wavy capsule through `pts`, suckers along its lower edge.
    const tentacle = (pts, r0, tone, suckers = true) => {
      const n = pts.length - 1, path = pts.map(([x, y], i) => [x, y, r0 * (1 - (i / n) * 0.8)]);
      s.cap(path, "K", { tone });
      if (!suckers) return;
      for (let i = 0; i < n; i++) {
        const [a, b] = [path[i], path[i + 1]];
        for (const t of [0.3, 0.8]) {
          const x = a[0] + (b[0] - a[0]) * t, y = a[1] + (b[1] - a[1]) * t, r = a[2] + (b[2] - a[2]) * t;
          const px = Math.round(x), py = Math.round(y + r * 0.7);
          if (s.at(px, py)?.slot === "K") s.px(px, py, "R", r > 3 ? 3 : 2);
        }
      }
    };
    // Back tentacles curling up (darker).
    tentacle([[86, 92], [102, 86], [110, 72], [106, 58], [112, 46], [120, 40], [124, 42]], 6, -1);
    tentacle([[72, 90], [70, 74], [76, 60], [86, 52], [92, 56]], 5, -1);
    tentacle([[92, 104], [108, 108], [120, 102], [124, 92]], 5.5, -1);
    // Mantle: the bulbous head rising behind, a lit dome.
    s.mass([["e", 70, 64, 20, 26], ["e", 66, 92, 18, 14]], "K", { k: 3, relief: 1.8 });
    // Front tentacles sprawling forward on the ground and one raised to strike.
    tentacle([[56, 100], [42, 110], [28, 116], [14, 118], [6, 112]], 7, 0);
    tentacle([[66, 106], [58, 118], [44, 122], [32, 121]], 6.5, 0);
    tentacle([[80, 108], [86, 120], [100, 122], [110, 118]], 6, 0);
    tentacle([[52, 92], [40, 90], [30, 80], [26, 66], [30, 54], [24, 44], [14, 40], [8, 42]], 6.5, 0);
    s.seams();

    // The eye: a huge yellow eye with a horizontal slit pupil, a heavy lid above.
    s.patch(50, 52, [
      "...0000000...",
      ".00111111100.",
      "0111hhhhh1110",
      "01hijjmjjih10",
      "0hijkkmkkjih0",
      "0hjkkmmmkkjh0",
      "0hijkkmkkjih0",
      "01hijjmjjih10",
      ".01hhhhhhh10.",
      "..000000000..",
    ]);
    s.px(53, 56, "Y", 5); s.px(54, 56, "Y", 5);
    s.line(50, 51, 62, 50, "K", 4); // the lid fold
    // The beak under the mantle, between the tentacles.
    s.poly([[58, 96], [52, 98], [50, 102], [54, 101], [58, 100]], "R", (x, y) => (y < 99 ? 4 : 2));
    // Warty spots on the mantle.
    for (const [x, y] of [[62, 48], [74, 44], [80, 58], [66, 58], [84, 72], [58, 84]]) { s.px(x, y, "K", 1); s.px(x + 1, y, "K", 4); }
  },
};
