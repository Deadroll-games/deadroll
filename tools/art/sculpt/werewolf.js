// Werewolf, 96, facing left: a hunched muscular wolf-man, gray-brown fur (K) with a shaggy mane (H),
// a snarling wolf head with laid-back ears, long arms with claws (R bone), digitigrade legs.
// Feet on y = 91.
module.exports = {
  id: 434, name: "werewolf", size: 96,
  comment: [
    "Werewolf, facing left: hunched and muscular, a shaggy mane, a snarling wolf head with laid-back",
    "ears, long arms with hooked claws, beast legs bent at the hock. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.55, 0.26, -0.12]);
    // Tail and the far limbs (darker).
    s.cap([[58, 58, 2.6], [66, 64, 2.2], [70, 72, 1.4]], "H", { tone: -1 });
    s.cap([[53, 58, 4], [50, 70, 3], [56, 80, 2.2]], "K", { tone: -1 });
    s.cap([[56, 80, 2], [54, 88, 1.8]], "K", { tone: -1 });
    s.poly([[50, 88], [57, 88], [57, 91], [49, 91]], "K", 1);
    s.cap([[54, 38, 3.6], [58, 50, 2.8], [55, 62, 2.4]], "K", { tone: -1 });
    // Torso: hunched, a deep chest, a narrow waist.
    s.mass([["c", [[50, 40, 9], [52, 50, 7.5], [53, 58, 5.5]]], ["e", 45, 44, 7.5, 7], ["e", 47, 38, 11, 7]], "K", { k: 2, relief: 2.8 });
    // Shaggy mane rising over the shoulders and the back of the neck.
    s.ell(49, 34, 10, 8, "H", { ragged: 0.5, tufts: 20, tone: 1 });
    // Near leg: thigh forward, shin back, the long foot.
    s.cap([[49, 60, 5.2], [43, 70, 3.4]], "K");
    s.ell(43, 70, 3, 2.8, "K");
    s.cap([[43, 71, 2.8], [49, 80, 2.1]], "K");
    s.cap([[49, 80, 2], [46, 88, 1.8]], "K");
    s.poly([[41, 87], [49, 87], [49, 91], [40, 91]], "K", (x) => (x < 44 ? 4 : 3));
    // Head low and forward: skull, a long muzzle, the open jaw; laid-back ears.
    s.mass([["e", 38, 34, 6, 5.5], ["c", [[34, 37, 3.6], [26, 40, 2.4]]]], "K", { k: 1.4, seam: true });
    s.cap([[34, 43, 2.2], [27, 44, 1.5]], "K", { tone: -1, seam: true });
    s.poly([[39, 31], [46, 21], [46, 32]], "K", (x) => (x < 43 ? 4 : 2));
    s.poly([[42, 30], [50, 23], [48, 33]], "K", 1);
    s.line(42, 29, 45, 24, "D", 2);
    // Near arm reaching forward and down, claws hooked.
    s.cap([[44, 40, 4.2], [38, 50, 3.4], [30, 56, 2.8]], "K", { seam: true });
    s.ell(28, 57, 3, 2.6, "K", { seam: true });
    s.seams();

    // Hooked claws.
    for (const [x, y] of [[24, 58], [25, 60], [27, 61]]) { s.px(x, y, "R", 1); s.px(x - 1, y + 1, "R", 2); s.px(x - 1, y + 2, "R", 4); }
    for (const [dx, dy] of [[-1, 4], [1, 4]]) s.px(55 + dx, 62 + dy, "R", 3);
    // Snarl: dark gap rising to the cheek, fangs, nose.
    const gy = (x) => 42 - Math.round((x - 26) / 5);
    for (let x = 26; x <= 36; x++) s.px(x, gy(x), "D", 1);
    for (const x of [27, 30, 33]) s.px(x, gy(x) - 1, "R", 5);
    for (const x of [28, 31]) s.px(x, gy(x) + 1, "R", 4);
    s.px(24, 39, "D", 2); s.px(24, 40, "D", 1);
    s.line(28, 36, 34, 33, "K", 4); // lit top of the muzzle
    // Brow and the eyes.
    s.line(32, 31, 38, 30, "K", 1);
    s.px(33, 32, "Y", 4); s.px(34, 32, "Y", 5); s.px(37, 32, "Y", 3);
    // Muscles painted in: the deltoid, the chest, the biceps; dark creases under them.
    s.patch(41, 38, [
      "..4444..",
      ".444443.",
      "4443332.",
      "433221..",
    ]);
    s.patch(40, 45, [
      ".3443..",
      "344432.",
      "1222211",
    ]);
    s.line(37, 47, 40, 44, "K", 4, false); s.line(36, 49, 38, 46, "K", 3, false);
    s.line(42, 52, 48, 53, "K", 1);
  },
};
