// Site-only art: menu icons 32x32 (bag, dungeon map, skills, settings), crafting benches 64x64
// (smithing anvil, alchemy table, scribe's shelf, provisions crate), a key and a sealed scroll.
// Never packed (assets/site_sprites, ids 2200+). Benches stay unplaced until guild masters exist.
const T = require("./tiles_lib");
const icon = (id, name, comment, build) => ({ id, name, size: 32, layer: "icon", comment: [comment], build: (s) => { s.curve([0.6, 0.3, -0.06]); build(s); } });
const bench = (id, name, comment, build) => ({ id, name, size: 64, layer: "tile", comment: [comment], build: (s) => { s.curve([0.6, 0.3, -0.06]); build(s); } });

const ui = [
  icon(2200, "ui_bag", "Menu: the bag, a worn leather backpack with straps and a buckle.", (s) => {
    s.mass([["e", 16, 18, 10, 11]], "P", { k: 1.6, relief: 1.5 });
    s.poly([[7, 10], [25, 10], [24, 15], [8, 15]], "P", (x, y) => (y < 12 ? 4 : 3));
    s.poly([[11, 19], [21, 19], [21, 27], [11, 27]], "P", (x) => (x < 14 ? 3 : 2));
    for (const x of [10, 22]) s.line(x, 8, x, 28, "S", 2);
    s.poly([[14, 13], [18, 13], [18, 17], [14, 17]], "A", 4); s.px(16, 15, "O", 0);
    s.cap([[12, 6, 1.4], [16, 3, 1.2], [20, 6, 1.4]], "S", {});
  }),
  icon(2201, "ui_map", "Menu: the dungeon map, a parchment with a red cross and a path.", (s) => {
    s.poly([[4, 7], [27, 7], [28, 26], [5, 26]], "P", (x, y) => ((x + y) % 9 === 0 ? 3 : 4));
    for (const x of [4, 27]) s.cap([[x, 5, 2.2], [x, 28, 2.2]], "S", {});
    for (const [x, y] of [[9, 20], [12, 18], [15, 19], [18, 16], [20, 13]]) s.px(x, y, "S", 1);
    s.line(19, 11, 23, 15, "E", 3); s.line(23, 11, 19, 15, "E", 3);
    s.poly([[8, 10], [13, 9], [12, 14], [7, 13]], "P", 2);
  }),
  icon(2202, "ui_skills", "Menu: skills, an open grimoire with a glowing rune.", (s) => {
    s.poly([[3, 9], [16, 12], [16, 28], [3, 25]], "P", (x) => (x < 7 ? 3 : 4));
    s.poly([[16, 12], [29, 9], [29, 25], [16, 28]], "P", (x) => (x > 25 ? 2 : 3));
    s.poly([[2, 26], [16, 29], [30, 26], [30, 27], [16, 30], [2, 27]], "S", 2);
    for (const y of [15, 18, 21]) s.line(5, y, 13, y + 1, "S", 1, false);
    s.ell(22, 19, 3.6, 3.6, "E", { tone: 1 }); s.px(22, 19, "E", 5); s.line(22, 15, 22, 23, "E", 4, false);
  }),
  icon(2203, "ui_settings", "Menu: settings, an iron gear.", (s) => {
    for (let k = 0; k < 8; k++) { const a = (k / 8) * Math.PI * 2; s.poly([[16 + Math.cos(a - 0.2) * 9, 16 + Math.sin(a - 0.2) * 9], [16 + Math.cos(a - 0.15) * 13, 16 + Math.sin(a - 0.15) * 13], [16 + Math.cos(a + 0.15) * 13, 16 + Math.sin(a + 0.15) * 13], [16 + Math.cos(a + 0.2) * 9, 16 + Math.sin(a + 0.2) * 9]], "P", 2); }
    s.ell(16, 16, 10, 10, "P", {}); s.ell(16, 16, 4, 4, "O", {});
  }),
];

const benches = [
  bench(2210, "bench_smith", "Smithing bench: an anvil on a stump, a hammer resting on it.", (s) => {
    s.mass([["c", [[32, 48, 12], [32, 56, 13]]]], "S", { k: 1.4 });
    for (const y of [50, 54, 58]) s.line(21, y, 43, y, "S", 1, false);
    s.poly([[10, 32], [50, 32], [56, 36], [46, 40], [40, 40], [42, 46], [22, 46], [24, 40], [14, 38]], "P", (x, y) => (y < 35 ? 4 : x < 30 ? 3 : 2));
    s.line(30, 30, 50, 22, "S", 3); s.line(30, 31, 50, 23, "S", 1);
    s.poly([[46, 18], [54, 18], [54, 26], [46, 26]], "P", 3);
  }),
  bench(2211, "bench_alchemy", "Alchemy table: flasks of colored liquids, an alembic, a mortar.", (s) => {
    s.poly([[4, 34], [60, 34], [60, 40], [4, 40]], "S", (x, y) => (y < 36 ? 4 : 3));
    for (const x of [8, 54]) for (let y = 40; y < 60; y++) { s.px(x, y, "S", 2, false); s.px(x + 1, y, "S", 1, false); }
    for (const [x, c] of [[14, "E"], [26, "A"], [48, "E"]]) { s.ell(x, 29, 4, 4.4, c, {}); s.line(x, 20, x, 24, "P", 3); s.px(x - 1, 27, c, 5); }
    s.mass([["e", 37, 26, 5, 5]], "A", {}); s.line(41, 22, 46, 16, "A", 3); s.line(46, 16, 50, 18, "A", 3);
    s.ell(54, 32, 3.4, 2, "P", {});
  }),
  bench(2212, "bench_scribe", "Scribe's shelf: books and scrolls on a tall shelf.", (s) => {
    s.poly([[8, 4], [56, 4], [56, 62], [8, 62]], "S", (x) => (x < 12 ? 3 : 2));
    for (const y of [20, 36, 52]) for (let x = 10; x < 55; x++) { s.px(x, y, "S", 4, false); s.px(x, y + 1, "S", 1, false); }
    for (const y0 of [6, 22, 38]) { let x = 11; while (x < 52) { const w = 2 + ((x * 7 + y0) % 3), h = 10 + ((x + y0) % 4); for (let yy = y0 + 13 - h; yy < y0 + 13; yy++) for (let xx = x; xx < x + w; xx++) s.px(xx, yy, (x + y0) % 3 ? "P" : "A", xx === x ? 4 : 2, false); x += w + 1; } }
  }),
  bench(2213, "bench_provisions", "Provisions crate: an open crate of sacks and a shovel.", (s) => {
    s.poly([[6, 30], [58, 30], [58, 60], [6, 60]], "S", (x, y) => (y < 33 ? 4 : x % 13 === 0 ? 1 : 2));
    for (const [x, y] of [[18, 28], [32, 26], [46, 28]]) { s.mass([["e", x, y, 7, 6]], "P", { k: 1.2 }); s.line(x - 2, y - 5, x + 2, y - 5, "P", 1); }
    s.line(50, 10, 52, 30, "S", 3); s.poly([[49, 6], [55, 6], [54, 12], [50, 12]], "A", 3);
  }),
  bench(2214, "item_key", "A big iron key.", (s) => {
    s.ell(18, 32, 9, 9, "P", {}); s.ell(18, 32, 4.4, 4.4, "O", {});
    s.cap([[26, 32, 2.6], [54, 32, 2.6]], "P", {});
    for (const [x, h] of [[46, 7], [52, 5]]) s.poly([[x - 2, 33], [x + 2, 33], [x + 2, 33 + h], [x - 2, 33 + h]], "P", 3);
  }),
  bench(2215, "item_scroll", "A rolled scroll with a wax seal.", (s) => {
    s.cap([[12, 44, 7], [52, 20, 7]], "P", {}); s.ell(11, 45, 5, 6, "P", { tone: -1 }); s.ell(53, 19, 5, 6, "P", { tone: 1 });
    s.line(28, 40, 36, 26, "S", 2); s.ell(34, 34, 4, 4, "A", {}); s.px(33, 33, "A", 5);
  }),
];

module.exports = [...ui, ...benches];
module.exports.ui = ui;
module.exports.benches = benches;
