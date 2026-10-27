// Wood Fairy King, a boss hovering on 1x1 but filling the 96 canvas: huge torn moth wings (element
// ramp R), a long robe of dead leaves and bark (H), a crown of thorny branches with glowing tips (Y),
// an orb in the near hand, a thorn staff in the far hand. Facing left; the robe hem is the lowest
// point at y = 90.
const insectWing = require("./wing");
module.exports = {
  id: 410, name: "wood_fairy_king", size: 96,
  comment: [
    "Wood Fairy King, a boss, facing left, hovering: huge torn moth wings and the orb take the",
    "element ramp (R), a long robe of dead leaves, a crown of thorns with glowing tips, a cold long",
    "face, a thorn staff. The hem is the lowest point at y = 90.",
  ],
  build(s) {
    s.curve([0.6, 0.3, -0.06]);
    // Moth wings: the far pair on the left (darker), the near pair on the right.
    insectWing(s, [[44, 38], [36, 24], [22, 12], [9, 11], [4, 20], [10, 32], [30, 42]], -1, [[16, 18], [24, 30]]);
    insectWing(s, [[44, 46], [30, 52], [18, 62], [15, 72], [24, 74], [36, 62]], -1, [[24, 64]]);
    insectWing(s, [[52, 38], [60, 22], [72, 9], [86, 7], [92, 16], [86, 32], [68, 44]], 0, [[78, 18], [70, 30], [84, 24]]);
    insectWing(s, [[52, 46], [68, 50], [82, 58], [85, 72], [76, 77], [62, 64]], 0, [[74, 64], [68, 58]]);
    // Thorn staff in the far hand, behind the robe.
    s.line(60, 22, 61, 88, "H", 3, false); s.line(61, 22, 62, 88, "H", 1, false);
    // Staff head: a curled thorn branch with a glowing bud.
    s.line(61, 22, 64, 18, "H", 2, false); s.line(64, 18, 66, 20, "H", 2, false); s.line(66, 20, 65, 23, "H", 1, false);
    s.px(64, 21, "Y", 5); s.px(63, 21, "Y", 4); s.px(65, 21, "Y", 3); s.px(64, 20, "Y", 4); s.px(64, 22, "Y", 2);
    for (const [x, y] of [[60, 40], [62, 52], [60, 64], [62, 76]]) s.px(x + (x < 61 ? -1 : 1), y, "H", 3);
    // Long hair down the back.
    for (const [tx, ty] of [[56, 48], [58, 42], [54, 52]]) s.cap([[49, 26, 3.4], [tx, ty, 1]], "H", { tone: -1 });
    // Robe: shoulders to a long ragged hem of leaves.
    s.poly([[42, 36], [54, 36], [56, 48], [61, 64], [64, 80], [62, 87], [58, 83], [55, 90], [52, 84], [49, 90],
      [46, 84], [43, 89], [40, 83], [36, 87], [35, 76], [39, 56], [41, 44]], "H",
      (x, y) => (x < 43 ? 3 : x > 55 ? 1 : 2));
    // Near arm forward with the orb.
    s.cap([[44, 40, 2.6], [37, 47, 2], [30, 49, 1.8]], "H");
    s.ell(28, 49, 2, 1.8, "K");
    s.ell(23, 47, 5, 5, "R", {});
    // Far hand on the staff.
    s.ell(61, 46, 2, 1.8, "K", { tone: -1 });
    s.seams();

    // Eye spots on the upper wings: a dark ring around a pale core.
    for (const [x, y, tone] of [[78, 20, 0], [18, 22, -1]]) {
      s.ell(x, y, 4, 3.4, "R", { tone: -3 });
      s.ell(x, y, 2.2, 1.8, "R", { tone: 1 + tone });
      s.px(x - 1, y - 1, "R", 5 + tone);
    }
    // Head, pixel by pixel: a long cold face in profile, high cheekbones, a glowing eye, a thin
    // mouth, the hair swept back under the crown.
    s.patch(39, 22, [
      ".....cddddd..",
      "....ccdddddd.",
      "...344cddddd.",
      "..34554cdddd.",
      "..4bbb44cddd.",
      ".34kl3443cdd.",
      "..4444433cdd.",
      ".44444433cdd.",
      "454444333cdd.",
      ".4444333cdd..",
      "..3n44333cd..",
      "..444333cd...",
      "...44433.....",
      "....3333.....",
    ]);
    // Crown of twisted thorny branches with glowing tips.
    for (const [x, y, dx, dy] of [[41, 23, -3, -6], [44, 22, -1, -8], [47, 22, 1, -9], [50, 23, 3, -7], [52, 24, 5, -4]]) {
      s.line(x, y, x + dx, y + dy, "H", 1); s.line(x + 1, y, x + dx + 1, y + dy, "H", 3, false);
      s.px(x + dx, y + dy - 1, "Y", 5); s.px(x + dx, y + dy, "Y", 3);
    }
    s.line(40, 24, 53, 24, "H", 2);
    // Orb core and crackle, sparks under the hem.
    s.px(22, 46, "R", 5); s.px(23, 46, "R", 5); s.px(22, 45, "R", 4);
    for (const [x, y] of [[17, 43], [16, 49], [28, 42], [19, 54], [25, 55]]) s.px(x, y, "R", 4);
    for (const [x, y] of [[44, 93], [50, 94], [56, 92]]) s.px(x, y, "R", 3);
    // Layers of leaves and bark strips on the robe.
    // Leaves scattered, not in rows: a hash picks the spots, each leaf a short stroke and a lit tip.
    const hash = (x, y) => (((x * 2654435761) ^ (y * 40503)) >>> 0) % 9;
    for (let y = 44; y <= 84; y += 3) for (let x = 37; x <= 61; x += 3) {
      const p0 = s.at(x, y);
      if (!p0 || p0.slot !== "H" || hash(x, y) > 1) continue;
      const len = 2 + (hash(y, x) % 2);
      s.line(x, y, x + 1, y + len, "H", 1, false); s.px(x - 1, y, "H", 3);
    }
    s.line(48, 38, 48, 86, "H", 1, false); // the robe's front edge
  },
};
