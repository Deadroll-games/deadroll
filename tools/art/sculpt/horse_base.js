// Shared horse anatomy, 96 canvas, facing left, hooves on y = 91: barrel, chest, hindquarters, an
// arched neck, a long head, legs with knees and hocks, hooves. Callers add mane, tail and features.
// o.lean: a gaunt horse (ribs, thin neck); o.hoof: slot of the hooves.
exports.far = (s, o = {}) => {
  const hoof = o.hoof || "R";
  // Far legs (darker): front leg back, hind leg forward under the belly.
  s.cap([[44, 62, 3], [44, 74, 1.8], [45, 86, 1.4]], "K", { tone: -1 });
  s.poly([[43, 86], [48, 86], [48, 91], [42, 91]], hoof, 1);
  s.cap([[68, 62, 4.2], [72, 72, 2.2], [69, 86, 1.4]], "K", { tone: -1 });
  s.poly([[67, 86], [72, 86], [72, 91], [66, 91]], hoof, 1);
};

exports.body = (s, o = {}) => {
  const hoof = o.hoof || "R";
  // Barrel, chest and hindquarters in one mass, the neck rising and arching forward.
  s.mass([
    ["e", 56, 58, 17, o.lean ? 7.5 : 9],
    ["e", 40, 58, 8, 9],
    ["e", 71, 57, 9, 10],
    ["c", [[40, 54, o.lean ? 5.5 : 6.5], [33, 44, o.lean ? 4.4 : 5.2], [29, 36, o.lean ? 3.6 : 4.4]]],
  ], "K", { k: 2.5, relief: 1.8 });
  // Head: skull, the long face, the muzzle.
  s.mass([["e", 26, 34, 5.4, 4.6], ["c", [[23, 37, 3.8], [16, 43, 2.8]]]], "K", { k: 1.4, seam: true });
  // Ears.
  s.poly([[27, 30], [29, 24], [31, 30]], "K", 3);
  // Near legs: front leg with knee and fetlock, hind leg with hock.
  s.cap([[38, 64, 3.2], [36, 75, 1.9], [37, 86, 1.5]], "K", { seam: true });
  s.ell(36, 75, 2.1, 2, "K", { seam: true });
  s.poly([[34, 86], [40, 86], [40, 91], [33, 91]], hoof, (x) => (x < 37 ? 3 : 2));
  s.cap([[74, 62, 4.6], [76, 72, 2.4], [72, 86, 1.5]], "K", { seam: true });
  s.ell(76, 72, 2.2, 2.1, "K", { seam: true });
  s.poly([[69, 86], [75, 86], [75, 91], [68, 91]], hoof, (x) => (x < 72 ? 3 : 2));
};

// Face details: eye, nostril, mouth; ribs for a gaunt horse.
exports.face = (s, o = {}) => {
  s.px(24, 33, "Y", 4); s.px(25, 33, "Y", 5); s.line(22, 32, 27, 31, "K", 1);
  s.px(15, 42, "D", 1); s.line(16, 45, 20, 44, "D", 1);
  // Ribs: vertical arcs on the flank, a lit edge before a shadow line.
  if (o.lean) for (const x0 of [50, 54, 58, 62]) for (let y = 53; y <= 63; y++) {
    const x = x0 + Math.round(((y - 58) * (y - 58)) / 12);
    const p = s.at(x, y), q = s.at(x + 1, y);
    if (p?.slot === "K") p.c = 3;
    if (q?.slot === "K") q.c = 1;
  }
};
