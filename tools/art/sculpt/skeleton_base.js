// Shared skeleton anatomy, facing left, feet on y = 91: the human canon of bodies.dsl (head 13x14,
// about 74 px tall). K = bone, Y = the glow in the sockets, D = holes and gaps. Callers add gear.
// Joints: shoulder y 35, elbow y 47, wrist y 58, hip y 57, knee y 74, ankle y 88.

// Far limbs, darker: call first.
exports.far = (s, o = {}) => {
  if (!o.noFarArm) {
    s.cap([[50, 35, 1.7], [54, 46, 1.5]], "K", { tone: -1 });
    s.cap([[54, 46, 1.4], [o.farHand?.[0] ?? 55, o.farHand?.[1] ?? 57, 1.2]], "K", { tone: -1 });
  }
  s.cap([[49, 59, 1.6], [53, 73, 1.3]], "K", { tone: -1 });
  s.ell(53, 74, 2, 1.8, "K", { tone: -1 });
  s.cap([[53, 75, 1.2], [51, 88, 1.1]], "K", { tone: -1 });
  s.poly([[48, 88], [53, 88], [53, 91], [47, 91]], "K", 1);
};

// Spine, ribcage, pelvis, near leg.
exports.body = (s) => {
  s.cap([[47, 32, 1.5], [48, 55, 1.5]], "K");
  // Ribs curving forward from the spine; gaps between them stay open.
  for (const y of [36, 39, 42, 45]) {
    const w = y < 44 ? 9 : 7;
    for (let i = 0; i <= w; i++) {
      const x = 47 - i, yy = y + Math.round((i * i) / 28);
      s.px(x, yy, "K", i < 3 ? 2 : 4, false);
      s.px(x, yy + 1, "K", i < 3 ? 1 : 2, false);
    }
  }
  s.line(39, 37, 39, 46, "K", 3, false); // sternum
  // Pelvis: two hip wings and the sacrum between them.
  s.poly([[41, 54], [46, 53], [46, 58], [43, 59]], "K", (x, y) => (y < 56 ? 4 : 3));
  s.poly([[49, 53], [54, 54], [52, 59], [49, 58]], "K", (x, y) => (y < 56 ? 3 : 2));
  s.cap([[47, 54, 1.4], [47, 60, 1]], "K");
  // Near leg: femur, knee, tibia, foot.
  s.cap([[44, 59, 1.7], [41, 73, 1.4]], "K");
  s.ell(41, 74, 2.3, 2.1, "K");
  s.cap([[41, 75, 1.3], [43, 88, 1.2]], "K");
  s.poly([[39, 88], [45, 88], [45, 91], [38, 91]], "K", (x) => (x < 41 ? 4 : 3));
};

// Collarbone and the near arm (shoulder, humerus, elbow, forearm, a bony hand at `hand`).
exports.arm = (s, hand = [34, 57], elbow = [39, 47]) => {
  s.line(40, 34, 50, 34, "K", 4, false); s.line(40, 35, 50, 35, "K", 2, false);
  s.ell(41, 35, 2.2, 2, "K");
  s.cap([[41, 36, 1.8], [elbow[0], elbow[1], 1.5]], "K");
  s.ell(elbow[0], elbow[1], 1.8, 1.6, "K");
  s.cap([[elbow[0], elbow[1], 1.4], [hand[0], hand[1], 1.3]], "K");
  s.ell(hand[0], hand[1], 2.2, 2, "K");
};

// The skull: cranium, cheekbone, the jaw hinged under the ear, sockets, nose hole, teeth.
exports.skull = (s, o = {}) => {
  s.ell(44, 23, 6.6, 6.8, "K");
  s.cap([[43, 27, 2.6], [39, 27, 2]], "K");
  s.cap([[45, 31, 1.7], [41, 31, 1.4]], "K");
  s.cap([[46, 30, 1.5], [47, 33, 1.3]], "K"); // neck vertebra
  s.seams();
  // Sockets: the near one big, the far one a narrow slot (3/4 view), the glow inside.
  s.patch(37, 20, [
    "..mmm.",
    ".mmmmm",
    ".mnkln",
    ".mmmmm",
    "..mm..",
  ]);
  if (o.eyes) for (const [x, y, c] of o.eyes) s.px(x, y, "Y", c);
  // Nose hole, teeth, the gap between the jaws.
  s.px(38, 25, "D", 1); s.px(38, 26, "D", 1);
  s.patch(39, 29, [
    "54545",
    "mmmmm",
    ".4545",
  ]);
  s.px(43, 27, "D", 1); s.px(44, 28, "D", 1); // the hollow under the cheekbone
  // Cracks on the cranium, lit top.
  s.line(46, 17, 48, 21, "D", 1); s.line(48, 21, 47, 23, "D", 1);
  s.line(40, 18, 44, 17, "K", 5, false);
};
