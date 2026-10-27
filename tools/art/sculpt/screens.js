// Site screens: the parchment frame for the dungeon map window (one picture, cut into 128 px
// pieces, a layout puts it back) and the loading-screen layouts per biome (made of our tiles,
// entrances and monsters; written by `layouts()`). Never packed (ids 2300+).
const T = require("./tiles_lib");
const { picture, pieces } = require("./site_lib");

// Parchment: an unrolled sheet between two rolls (P parchment, S the rolls' dark ends), torn edges,
// stains; the middle stays plain for the map. 384 x 256.
const sheet = picture(384, 256, (s) => {
  const r = T.rnd(2300);
  const top = (x) => 22 + Math.round(Math.sin(x / 9) * 1.5 + (r() < 0.08 ? 3 : 0));
  const bot = (x) => 234 - Math.round(Math.cos(x / 11) * 1.5 + (r() < 0.08 ? 3 : 0));
  for (let x = 36; x < 348; x++) {
    const t = top(x), b = bot(x);
    for (let y = t; y <= b; y++) {
      const edge = Math.min(y - t, b - y, x - 36, 347 - x);
      const h = (((x * 73856093) ^ (y * 19349663)) >>> 0) % 31;
      s.px(x, y, "P", edge < 2 ? 2 : edge < 6 ? 3 : h === 0 ? 3 : 4, false);
    }
  }
  // Stains and wear.
  // Water stains: only a darker ragged rim, the inside stays the paper color.
  for (let i = 0; i < 5; i++) {
    const cx = 60 + r() * 264, cy = 40 + r() * 176, w = 8 + r() * 16, h = 6 + r() * 10;
    for (let t = 0; t < Math.PI * 2; t += 0.03) {
      if (r() < 0.3) continue;
      const x = Math.round(cx + Math.cos(t) * w * (0.9 + r() * 0.2)), y = Math.round(cy + Math.sin(t) * h * (0.9 + r() * 0.2));
      const p = s.at(x, y); if (p && p.slot === "P" && p.c > 2) p.c = 3;
    }
  }
  // The rolls at both sides: cylinders with spiral ends.
  for (const x0 of [8, 348]) {
    for (let y = 14; y < 244; y++) for (let x = x0; x < x0 + 28; x++) {
      const t = (x - x0) / 27;
      s.px(x, y, "P", t < 0.2 ? 4 : t < 0.55 ? 3 : t < 0.85 ? 2 : 1, false);
    }
    for (const yc of [14, 243]) for (let x = x0; x < x0 + 28; x++) s.px(x, yc, "S", 2, false);
    for (let t = 0; t < Math.PI * 5; t += 0.15) { const rr = 1 + t * 0.7; s.px(Math.round(x0 + 14 + Math.cos(t) * rr), Math.round(232 + Math.sin(t) * rr * 0.5), "S", 1, false); }
    for (let y = 30; y < 220; y += 22) s.line(x0 + 20, y, x0 + 20, y + 10, "P", 1, false);
  }
});

const parch = pieces(sheet, 2300, "parchment");
module.exports = parch.models;

// Loading screens: 384 x 256, the biome's entrance in the middle of its floor and walls, its
// monsters gathered around, a dark vignette drawn by the background color.
module.exports.layouts = () => {
  const out = { parchment: ["canvas 384 256", ...parch.layout] };
  const biomes = {
    caves: { wall: (m) => (m === 10 ? 501 : 507 + (m < 10 ? m : m - 1)), floor: [500, 522, 523, 524], ent: "tile_entrance", mon: [["goblin", 1, 2], ["goblin_warrior", 2, 3], ["troll", 4, 3], ["goblin_shaman", 4, 1]] },
    forest: { wall: (m) => 540 + m, floor: [563, 564, 565, 563], ent: "forest_entrance", mon: [["treant", 1, 2], ["wolf", 2, 3], ["bear", 4, 3], ["wood_fairy", 4, 1]] },
    crypt: { wall: (m) => 800 + m, floor: [816, 817, 818, 819], ent: "crypt_entrance", mon: [["skeleton", 1, 2], ["ghoul", 2, 3], ["reaper", 4, 3], ["skeleton_warrior", 4, 1]] },
  };
  const map = ["WWWWWW", "W....W", "W....W", "......"];
  for (const [name, b] of Object.entries(biomes)) {
    const L = ["canvas 384 256", "bg #08070a"];
    for (let y = 0; y < map.length; y++) for (let x = 0; x < map[y].length; x++) {
      if (map[y][x] === "W") {
        const w = (dx, dy) => (map[y + dy]?.[x + dx] === "W" ? 1 : 0);
        L.push(`put ${b.wall(w(0, -1) | (w(1, 0) << 1) | (w(0, 1) << 2) | (w(-1, 0) << 3))} ${x * 64} ${y * 64}`);
      } else L.push(`put ${b.floor[(x + y) % 4]} ${x * 64} ${y * 64}`);
    }
    L.push(`put ${b.ent} ${2.5 * 64} 64`);
    for (const [m, x, y] of b.mon) L.push(`cell ${m} ${x} ${y}`);
    out[`loading_${name}`] = L;
  }
  return out;
};
