// Wyvern, 96, facing left: a lean drake on two hind legs, the wings are its arms (claws at the
// wrists), a long neck and snapping jaws, a tail ending in a venom sting (Y toxic glow).
// Feet on y = 91.
const wing = require("./bat_wing");
module.exports = {
  id: 441, name: "wyvern", size: 96,
  comment: [
    "Wyvern, facing left: a lean drake on two hind legs, wings for arms with wrist claws, a long",
    "neck with snapping jaws, a tail ending in a glowing venom sting. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.7, 0.45, 0.06]);
    // Far wing (darker), half folded high behind.
    wing(s, [52, 50], [64, 26], [[72, 8], [84, 10], [92, 24], [88, 40]], [62, 56], -1, { bone: "K", skin: "H", arm: 2.4, finger: 3 });
    // Tail curling back and up, the sting.
    s.mass([["c", [[60, 66, 6], [72, 72, 4.5], [82, 70, 3.2], [88, 60, 2]]]], "K", { tone: -1 });
    s.poly([[86, 60], [90, 50], [92, 58]], "R", 3);
    // Far leg (darker).
    s.mass([["c", [[56, 68, 5], [60, 78, 3.6], [57, 88, 2.8]]], ["e", 54, 90, 5, 2]], "K", { tone: -1, seam: true });
    // Body and the neck rising forward.
    s.mass([["e", 54, 62, 12, 9], ["c", [[46, 58, 7], [38, 46, 5.5], [34, 38, 5]]]], "K", { k: 2, relief: 1.8 });
    // Head: a long snout, horns back.
    s.mass([["e", 30, 34, 6, 5], ["c", [[25, 36, 3.6], [16, 38, 2.6]]]], "K", { k: 1.4, seam: true });
    s.mass([["c", [[24, 41, 2.4], [17, 42, 1.8]]]], "K", { tone: -1, seam: true });
    s.cap([[33, 31, 1.8], [40, 26, 1.2], [45, 25, 0.6]], "R", {});
    // Near leg: thigh, shin back, a clawed foot.
    s.mass([["c", [[50, 68, 5.5], [46, 78, 4], [50, 86, 3]]], ["e", 46, 89, 6, 2.4]], "K", { seam: true });
    // Near wing raised, its wrist claw forward.
    wing(s, [48, 52], [40, 22], [[32, 6], [46, 4], [58, 12], [62, 28], [58, 44]], [54, 58], 0, { bone: "K", skin: "H", arm: 2.6, finger: 3 });
    s.seams();

    // Face: eye, nostril, the jaws with fangs and venom drool.
    s.px(28, 33, "Y", 4); s.px(29, 33, "Y", 5);
    s.line(26, 31, 32, 30, "K", 1);
    s.px(15, 37, "D", 1);
    s.line(16, 39, 28, 39, "D", 0);
    for (const x of [17, 20, 23, 26]) s.px(x, 38, "R", 5);
    for (const x of [18, 21, 24]) s.px(x, 40, "R", 4);
    s.px(19, 42, "Y", 4); s.px(19, 43, "Y", 2);
    // Spines on the neck and back, belly band.
    for (const [x, y] of [[38, 42], [44, 50], [56, 53], [64, 56]]) s.poly([[x - 1.4, y + 1], [x + 2, y - 3], [x + 1.4, y + 1]], "R", 2);
    for (let k = 0; k < 10; k += 2) s.px(42 - k * 0.6 | 0, 58 - k, "K", 4);
    // The sting glows with venom.
    s.px(90, 52, "Y", 5); s.px(90, 53, "Y", 3);
    for (const x of [42, 44, 50, 52]) s.px(x, 91, "R", 3);
  },
};
