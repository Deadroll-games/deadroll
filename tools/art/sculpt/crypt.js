// Crypt tiles (ids 800..834): cut stone walls of cold grey-violet blocks with skull niches, heavy
// tomb slabs with carved lines, grave soil with bones, an iron grate door, black still water.
// Materials: crypt_wall, crypt_floor, crypt_door (tools/art TILE_LOOKS).
const T = require("./tiles_lib");
const biome = require("./biome");

const style = {
  capH: 22,
  // The top: dark flat slabs with straight joints (cut stone, not rough rock).
  cap(s, r, x0, y0, x1, y1) {
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) s.px(x, y, "P", 1, false);
    for (let x = x0 + 16; x < x1; x += 21) for (let y = y0; y < y1; y++) s.px(x, y, "P", 0, false);
    for (let y = y0 + 15; y < y1; y += 20) for (let x = x0; x < x1; x++) s.px(x, y, "P", 0, false);
    T.scatter(s, x0, y0, x1, y1, r, 8, "P", 2, "P");
  },
  // The face: regular cut blocks, a skull niche in some cells, a plinth at the foot.
  face(s, r, x0, y0, x1, y1) {
    T.stones(s, x0, y0, x1, y1 - 4, r, { rowH: [10, 11], w: [16, 22], cracks: 0.3, tone: -1 });
    for (let y = y1 - 4; y < y1; y++) for (let x = x0; x < x1; x++) s.px(x, y, "P", y === y1 - 4 ? 3 : y === y1 - 1 ? 0 : 1, false);
    if (r() < 0.5) {
      const nx = x0 + 18 + Math.floor(r() * 18), ny = y0 + 8;
      for (let y = ny; y < ny + 14; y++) for (let x = nx; x < nx + 12; x++) s.px(x, y, "O", 0, false);
      for (let x = nx; x < nx + 12; x++) s.px(x, ny + 14, "P", 3, false);
      s.patch(nx + 2, ny + 5, [
        ".jkkj...",
        "jkkkkj..",
        "jmkkmj..",
        ".jmjj...",
        ".jjjj...",
      ].map((row) => row.replace(/j/g, "j").replace(/k/g, "k")));
    }
  },
  floorRowH: [18, 26], floorW: [24, 34],
  // Tomb slabs: carved lines and a cross on some, cracks, bones on the ground variant.
  floorDetail(s, r, v) {
    if (v === 1) { s.line(20, 18, 20, 44, "P", 1); s.line(12, 26, 28, 26, "P", 1); s.line(21, 18, 21, 44, "P", 3); }
    if (v === 2) for (let i = 0; i < 3; i++) { let x = 8 + Math.floor(r() * 48), y = 8 + Math.floor(r() * 48); for (let k = 0; k < 10; k++) { s.px(x, y, "S", 0, false); x += r() < 0.5 ? 1 : -1; y += 1; } }
    if (v === 3) { for (const [x, y] of [[40, 40], [44, 42], [36, 44]]) { s.line(x, y, x + 6, y - 2, "A", 4); s.px(x - 1, y, "A", 4); s.px(x + 7, y - 2, "A", 4); } }
  },
};

// Grave soil (S dirt) with bones (A), a skull on one variant, stones (P).
const grave = (seed, v) => (s) => {
  T.ground(seed, 0)(s);
  const r = T.rnd(seed + 7);
  for (let i = 0; i < 3 + v * 2; i++) { const x = 6 + Math.floor(r() * 50), y = 6 + Math.floor(r() * 50); s.line(x, y, x + 5, y + (r() < 0.5 ? 1 : -1), "A", 4); s.px(x - 1, y, "A", 4); }
  if (v === 2) s.patch(30, 30, [
    ".kkkk.",
    "kkkkkk",
    "kmkkmk",
    ".kmkk.",
    ".k.k..",
  ]);
};

// Door: an iron grate (S blackiron) in a pointed stone arch.
const door = (s) => {
  const r = T.rnd(826);
  style.cap(s, r, 0, 0, 64, 22);
  for (let x = 0; x < 64; x++) s.px(x, 21, "P", 4, false);
  T.stones(s, 0, 22, 64, 60, r, { rowH: [10, 11], w: [16, 22], cracks: 0.2, tone: -1 });
  for (let y = 60; y < 64; y++) for (let x = 0; x < 64; x++) s.px(x, y, "P", y === 60 ? 3 : 1, false);
  const inArch = (x, y) => x >= 18 && x < 46 && (y >= 36 || Math.abs(x - 32) <= (y - 22) * 0.9);
  for (let y = 22; y < 64; y++) for (let x = 14; x < 50; x++) {
    const outer = x >= 15 && x < 49 && (y >= 36 || Math.abs(x - 32) <= (y - 18) * 0.95);
    if (outer && !inArch(x, y)) s.px(x, y, "P", x < 32 ? 4 : 2, false);
    if (inArch(x, y)) s.px(x, y, "O", 0, false);
  }
  for (let x = 20; x < 46; x += 5) for (let y = 26; y < 63; y++) if (inArch(x, y)) { s.px(x, y, "S", 3, false); s.px(x + 1, y, "S", 1, false); }
  for (const y of [40, 52]) for (let x = 18; x < 46; x++) { s.px(x, y, "S", 4, false); s.px(x, y + 1, "S", 1, false); }
  for (let x = 20; x < 46; x += 5) s.px(x, 62, "S", 5, false);
};

module.exports = biome({ base: 800, prefix: "crypt", title: "Crypt", style, door, ground: grave });
