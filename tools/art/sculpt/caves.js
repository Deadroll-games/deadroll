// Caves and Ruins tiles: ruined masonry walls (16 autotile shapes), cave floors, water, door, stairs,
// rubble, chasm, void. Ids: 500 floor, 501 wall E+W (the shape the current map uses), 502 shallow
// water, 503 door keep their old ids; the rest from 507 (FORMAT.md 1.5, tools/art/README.md).
const T = require("./tiles_lib");

const style = {
  capH: 22,
  // The top of the wall: big rough dark slabs.
  cap(s, r, x0, y0, x1, y1) {
    // A dark rough top: one base tone, a few large low-contrast slabs, cracks, sparse moss.
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) s.px(x, y, "P", 1, false);
    for (let i = 0; i < 3; i++) {
      const cx = x0 + 6 + r() * (x1 - x0 - 12), cy = y0 + 4 + r() * Math.max(1, y1 - y0 - 8);
      const w = 6 + r() * 8, h = 4 + r() * Math.min(8, (y1 - y0) / 3);
      s.poly([[cx - w, cy - h * 0.6], [cx - w * 0.2, cy - h], [cx + w, cy - h * 0.4], [cx + w * 0.7, cy + h], [cx - w * 0.6, cy + h * 0.8]], "P",
        (x, y) => ((x - cx) / w + (y - cy) / h < -0.3 ? 2 : 1));
    }
    for (let i = 0; i < 4; i++) {
      let x = x0 + Math.floor(r() * (x1 - x0)), y = y0 + Math.floor(r() * (y1 - y0));
      for (let k = 0; k < 6; k++) { s.px(x, y, "O", 0, false); x += r() < 0.5 ? 1 : -1; y += 1; if (y >= y1) break; }
    }
    T.scatter(s, x0, y0, x1, y1, r, 6, "S", 1, "P");
  },
  // The front face: irregular blocks, moss in the joints near the foot, a shadow band at the base.
  face(s, r, x0, y0, x1, y1) {
    T.stones(s, x0, y0, x1, y1 - 3, r, { rowH: [9, 11], w: [12, 18], cracks: 0.4, tone: -1 });
    for (let y = y1 - 3; y < y1; y++) for (let x = x0; x < x1; x++) s.px(x, y, "P", y === y1 - 1 ? 0 : 1, false);
    T.scatter(s, x0, y1 - 14, x1, y1 - 3, r, 14, "S", () => 1 + Math.floor(r() * 3), "O");
    T.scatter(s, x0, y0, x1, y1 - 3, r, 6, "S", 2, "O");
  },
};

style.floorRowH = [14, 22]; style.floorW = [16, 30];
// Extras by variant: 0 plain, 1 cracked, 2 pebbles and a bone, 3 a dark stain and moss.
style.floorDetail = (s, r, v) => {
  if (v === 1) for (let i = 0; i < 3; i++) {
    let x = 8 + Math.floor(r() * 48), y = 8 + Math.floor(r() * 48);
    for (let k = 0; k < 9; k++) { s.px(x, y, "S", 0, false); x += r() < 0.5 ? 1 : 0; y += r() < 0.6 ? 1 : 0; x -= r() < 0.3 ? 1 : 0; }
  }
  if (v === 2) {
    T.scatter(s, 2, 2, 62, 62, r, 18, "P", () => 2 + Math.floor(r() * 3));
    s.line(20, 40, 30, 36, "A", 4); s.line(20, 41, 30, 37, "A", 2); s.px(19, 40, "A", 4); s.px(31, 36, "A", 4);
  }
  if (v === 3) {
    s.ell(36, 30, 9, 6, "S", { tone: -1 });
    T.scatter(s, 2, 2, 62, 62, r, 16, "S", () => 2 + Math.floor(r() * 2), "S");
  }
};

const wallId = (m) => (m === 10 ? 501 : 507 + (m < 10 ? m : m - 1));
const models = [];
for (let m = 0; m < 16; m++) {
  models.push({
    id: wallId(m), name: m === 10 ? "tile_wall" : `cave_wall_${m}`, size: 64, layer: "tile", flags: "no_outline",
    comment: m === 0 ? ["Caves and Ruins: wall shapes by the neighbor mask (bit 0 N, 1 E, 2 S, 3 W walls)."] : [`wall mask ${m}`],
    build: T.wall(style, m, 100 + m),
  });
}
// Floors: 500 is variant 0 (the id the current map uses), 522..524 the other variants.
for (let v = 0; v < 4; v++) {
  models.push({
    id: v === 0 ? 500 : 521 + v, name: v === 0 ? "tile_floor" : `cave_floor_${v}`, size: 64, layer: "tile", flags: "no_outline",
    comment: v === 0 ? ["Caves and Ruins: worn flagstones with dirt in the joints, four variants."] : [`floor variant ${v}`],
    build: T.floor(style, 200 + v, v),
  });
}
// Door: a wall cell (E and W walls) whose face is a stone arch (S) around an iron-bound door (P wood,
// A iron), a ring pull.
const door = (s) => {
  const r = T.rnd(503);
  for (let y = 0; y < 22; y++) for (let x = 0; x < 64; x++) s.px(x, y, "S", 1, false);
  for (let x = 0; x < 64; x++) { s.px(x, 0, "S", 3, false); s.px(x, 21, "S", 4, false); }
  T.stones(s, 0, 22, 64, 61, r, { slot: "S", rowH: [9, 11], w: [12, 18], tone: -1, cracks: 0.3 });
  for (let y = 61; y < 64; y++) for (let x = 0; x < 64; x++) s.px(x, y, "S", y === 63 ? 0 : 1, false);
  // The arch opening and the door in it.
  const inArch = (x, y) => x >= 18 && x < 46 && (y >= 34 || (x - 32) * (x - 32) / 196 + (y - 34) * (y - 34) / 100 <= 1);
  for (let y = 24; y < 64; y++) for (let x = 16; x < 48; x++) {
    const inner = inArch(x, y), outer = x >= 16 && x < 48 && (y >= 34 || (x - 32) * (x - 32) / 256 + (y - 34) * (y - 34) / 144 <= 1);
    if (outer && !inner) s.px(x, y, "S", y < 30 ? 4 : x < 32 ? 3 : 2, false);
    if (inner) s.px(x, y, "P", x % 7 === 3 ? 1 : x < 24 ? 3 : 2, false);
  }
  for (const y of [40, 54]) for (let x = 18; x < 46; x++) { s.px(x, y, "A", 3, false); s.px(x, y + 1, "A", 1, false); }
  for (const [x, y] of [[20, 40], [43, 40], [20, 54], [43, 54]]) s.px(x, y, "A", 5, false);
  s.ell(39, 47, 2, 2, "A", {}); s.px(39, 47, "P", 0, false);
  for (let y = 34; y < 64; y++) s.px(17, y, "O", 0, false), s.px(46, y, "O", 0, false);
};
const add = (id, name, build, comment) => models.push({ id, name, size: 64, layer: "tile", flags: "no_outline", comment: comment ? [comment] : [], build });
add(503, "tile_door", door, "Door in a wall line: a stone arch around an iron-bound door.");
add(502, "tile_water", T.water(502, false), "Shallow water over a stony bottom; crests repeat every 32 px.");
add(531, "cave_water_1", T.water(531, false));
add(532, "cave_deep", T.water(532, true), "Deep water: no bottom.");
for (let v = 0; v < 3; v++) add(525 + v, `cave_wood_${v}`, T.planks(525 + v, v), v === 0 ? "Wooden planks, more rot on later variants." : undefined);
for (let v = 0; v < 3; v++) add(528 + v, `cave_dirt_${v}`, T.ground(528 + v, v), v === 0 ? "Bare ground: stones, bones, a puddle." : undefined);
add(533, "cave_stairs", T.stairs(533), "Stairs going down into the dark.");
add(534, "cave_rubble_0", T.rubble(T.floor(style, 534, 0), 1534), "Rubble: broken stones over the floor.");
add(535, "cave_rubble_1", T.rubble(T.ground(535, 0), 1535));
add(536, "cave_chasm", T.chasm(536), "Chasm: a black pit below a rock rim.");
add(537, "cave_void", T.solid(style, 537), "Void: solid rock.");
module.exports = models;
module.exports.style = style;
module.exports.wallId = wallId;
