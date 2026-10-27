// Dragon, 2x2 on 128, facing left: crimson-black scales (K), a long neck raised, a horned head with
// a glowing maw (Y fire), leathery wings spread over the back (H ash membrane, K bones), four
// clawed legs (R bone claws, horns, spines), a spined tail sweeping back. Feet on y = 123.
const wing = require("./bat_wing");
module.exports = {
  id: 440, name: "dragon", size: 128,
  comment: [
    "Dragon, a 2x2 boss-sized beast, facing left: crimson-black scales, a raised neck, a horned",
    "head with a glowing maw, leathery wings spread over the back, clawed legs, a spined tail.",
  ],
  build(s) {
    s.curve([0.7, 0.45, 0.06]);
    // Far wing (darker), high behind.
    wing(s, [70, 62], [86, 30], [[96, 8], [110, 10], [122, 24], [124, 44], [112, 58]], [84, 70], -1,
      { bone: "K", skin: "H", arm: 3, finger: 3 });
    // Tail sweeping back, far legs.
    s.mass([["c", [[92, 88, 9], [106, 96, 6.5], [116, 106, 4.5], [122, 118, 2.5]]]], "K", { tone: -1 });
    s.mass([["c", [[58, 92, 7], [54, 106, 5.5], [56, 117, 5]]], ["e", 53, 120, 7, 3]], "K", { tone: -1, seam: true });
    s.mass([["c", [[88, 94, 8], [94, 106, 6], [90, 117, 5]]], ["e", 87, 120, 7, 3]], "K", { tone: -1, seam: true });
    // Body and the base of the neck.
    s.mass([["e", 72, 84, 22, 14], ["c", [[56, 80, 12], [46, 64, 9]]]], "K", { k: 3, relief: 2 });
    // Neck rising forward, the head turned left.
    s.mass([["c", [[46, 66, 9], [38, 52, 7.5], [36, 42, 6.5]]]], "K", { k: 2, relief: 1.8, seam: true });
    s.mass([["e", 32, 36, 9, 7], ["c", [[26, 39, 5.5], [14, 42, 4]]]], "K", { k: 1.6, seam: true });
    s.mass([["c", [[26, 46, 3.6], [15, 48, 2.6]]]], "K", { tone: -1, seam: true });
    // Horns sweeping back.
    s.cap([[36, 32, 2.6], [44, 26, 2], [52, 24, 1.2]], "R", {});
    s.cap([[32, 31, 2.2], [36, 22, 1.6], [42, 17, 0.8]], "R", { tone: -1 });
    // Near legs: the foreleg forward, the haunch and hind leg.
    s.mass([["c", [[56, 90, 8], [48, 102, 6], [46, 116, 5.5]]], ["e", 42, 120, 8, 3]], "K", { seam: true });
    s.mass([["e", 86, 88, 11, 12], ["c", [[90, 96, 8], [86, 108, 6], [92, 117, 5]]], ["e", 89, 120, 8, 3]], "K", { seam: true });
    // Near wing, raised over the back.
    wing(s, [62, 64], [70, 28], [[66, 6], [80, 4], [94, 12], [100, 30], [94, 48]], [74, 70], 0,
      { bone: "K", skin: "H", arm: 3.4, finger: 3 });
    s.seams();

    // Spines along the neck, back and tail.
    for (const [x, y, h] of [[40, 44, 4], [44, 52, 5], [50, 62, 5], [62, 70, 5], [74, 70, 5], [86, 72, 5], [100, 90, 4], [110, 98, 3], [117, 108, 2]]) {
      s.poly([[x - 1.6, y + 1], [x + 2, y - h], [x + 1.6, y + 1]], "R", (px) => (px < x + 1 ? 3 : 1));
    }
    // Belly scales: lighter bands under the neck and belly.
    for (const [a, b] of [[[42, 62], [38, 52]], [[56, 92], [80, 96]]]) {
      const n = Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
      for (let k = 0; k <= n; k += 2) {
        const x = Math.round(a[0] + ((b[0] - a[0]) * k) / n), y = Math.round(a[1] + ((b[1] - a[1]) * k) / n);
        if (s.at(x, y)?.slot === "K") { s.px(x, y, "K", 4); s.px(x + 1, y, "K", 3); }
      }
    }
    // Scale texture: a sparse lattice of darker scale edges on the body.
    for (let y = 70; y < 104; y += 3) for (let x = 50; x < 104; x += 4) {
      const ox = x + ((y / 3) % 2) * 2, p = s.at(ox, y);
      if (p?.slot === "K" && p.c >= 2) { p.c -= 1; const q = s.at(ox + 1, y + 1); if (q?.slot === "K" && q.c >= 2) q.c -= 1; }
    }
    // Face: a heavy brow, the burning eye, nostril, the maw open with fangs and fire inside.
    s.line(24, 34, 34, 32, "K", 1);
    s.px(27, 35, "Y", 4); s.px(28, 35, "Y", 5); s.px(29, 35, "Y", 4);
    s.px(13, 40, "D", 1); s.px(14, 39, "D", 1);
    const gy = (x) => 45 - Math.round((x - 13) / 6);
    for (let x = 13; x <= 30; x++) { s.px(x, gy(x), "Y", x < 20 ? 4 : 2); s.px(x, gy(x) - 1, "D", 0); }
    for (const x of [14, 17, 20, 24]) s.px(x, gy(x) - 2, "R", 5);
    for (const x of [16, 19, 23]) s.px(x, gy(x) + 1, "R", 4);
    // Fire and embers spilling from the maw.
    for (const [x, y, c] of [[10, 46, 5], [8, 48, 4], [11, 49, 3], [6, 45, 3], [9, 52, 2]]) s.px(x, y, "Y", c);
    // Claws.
    for (const x of [35, 38, 41, 84, 87]) s.px(x, 123, "R", 3);
  },
};
