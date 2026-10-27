// Goblin Chieftain, a boss, 2x2 on 128: the goblin body at 1.9 scale, the head at 1.65 (a warlord,
// not a big-headed grunt), a crown of tarnished gold spikes, a fur mantle, a harness with trophy
// skulls, a huge notched cleaver forward. Feet on y = 123.
const G = require("./goblin_base");
const K = 1.9, DX = 17.8, DY = 32;
const cx = (x) => Math.round(DX + 48 + (x - 48) * K), cy = (y) => Math.round(DY + 91 + (y - 91) * K);
// The head: scaled 1.65 about the neck (40, 58), which stays where the body puts it.
const HK = 1.65, HAX = 40, HAY = 58, HDX = cx(40) - 40, HDY = cy(58) - 58;
const hx = (x) => Math.round(HDX + HAX + (x - HAX) * HK), hy = (y) => Math.round(HDY + HAY + (y - HAY) * HK);
module.exports = {
  id: 402, name: "goblin_chief", size: 128,
  comment: [
    "Goblin Chieftain, a boss, facing left: a heavy scarred warlord, a crown of tarnished gold",
    "spikes, a fur mantle, trophy skulls on a harness, a huge notched cleaver forward. Feet on y = 123.",
  ],
  build(s) {
    s.curve([0.66, 0.36, -0.02]);
    s.transform(K, 48, 91, DX, DY);
    G.back(s, { noEar: true });
    s.ell(55, 58, 9.5, 8.5, "H", { ragged: 0.35, tufts: 20 }); // fur mantle
    G.body(s);
    s.ell(49, 63, 10.5, 10.5, "K"); // broad chest
    s.transform(HK, HAX, HAY, HDX, HDY);
    G.farEar(s, { earDown: true });
    G.head(s, { shortNose: true });
    // Crown of tarnished gold: a band on the brow line and four spikes.
    s.poly([[32, 45], [48, 44], [48, 47], [32, 48]], "Y", (x, y) => (y < 46 ? 3 : 2));
    for (const [x, h] of [[34, 3.5], [38, 4.5], [42, 4.5], [46, 3.5]])
      s.poly([[x - 1.2, 45], [x, 45 - h], [x + 1.2, 45]], "Y", (px) => (px < x ? 3 : 2));
    s.transform(K, 48, 91, DX, DY);
    // Near arm forward at hip height, the fist holding the cleaver.
    s.cap([[44, 60, 3.8], [39, 68, 3.2], [32, 67, 3]], "K");
    s.ell(30, 67, 3.2, 3.2, "K");
    s.seams();
    // Cleaver: haft through the fist, a broad blade forward and up.
    s.poly([[30, 72], [31.5, 72], [31, 61], [29.5, 61]], "H", 2);
    s.poly([[31, 63], [22, 53], [17, 56], [18, 61], [26, 67]], "R", (x, y) => (x < 22 ? 3 : 2));
    s.transform(1, 0, 0, 0, 0);

    // Canvas-scale details.
    // Cleaver edge, notches and rust.
    s.line(cx(22), cy(53), cx(17), cy(56), "R", 4); s.line(cx(17), cy(56), cx(18), cy(61), "R", 5);
    s.line(cx(18), cy(61), cx(26), cy(67), "R", 3);
    for (const [x, y] of [[cx(17) + 1, cy(58)], [cx(20), cy(63)]]) { s.erase(x, y); s.erase(x, y + 1); }
    for (const [x, y] of [[cx(24), cy(60)], [cx(26), cy(63)], [cx(22), cy(57)], [cx(27), cy(61)]]) { s.px(x, y, "H", 3); s.px(x + 1, y, "H", 2); }
    // Harness: a strap from the near shoulder to the far hip, a gold buckle, two trophy skulls.
    s.line(cx(44), cy(57), cx(57), cy(70), "H", 1); s.line(cx(45), cy(57), cx(58), cy(70), "H", 3);
    s.line(cx(46), cy(57), cx(58), cy(69), "H", 2);
    const bx = cx(50), by = cy(63);
    s.poly([[bx - 2, by - 2], [bx + 2, by - 2], [bx + 2, by + 2], [bx - 2, by + 2]], "Y", 3);
    s.px(bx, by, "Y", 1); s.px(bx - 1, by - 1, "Y", 4);
    for (const [mx, my] of [[46.5, 60], [54, 67.5]]) {
      const x = cx(mx), y = cy(my);
      s.ell(x, y, 2.6, 2.4, "R", {});
      s.px(x - 1, y, "D", 1); s.px(x + 1, y, "D", 1); s.px(x, y + 2, "D", 1);
    }
    // Belt with a gold buckle; leg wraps.
    s.line(cx(40), cy(70), cx(57), cy(70), "H", 1); s.line(cx(40), cy(70) + 1, cx(57), cy(70) + 1, "H", 1);
    s.poly([[cx(46), cy(70) - 1], [cx(48), cy(70) - 1], [cx(48), cy(70) + 2], [cx(46), cy(70) + 2]], "Y", 3);
    for (let y = cy(80); y <= cy(87); y += 3) s.line(cx(41), y, cx(46), y + 1, "H", 2);
    // Skin texture.
    // Warty skin: sparse 2x2 darker blotches, a lighter pixel on top of each.
    const hash = (x, y) => (((x * 73856093) ^ (y * 19349663)) >>> 0) % 13;
    s.recolor((x, y, c) => (hash(x >> 1, y >> 1) === 0 && c >= 2 ? c - 1 : c), "K");
    s.recolor((x, y, c) => (hash(x >> 1, (y + 1) >> 1) === 0 && hash(x >> 1, y >> 1) !== 0 && c >= 1 && c <= 3 && y % 2 === 1 ? c + 1 : c), "K");
    // Face at the head scale: scars, brow, burning eyes, jagged teeth.
    s.line(hx(39), hy(46), hx(43), hy(49), "K", 5);
    s.line(hx(33), hy(53), hx(36), hy(55), "K", 5);
    s.line(hx(31), hy(47), hx(38), hy(49), "K", 1);
    const ex = hx(33), ey = hy(49) + 1;
    s.px(ex, ey, "Y", 4); s.px(ex + 1, ey, "Y", 5); s.px(ex + 2, ey, "Y", 4); s.px(ex + 1, ey + 1, "Y", 3);
    s.px(hx(37), ey + 1, "Y", 3); s.px(hx(37) + 1, ey + 1, "Y", 2);
    const mx0 = hx(31), my0 = hy(57), mx1 = hx(40), my1 = hy(58);
    s.line(mx0, my0, mx1, my1, "D", 1);
    for (let x = mx0 + 1; x < mx1; x += 3) { s.px(x, my0 - 1 + Math.round(((x - mx0) * (my1 - my0)) / (mx1 - mx0)), "R", 5); }
    for (let x = mx0 + 2; x < mx1; x += 3) { s.px(x, my0 + 1 + Math.round(((x - mx0) * (my1 - my0)) / (mx1 - mx0)), "R", 3); }
    // Claws on the back hand and the toes.
    for (const x of [cx(56), cx(58)]) s.px(x, cy(81), "R", 3);
    for (const x of [cx(36), cx(38), cx(48), cx(50)]) s.px(x, 123, "R", 3);
  },
};
