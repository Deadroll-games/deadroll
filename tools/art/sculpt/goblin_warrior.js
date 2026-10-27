// Goblin warrior: the goblin canon at 1.12 scale, rusty half-helmet, leather armor, round shield on
// the back arm in front of the belly, a hatchet forward in the near hand. Feet on y = 91.
const G = require("./goblin_base");
module.exports = {
  id: 404, name: "goblin_warrior", size: 96,
  comment: [
    "Goblin warrior, facing left: bigger than a goblin, a dented rusty half-helmet with a nose",
    "guard, a leather jerkin with a belt, a round shield, a hatchet forward. Feet on y = 91.",
  ],
  build(s) {
    s.curve([0.66, 0.36, -0.02]);
    s.transform(1.12, 48, 91, 0, 0);
    G.back(s, { earDown: true });
    G.body(s, { noLoincloth: true });
    // Leather jerkin over the torso, a skirt of strips.
    s.ell(49, 64, 9.6, 10.4, "H");
    s.poly([[40, 70], [57, 70], [57, 79], [54, 77], [52, 80], [49, 77], [46, 80], [43, 77], [40, 79]], "H",
      (x, y) => (y < 72 ? 3 : 2));
    G.head(s);
    // Near arm forward and down, the hand holding the hatchet (same pose as the goblin's blade).
    s.cap([[44, 60, 3.6], [39, 68, 2.8], [32, 67, 2.6]], "K");
    s.ell(30, 67, 3.2, 3, "K");
    s.seams();
    // Helmet sits on the skull down to the brow, over the ears' roots; drawn after the seams.
    s.poly([[30, 49], [31, 45], [35, 42], [41, 41], [47, 42], [50, 45], [51, 49]], "R",
      (x, y) => (y < 43 && x < 42 ? 3 : y < 46 ? 2 : 1));
    s.line(30, 49, 51, 49, "R", 0);
    s.poly([[30, 49], [32, 49], [31, 53], [30, 53]], "R", 2);
    s.px(35, 43, "R", 4); // glint
    for (const [x, y] of [[42, 45], [43, 45], [45, 46], [39, 47], [40, 47], [47, 47]]) s.px(x, y, "H", 2); // rust patches
    s.px(47, 44, "R", 0); // dent
    // Jerkin details: collar, belt with a buckle, strap seams.
    s.line(43, 56, 54, 56, "H", 1);
    s.line(41, 70, 57, 70, "H", 1);
    s.px(44, 70, "R", 4); s.px(45, 70, "R", 3);
    // Riveted leather bands across the chest and belly.
    for (const by of [60, 65]) {
      for (let x = 40; x <= 57; x++) {
        const p0 = s.at(Math.round(48 + (x - 48) * 1.12), Math.round(91 + (by - 91) * 1.12));
        if (p0 && p0.slot === "H") { s.px(x, by, "H", 3); s.px(x, by + 1, "H", 1); }
      }
      for (const x of [43, 47, 51]) s.px(x, by, "R", 4);
    }
    // Hatchet: haft through the fist, a bearded head forward.
    s.line(33, 72, 29, 57, "H", 2); s.line(34, 72, 30, 57, "H", 3);
    // Bearded blade: straight back on the haft, a curved cutting edge forward.
    s.poly([[30, 60], [27, 60], [24, 56], [21, 57], [20, 61], [21, 65], [24, 68], [27, 64], [30, 64]], "R",
      (x) => (x > 26 ? 1 : 2));
    s.line(24, 56, 21, 57, "R", 4); s.line(20, 58, 20, 63, "R", 4); s.line(21, 65, 23, 67, "R", 3);
    s.px(25, 61, "H", 2); s.px(24, 63, "H", 2); // rust
    // Round shield on the back arm, in front of the belly: iron rim, planks, a rusty boss.
    s.ell(56, 71, 6.5, 7.5, "R", { tone: -2 });
    s.ell(56, 71, 5.3, 6.3, "H", {});
    for (const lx of [53, 56, 59]) s.line(lx, 66, lx, 76, "H", 1);
    s.ell(56, 71, 1.8, 2, "R", { tone: -1 });
    s.px(55, 70, "H", 3); // rust on the boss
    G.face(s, { noScars: true, teeth: 4 });
  },
};
