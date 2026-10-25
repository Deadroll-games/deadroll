// Skeleton warrior, 96, facing left: the skeleton with a dented rusty kettle helmet, a round
// wooden shield with an iron rim on the far arm in front of the ribs, a notched sword forward.
const B = require("./skeleton_base");
module.exports = {
  id: 421, name: "skeleton_warrior", size: 96,
  comment: [
    "Skeleton warrior, facing left: a dented rusty kettle helmet, a round wooden shield with an",
    "iron rim held in front of the ribs, a notched sword forward. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.6, 0.3, -0.06]);
    B.far(s, { farHand: [52, 50] });
    B.body(s);
    B.arm(s, [33, 56], [38, 47]);
    B.skull(s);
    // Kettle helmet: a dome over the cranium and a wide brim, rust and a dent.
    s.poly([[38, 19], [40, 15], [45, 13], [50, 15], [52, 19]], "R", (x, y) => (y < 16 && x < 46 ? 3 : y < 18 ? 2 : 1));
    s.line(35, 19, 54, 19, "R", 3); s.line(35, 20, 54, 20, "R", 1);
    for (const [x, y] of [[44, 15], [47, 17], [41, 17], [50, 18]]) s.px(x, y, "H", 2);
    s.px(42, 14, "R", 4); s.px(48, 16, "R", 0);
    // Round shield on the far arm, in front of the ribs: planks, iron rim, a boss.
    s.ell(49, 46, 8, 9, "R", { tone: -2 });
    s.ell(49, 46, 6.8, 7.8, "H", { tone: -1 });
    for (const lx of [45, 49, 53]) s.line(lx, 39, lx, 53, "H", 1, false);
    s.ell(49, 46, 2, 2.2, "R", {});
    s.px(48, 45, "R", 5);
    for (const [x, y] of [[44, 42], [54, 50], [47, 52]]) s.px(x, y, "H", 4); // scratches
    // Sword: grip, guard, a notched blade forward and up.
    s.line(34, 59, 32, 53, "H", 2); s.line(29, 56, 36, 53, "R", 2);
    s.poly([[32, 54], [17, 40], [15, 40], [16, 43], [30, 56]], "R", (x, y) => (x + y < 74 ? 3 : 2));
    s.line(15, 40, 30, 55, "R", 4, false);
    s.erase(20, 46); s.erase(24, 49);
    for (const [x, y] of [[22, 46], [26, 50], [19, 43]]) s.px(x, y, "H", 3);
    s.px(32, 57, "K", 4); s.px(31, 56, "K", 3); s.px(34, 58, "K", 2);
  },
};
