// Cave goblin, facing left, hunched, a rusty notched blade forward. Feet on y = 91.
module.exports = {
  id: 401, name: "goblin", size: 96,
  comment: [
    "Cave goblin, facing left: hunched and wiry, a big head with long drooping ears, a hooked nose,",
    "a crooked grin, a notched rusty blade forward, rags and a chest strap. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.66, 0.36, -0.02]);
    // Far (back) arm and far leg, darker.
    s.cap([[54, 60, 3], [59, 69, 2.4], [58, 77, 2.2]], "K", { tone: -1 });
    s.ell(57, 79, 2.8, 2.6, "K", { tone: -1 });
    s.cap([[52, 76, 3.8], [56, 83, 3], [55, 89, 2.6]], "K", { tone: -1 });
    s.ell(53, 90.3, 4, 1.6, "K", { tone: -1 });
    // Far ear, behind the head.
    s.poly([[43, 46], [55, 41], [64, 37], [57, 44], [47, 51]], "K", (x) => (x > 55 ? 1 : 2));
    // Torso hunched forward, belly, hips.
    s.ell(49, 64, 9, 10, "K");
    s.ell(48, 72, 7.5, 5, "K");
    // Near leg: thigh, bent knee, shin, big flat foot.
    s.cap([[46, 75, 4.2], [42, 82, 3.4], [44, 89, 2.8]], "K");
    s.ell(41, 90.3, 5, 1.7, "K");
    // Loincloth with a ragged hem and a belt.
    s.poly([[40, 70], [57, 70], [56, 79], [53, 76], [51, 80], [48, 76], [45, 80], [42, 76], [40, 78]], "H",
      (x, y) => (y < 72 ? 3 : x < 46 ? 3 : 2));
    s.line(40, 70, 57, 70, "H", 1);
    // Chest strap.
    for (let i = 0; i < 12; i++) { s.px(44 + i, 57 + i, "H", 2); s.px(45 + i, 57 + i, "H", 3); }
    // Head: big, low on the shoulders, pushed forward.
    s.ell(40, 51, 9.5, 8.5, "K");
    // Hooked nose, jaw.
    s.cap([[33, 50, 3], [28, 52, 2.2], [27, 54, 1.4]], "K");
    s.ell(37, 58, 6.5, 3.5, "K");
    // Near ear: long, drooping back, notched.
    s.poly([[42, 45], [52, 46], [64, 52], [53, 53], [44, 52]], "K", (x, y) => (y < 49 ? 4 : 3));
    s.poly([[46, 48], [53, 49], [59, 52], [52, 51]], "D", 2);
    s.erase(58, 51); s.erase(57, 52); s.erase(58, 52);
    // Near arm: shoulder, elbow down, the hand forward holding the blade.
    s.cap([[44, 60, 3.4], [39, 68, 2.6], [32, 67, 2.4]], "K");
    s.ell(30, 67, 3, 2.8, "K");
    s.seams();

    // Notched rusty blade: grip, guard, a broad short blade pointing forward and up.
    s.line(32, 71, 31, 64, "H", 2); s.line(33, 71, 32, 64, "H", 3);
    s.line(27, 67, 32, 62, "R", 1); s.line(28, 67, 33, 62, "R", 2);
    s.poly([[30, 63], [17, 53], [12, 52], [14, 56], [17, 60], [28, 67]], "R", (x, y) => (y + (x >> 1) < 64 ? 4 : y + (x >> 1) < 67 ? 3 : 2));
    s.line(13, 53, 28, 64, "R", 5, false);
    s.erase(18, 59); s.erase(19, 60); s.erase(23, 63); // notches
    // Rust patches (H) on the blade.
    for (const [x, y] of [[20, 58], [21, 58], [21, 59], [25, 62], [26, 63], [16, 55], [24, 60]]) s.px(x, y, "H", 3);
    for (const [x, y] of [[22, 59], [26, 62]]) s.px(x, y, "H", 2);

    // Skin texture, scars on the scalp.
    s.recolor((x, y, c) => ((x + 2 * y) % 7 === 0 && c >= 2 && c <= 4 ? c - 1 : c), "K");
    s.line(38, 44, 42, 47, "K", 5);
    s.line(43, 44, 45, 46, "K", 5);
    // Brow, slanted glowing eyes.
    s.line(31, 47, 38, 49, "K", 1);
    s.px(33, 49, "Y", 4); s.px(34, 49, "Y", 5); s.px(35, 50, "Y", 3);
    s.px(37, 50, "Y", 2);
    // Grin: dark mouth line, crooked teeth.
    s.line(31, 57, 40, 58, "D", 1);
    for (const x of [32, 35, 38]) s.px(x, 57, "R", 5);
    for (const x of [33, 37]) s.px(x, 58, "R", 4);
    // Claws on the back hand and the toes.
    s.px(56, 81, "R", 3); s.px(58, 81, "R", 3);
    for (const x of [36, 38, 48, 50]) s.px(x, 91, "R", 3);
  },
};
