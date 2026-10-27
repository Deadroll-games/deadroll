// War dog, 96, facing left: a scarred mastiff (K brown) in riveted leather barding (H), a spiked iron
// collar and a muzzle cage (R steel), a snarl, glowing eyes (Y). Paws on y = 91.
module.exports = {
  id: 459, name: "war_dog", size: 96,
  comment: [
    "War dog, facing left: a scarred mastiff in riveted leather barding, a spiked iron collar and a",
    "muzzle cage, snarling, glowing eyes, heavy paws. Paws on y = 91.",
  ],
  build(s) {
    s.curve([0.64, 0.36, -0.02]);
    // Tail, far legs.
    s.cap([[76, 58, 2.4], [82, 52, 1.8], [84, 46, 1.2]], "K", { tone: -1 });
    s.cap([[44, 66, 3.4], [44, 78, 2.6], [45, 88, 2.4]], "K", { tone: -1 });
    s.ell(43, 90, 3.4, 1.6, "K", { tone: -1 });
    s.cap([[70, 64, 4.6], [74, 74, 3], [70, 88, 2.4]], "K", { tone: -1 });
    s.ell(69, 90, 3.4, 1.6, "K", { tone: -1 });
    // Body: a deep chest, a thick neck.
    s.mass([["e", 56, 62, 18, 10], ["e", 40, 60, 10, 11], ["c", [[38, 54, 8], [30, 46, 7]]]], "K", { k: 2.5, relief: 1.8 });
    // Leather barding over the back.
    s.poly([[42, 52], [74, 52], [76, 64], [70, 66], [62, 64], [52, 66], [44, 64]], "H", (x, y) => (y < 55 ? 3 : y < 62 ? 2 : 1));
    for (const x of [48, 56, 64, 72]) { s.px(x, 56, "R", 4); s.px(x, 61, "R", 3); }
    s.line(42, 52, 74, 52, "H", 4);
    // Near legs: thick, the paws broad.
    s.cap([[38, 66, 3.8], [36, 78, 2.8], [37, 88, 2.6]], "K", { seam: true });
    s.ell(35, 90, 4, 1.8, "K", { seam: true });
    s.cap([[72, 64, 5.4], [78, 74, 3.2], [74, 88, 2.6]], "K", { seam: true });
    s.ell(73, 90, 4, 1.8, "K", { seam: true });
    // Head: a broad mastiff skull, short muzzle, the jowls, cropped ears.
    s.mass([["e", 28, 42, 7.5, 7], ["c", [[22, 46, 5], [16, 48, 4]]], ["e", 22, 51, 5, 3]], "K", { k: 1.6, seam: true });
    s.poly([[30, 36], [33, 31], [35, 37]], "K", 2);
    s.seams();

    // Snarl under the cage: the dark gap, fangs.
    s.line(14, 50, 26, 52, "D", 0);
    for (const x of [15, 18, 21, 24]) s.px(x, 50, "K", 5);
    s.px(16, 52, "K", 4); s.px(22, 53, "K", 4);
    // Muzzle cage: iron straps over the snout.
    for (const x of [16, 20, 24]) s.line(x, 43, x, 54, "R", 3, false);
    s.line(13, 47, 27, 47, "R", 2, false); s.line(14, 54, 27, 54, "R", 2, false);
    // Spiked collar.
    s.line(32, 48, 38, 58, "R", 2); s.line(33, 48, 39, 58, "R", 1);
    for (const [x, y] of [[32, 49], [34, 52], [36, 55], [38, 57]]) s.poly([[x, y], [x - 3, y + 1], [x, y + 2]], "R", 4);
    // Eyes, brow, scars.
    s.line(22, 39, 30, 38, "K", 1);
    s.px(24, 41, "Y", 4); s.px(25, 41, "Y", 5);
    s.line(26, 44, 31, 49, "K", 5); s.line(44, 68, 48, 74, "K", 4);
  },
};
