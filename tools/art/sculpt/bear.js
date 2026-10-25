// Cave bear, 2x2 on 128, facing left, roaring, walking on four legs. Feet on y = 123.
module.exports = {
  id: 406, name: "bear", size: 128,
  comment: [
    "Cave bear, a 2x2 tank, facing left: one heavy mass with a hump over the shoulders, the head",
    "low and roaring, four thick legs under chest and hips, scars, far legs darker. Feet on y = 123.",
  ],
  build(s) {
    s.curve([0.74, 0.5, 0.14]);
    // Far legs (darker): front under the chest, hind under the hip.
    s.mass([["c", [[57, 92, 9], [57, 106, 7.5], [56, 117, 7]]], ["e", 53, 120, 8.5, 3.2]], "K", { tone: -1, seam: true });
    s.mass([["c", [[91, 94, 9], [92, 106, 7.5], [92, 117, 7]]], ["e", 89, 120, 8.5, 3.2]], "K", { tone: -1, seam: true });
    // One body: barrel, hump, rump, belly fur, shoulder and thigh, neck and head.
    s.mass([
      ["c", [[54, 80, 16], [72, 77, 18], [98, 80, 17]]],
      ["c", [[54, 62, 9], [62, 57, 12], [80, 66, 10]]],
      ["c", [[52, 66, 9.5], [42, 71, 8.5]]],
      ["e", 75, 92, 23, 7, { ragged: 0.4, tufts: 34 }],
      ["c", [[50, 82, 10.5], [47, 96, 10]]],
      ["e", 103, 88, 12.5, 14],
      ["e", 33, 72, 13, 12],
      ["c", [[24, 77, 7.5], [12, 79, 6]]],
      ["e", 35, 59, 5.5, 6],
    ], "K", { k: 4, relief: 2.2 });
    // Lower near legs: forearm, wrist, paw; shin, paw.
    s.mass([["c", [[46, 98, 9.5], [44, 110, 8], [43, 117, 7.5]]], ["e", 40, 120, 10.5, 3.5]], "K", { seam: true });
    s.mass([["c", [[103, 100, 9.5], [102, 111, 8], [101, 117, 7.5]]], ["e", 98, 120, 10, 3.5]], "K", { seam: true });
    // Far ear, and the open lower jaw hinged high at the cheek.
    s.ell(42, 59, 5, 5.5, "K", { tone: -1 });
    s.mass([["c", [[28, 81, 4], [21, 85, 3.8], [14, 87, 3]]]], "K", { tone: -1, seam: true });
    s.seams();

    // Fur strands and the shadowed underside.
    s.recolor((x, y, c) => ((x + 2 * y) % 7 === 0 && c >= 2 && c <= 4 ? c - 1 : c), "K");
    s.recolor((x, y, c) => (y > 92 && x > 50 && x < 96 && c > 1 ? c - 1 : c), "K");
    // Ragged fur along the hump.
    for (let x = 46; x <= 92; x += 4) {
      const top = [...Array(128).keys()].find((y) => s.at(x, y));
      if (top !== undefined) s.poly([[x - 2, top + 2], [x + 1, top - 2], [x + 2, top + 2]], "K", 3);
    }
    // Inside the ear.
    s.ell(35, 59.5, 1.6, 2, "D", {});
    // Roar: the gap rises from the lips to the hinge at the cheek; fangs and teeth.
    const gy = (x) => 84 - Math.round((x - 11) / 4);
    for (let x = 11; x <= 27; x++) { s.px(x, gy(x), "D", 1); s.px(x, gy(x) + 1, "D", 1); }
    for (const x of [13, 16, 19, 23]) s.px(x, gy(x) - 1, "R", 5);
    s.px(14, gy(14), "R", 4); s.px(14, gy(14) + 1, "R", 3); // upper fang
    for (const x of [17, 20]) s.px(x, gy(x) + 2, "R", 4);
    s.px(18, gy(18) + 1, "R", 5); // lower fang
    // Nose and the lit snout top.
    s.ell(8, 77, 2.8, 2.4, "D", {});
    s.px(7, 76, "D", 3);
    s.line(12, 73, 22, 68, "K", 4);
    // Scars: across the snout and the flank.
    s.line(20, 70, 25, 75, "K", 5);
    s.line(76, 70, 84, 78, "K", 4);
    s.line(79, 69, 87, 77, "K", 4);
    // Brow and the small furious eyes.
    s.line(20, 69, 30, 68, "K", 1);
    s.px(23, 71, "Y", 3); s.px(24, 71, "Y", 5); s.px(25, 70, "Y", 4); s.px(26, 71, "Y", 2);
    // Claws on the four paws.
    for (const x of [31, 34, 37, 47, 50, 88, 91, 94]) s.px(x, 123, "R", 3);
  },
};
