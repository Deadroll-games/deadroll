// Shared goblin anatomy (facing left, feet on y = 91 at scale 1). Callers add the near arm and gear.
// One canon for the whole goblin family: big head (about 1/3 of the height), hunched torso, short
// bent legs under the hips, long arms (the hand reaches the hip), long drooping ears.
exports.back = (s, o = {}) => {
  // Far (back) arm and far leg, darker.
  s.cap([[54, 60, 3], [59, 69, 2.4], [58, 77, 2.2]], "K", { tone: -1 });
  s.ell(57, 79, 2.8, 2.6, "K", { tone: -1 });
  s.cap([[52, 76, 3.8], [56, 83, 3], [55, 89, 2.6]], "K", { tone: -1 });
  s.ell(53, 90.3, 4, 1.6, "K", { tone: -1 });
  // Far ear, behind the head.
  if (o.noEar) return;
  exports.farEar(s, o);
};

exports.farEar = (s, o = {}) => {
  if (o.earDown) s.poly([[44, 47], [55, 46], [63, 47], [56, 50], [47, 51]], "K", (x) => (x > 55 ? 1 : 2));
  else s.poly([[43, 46], [55, 41], [64, 37], [57, 44], [47, 51]], "K", (x) => (x > 55 ? 1 : 2));
};

exports.body = (s, o = {}) => {
  // Torso hunched forward, belly, hips.
  s.ell(49, 64, 9, 10, "K");
  s.ell(48, 72, 7.5, 5, "K");
  // Near leg: thigh, bent knee, shin, big flat foot.
  s.cap([[46, 75, 4.2], [42, 82, 3.4], [44, 89, 2.8]], "K");
  s.ell(41, 90.3, 5, 1.7, "K");
  if (!o.noLoincloth) {
    s.poly([[40, 70], [57, 70], [56, 79], [53, 76], [51, 80], [48, 76], [45, 80], [42, 76], [40, 78]], "H",
      (x, y) => (y < 72 ? 3 : x < 46 ? 3 : 2));
    s.line(40, 70, 57, 70, "H", 1);
  }
};

exports.head = (s, o = {}) => {
  s.ell(40, 51, 9.5, 8.5, "K");
  // Hooked nose, jaw.
  if (o.shortNose) s.cap([[33, 50, 3], [30, 51, 2.4], [29, 53, 1.5]], "K");
  else s.cap([[33, 50, 3], [28, 52, 2.2], [27, 54, 1.4]], "K");
  s.ell(37, 58, 6.5, 3.5, "K");
  // Near ear: long, drooping back, notched.
  s.poly([[42, 45], [52, 46], [64, 52], [53, 53], [44, 52]], "K", (x, y) => (y < 49 ? 4 : 3));
  s.poly([[46, 48], [53, 49], [59, 52], [52, 51]], "D", 2);
  s.erase(58, 51); s.erase(57, 52); s.erase(58, 52);
};

// Skin texture, face details: call after `seams`.
exports.face = (s, o = {}) => {
  s.recolor((x, y, c) => ((x + 2 * y) % 7 === 0 && c >= 2 && c <= 4 ? c - 1 : c), "K");
  if (!o.noScars) { s.line(38, 44, 42, 47, "K", 5); s.line(43, 44, 45, 46, "K", 5); }
  s.line(31, 47, 38, 49, "K", 1);
  s.px(33, 49, "Y", 4); s.px(34, 49, "Y", 5); s.px(35, 50, "Y", 3);
  s.px(37, 50, "Y", 2);
  s.line(31, 57, 40, 58, "D", 1);
  for (const x of [32, 35, 38]) s.px(x, 57, "R", o.teeth || 5);
  for (const x of [33, 37]) s.px(x, 58, "R", o.teeth ? o.teeth - 1 : 4);
  s.px(56, 81, "R", 3); s.px(58, 81, "R", 3);
  for (const x of [36, 38, 48, 50]) s.px(x, 91, "R", 3);
};
