// Map points drawn over their floor cell (the app draws them on top of the tile): the entrance by
// biome, the exit portal, the chest closed and opened. Outlined objects on a transparent cell.
const T = require("./tiles_lib");
const base = { size: 64, layer: "tile" };

// 504: the Caves and Ruins entrance, a timbered mine adit cut into a rock face.
const mine = (s) => {
  const r = T.rnd(504);
  // Rock face around the opening (S cave rock), faceted.
  s.poly([[4, 62], [6, 30], [14, 16], [30, 10], [48, 12], [58, 26], [60, 62]], "S", T.facet(32, 30, 28, 26, 0));
  for (let i = 0; i < 5; i++) { const x = 8 + Math.floor(r() * 48), y = 14 + Math.floor(r() * 40); s.line(x, y, x + 3, y + 4, "S", 0, false); }
  // The dark opening receding into the hill.
  s.poly([[20, 62], [20, 30], [44, 30], [44, 62]], "O", 0);
  for (let y = 31; y < 62; y++) for (let x = 21; x < 44; x++) if ((x + y) % 7 === 0 && y > 44) s.px(x, y, "S", 0, false);
  // Timber frame: posts, a lintel, a brace; a hanging lantern.
  for (const x of [17, 44]) for (let y = 28; y < 62; y++) { s.px(x, y, "P", 3, false); s.px(x + 1, y, "P", 2, false); s.px(x + 2, y, "P", 1, false); }
  for (let x = 15; x < 49; x++) { s.px(x, 26, "P", 4, false); s.px(x, 27, "P", 3, false); s.px(x, 28, "P", 2, false); s.px(x, 29, "P", 1, false); }
  s.line(20, 30, 26, 36, "P", 2); s.line(43, 30, 37, 36, "P", 1);
  for (const x of [18, 30, 45]) s.px(x, 27, "A", 4, false);
  s.line(32, 30, 32, 34, "A", 1); s.ell(32, 37, 2, 2.6, "E", {}); s.px(32, 37, "E", 5);
  // Rails running out of the adit.
  s.line(24, 62, 26, 46, "A", 3); s.line(40, 62, 38, 46, "A", 3);
  for (let y = 48; y < 62; y += 4) s.line(25, y, 39, y, "P", 1);
};

// 505: the exit, a standing portal of stone slabs with a glowing spiral inside (E).
const portal = (s) => {
  const r = T.rnd(505);
  // Two standing stones and a lintel stone, a broad base slab.
  s.poly([[8, 62], [56, 62], [58, 57], [6, 57]], "P", (x, y) => (y < 59 ? 3 : 2));
  s.poly([[10, 57], [12, 14], [20, 12], [21, 57]], "P", T.facet(16, 34, 6, 22, 0));
  s.poly([[43, 57], [44, 12], [52, 14], [54, 57]], "P", T.facet(48, 34, 6, 22, -1));
  s.poly([[6, 14], [8, 6], [56, 6], [58, 14]], "P", (x, y) => (y < 9 ? 4 : 2));
  for (let i = 0; i < 6; i++) { const x = 10 + Math.floor(r() * 44), y = 8 + Math.floor(r() * 48); if (s.at(x, y)?.slot === "P") s.line(x, y, x + 1, y + 3, "P", 0, false); }
  // Runes carved on the standing stones (U), the spiral of light between them.
  for (const [x, y] of [[15, 22], [16, 30], [15, 38], [48, 22], [47, 30], [48, 38]]) { s.px(x, y, "U", 4, false); s.px(x + 1, y, "U", 3, false); s.px(x, y + 1, "U", 3, false); }
  for (let y = 15; y < 57; y++) for (let x = 22; x < 43; x++) s.px(x, y, "E", 1, false);
  for (let t = 0; t < 1; t += 0.004) {
    const a = t * Math.PI * 7, rr = 1 + t * 10;
    const x = Math.round(32 + Math.cos(a) * rr), y = Math.round(36 + Math.sin(a) * rr * 1.7);
    if (x > 21 && x < 43 && y > 14 && y < 57) s.px(x, y, "E", t < 0.3 ? 5 : t < 0.7 ? 4 : 3, false);
  }
  s.px(32, 36, "E", 5);
};

// 580 / 581: the chest, iron-bound wood (P wood, S iron bands, A lock), closed and opened.
const chest = (open) => (s) => {
  // Body: front face (lit left), the side face darker.
  s.poly([[14, 40], [44, 40], [44, 60], [14, 60]], "P", (x, y) => (y < 44 ? 3 : 2));
  s.poly([[44, 40], [52, 35], [52, 55], [44, 60]], "P", 1);
  for (const y of [47, 54]) s.line(14, y, 43, y, "P", 0, false);
  for (const x of [16, 41]) { s.line(x, 40, x, 60, "S", 3); s.line(x + 1, 40, x + 1, 60, "S", 1); }
  s.line(45, 40, 51, 36, "S", 2); s.line(45, 59, 51, 55, "S", 2);
  if (!open) {
    // Arched lid on top.
    s.poly([[14, 40], [14, 33], [18, 28], [40, 28], [44, 33], [44, 40]], "P", (x, y) => (y < 32 ? 4 : 3));
    s.poly([[44, 40], [44, 33], [40, 28], [48, 24], [52, 29], [52, 35]], "P", 2);
    for (const x of [16, 41]) { s.line(x, 30, x, 40, "S", 4); }
    s.line(14, 40, 44, 40, "S", 2);
    s.poly([[27, 39], [31, 39], [31, 45], [27, 45]], "A", (x, y) => (y < 41 ? 4 : 2));
    s.px(29, 43, "O", 0, false);
  } else {
    // The lid thrown back, the inside empty and dark.
    // The lid thrown back behind the box (its underside seen), then the dark empty inside in front.
    s.poly([[22, 35], [20, 16], [48, 12], [52, 30], [51, 35]], "P", (x, y) => (x < 30 ? 2 : 1));
    for (const x of [24, 46]) s.line(x - 1, 34, x, 15, "S", 2);
    s.line(20, 16, 48, 12, "S", 3);
    s.poly([[14, 40], [44, 40], [52, 35], [22, 35]], "O", 0);
    s.poly([[16, 40], [42, 40], [48, 37], [24, 37]], "P", 0);
    s.line(14, 40, 44, 40, "S", 3);
    s.poly([[27, 40], [31, 40], [31, 44], [27, 44]], "A", 2);
  }
};

// 582: the Forest entrance, a path into the thicket under a stone arch overgrown with roots.
const forestGate = (s) => {
  // Thicket behind (S dark leaves), the path (P stone) running into the dark.
  s.ell(32, 24, 28, 18, "S", { ragged: 0.35, tufts: 18 });
  s.poly([[24, 62], [40, 62], [36, 30], [28, 30]], "O", 0);
  for (let y = 34; y < 62; y += 5) s.line(30 - (y - 30) / 8, y, 34 + (y - 30) / 8, y, "E", 1, false);
  // The arch: two pillars and a curved top of stone (P), roots (S) creeping over it.
  for (const x0 of [14, 42]) s.poly([[x0, 62], [x0, 26], [x0 + 8, 24], [x0 + 8, 62]], "P", T.facet(x0 + 4, 44, 4, 18, x0 > 30 ? -1 : 0));
  for (let x = 14; x < 50; x++) { const y = Math.round(22 - Math.sin(((x - 14) / 36) * Math.PI) * 8); for (let k = 0; k < 5; k++) s.px(x, y + k, "P", k === 0 ? 4 : k < 3 ? 3 : 1, false); }
  for (const [a, b] of [[[16, 18], [20, 40]], [[44, 16], [48, 42]], [[30, 14], [26, 22]]]) { s.line(a[0], a[1], b[0], b[1], "S", 3); s.line(a[0] + 1, a[1], b[0] + 1, b[1], "S", 1); }
};

// 583: the Crypt entrance, stone steps going down to a dark doorway under a pediment.
const cryptStair = (s) => {
  s.poly([[8, 62], [8, 20], [32, 8], [56, 20], [56, 62]], "P", T.facet(32, 30, 24, 26, -1));
  s.poly([[20, 62], [20, 30], [44, 30], [44, 62]], "O", 0);
  for (let k = 0; k < 5; k++) { const y = 46 + k * 3, inset = 2 + k; for (let x = 20 + inset; x < 44 - inset; x++) { s.px(x, y, "P", Math.max(1, 3 - k), false); s.px(x, y + 1, "P", 0, false); } }
  for (let x = 8; x < 57; x++) { s.px(x, 28, "P", 4, false); s.px(x, 29, "P", 2, false); }
  s.patch(28, 14, [
    ".kkkk.",
    "kkkkkk",
    "kmkkmk",
    ".kmkk.",
  ]);
  for (const x of [12, 52]) for (let y = 32; y < 62; y += 6) s.line(x - 2, y, x + 2, y, "P", 0, false);
};

module.exports = [
  { ...base, id: 582, name: "forest_entrance", comment: ["Forest entrance: a path into the thicket under a stone arch overgrown with roots."], build: forestGate },
  { ...base, id: 583, name: "crypt_entrance", comment: ["Crypt entrance: stone steps down to a dark doorway under a pediment with a skull."], build: cryptStair },
  { ...base, id: 504, name: "tile_entrance", comment: ["Caves and Ruins entrance: a timbered mine adit in a rock face, rails, a lantern."], build: mine },
  { ...base, id: 505, name: "tile_exit", comment: ["Exit, all biomes: a portal of standing stones with a glowing spiral (E)."], build: portal },
  { ...base, id: 580, name: "chest_closed", comment: ["Chest: iron-bound wood, closed."], build: chest(false) },
  { ...base, id: 581, name: "chest_open", comment: ["Chest: opened, empty and dark inside."], build: chest(true) },
];
