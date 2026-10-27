// City tiles and interiors for the site (docs/CITY_DESCRIPTION.md 4, 7): street cobbles, square
// paving, grass, the city wall; interior floor and wall, and furniture objects (counter, table,
// stool, barrel, forge, anvil, the guild board). 64 px cells, never packed (ids 2000+).
const T = require("./tiles_lib");

const tile = (id, name, build, comment) => ({ id, name, size: 64, layer: "tile", flags: "no_outline", comment: comment ? [comment] : [], build });
const obj = (id, name, build, comment) => ({ id, name, size: 64, layer: "tile", comment: comment ? [comment] : [], build });

const cobbles = { floorRowH: [6, 9], floorW: [6, 11] };
const paving = { floorRowH: [18, 28], floorW: [22, 34], floorDetail: (s, r, v) => { if (v === 1) T.scatter(s, 0, 0, 64, 64, r, 30, "S", 3, "S"); } };
const wallStyle = {
  capH: 20,
  // The wall walk seen from above: flagstones, the merlons along the outer edge.
  cap(s, r, x0, y0, x1, y1) {
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) s.px(x, y, "P", 2, false);
    for (let x = x0; x < x1; x += 16) for (let y = y0; y < y1; y++) s.px(x, y, "P", 0, false);
    for (let y = y0 + 10; y < y1; y += 14) for (let x = x0; x < x1; x++) s.px(x, y, "P", 0, false);
    T.scatter(s, x0, y0, x1, y1, r, 8, "P", 3, "P");
  },
  face(s, r, x0, y0, x1, y1) {
    T.stones(s, x0, y0, x1, y1 - 3, r, { rowH: [8, 10], w: [12, 18], cracks: 0.3 });
    for (let y = y1 - 3; y < y1; y++) for (let x = x0; x < x1; x++) s.px(x, y, "P", y === y1 - 1 ? 0 : 1, false);
  },
};

// Interior: planked floor, a wall with a wainscot of planks under plaster.
const innerWall = {
  capH: 14,
  cap(s, r, x0, y0, x1, y1) { for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) s.px(x, y, "P", 1, false); },
  face(s, r, x0, y0, x1, y1) {
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) s.px(x, y, "P", (x * 7 + y * 5) % 11 === 0 ? 2 : 3, false);
    for (let y = y1 - 20; y < y1; y++) for (let x = x0; x < x1; x++) s.px(x, y, "S", (x - x0) % 8 === 0 ? 1 : y === y1 - 20 ? 4 : 2, false);
    for (let x = x0; x < x1; x++) s.px(x, y1 - 1, "S", 0, false);
  },
};

const models = [
  tile(2000, "city_street_0", T.floor(cobbles, 2000, 0), "City street: small cobbles."),
  tile(2001, "city_street_1", T.floor(cobbles, 2001, 1)),
  tile(2002, "city_square_0", T.floor(paving, 2002, 0), "City square: large paving slabs."),
  tile(2003, "city_square_1", T.floor(paving, 2003, 1)),
  tile(2004, "city_grass_0", (s) => require("./forest_grass")(2004, 0)(s), "Grass between the houses."),
  tile(2005, "city_grass_1", (s) => require("./forest_grass")(2005, 1)(s)),
  tile(2006, "city_wall_10", T.wall(wallStyle, 10, 2006), "City wall: straight, end and post shapes (bit 0 N, 1 E, 2 S, 3 W)."),
  tile(2007, "city_wall_5", T.wall(wallStyle, 5, 2007)),
  tile(2008, "city_wall_0", T.wall(wallStyle, 0, 2008)),
  tile(2010, "interior_floor_0", T.planks(2010, 0), "Interior floor: planks."),
  tile(2011, "interior_floor_1", T.planks(2011, 1)),
  tile(2012, "interior_wall_10", T.wall(innerWall, 10, 2012), "Interior wall: plaster over a plank wainscot."),
  tile(2013, "interior_wall_5", T.wall(innerWall, 5, 2013)),
  // Furniture, outlined objects on a transparent cell (P wood, S iron, A brass, E fire).
  obj(2020, "furniture_counter", (s) => {
    s.poly([[2, 30], [62, 30], [62, 36], [2, 36]], "P", (x, y) => (y < 32 ? 4 : 3));
    s.poly([[3, 36], [61, 36], [61, 60], [3, 60]], "P", (x) => (x % 10 === 0 ? 1 : 2));
    for (const [x, y] of [[14, 26], [24, 24], [44, 26]]) { s.ell(x, y, 3, 4, "A", {}); s.px(x - 1, y - 2, "A", 5); }
  }, "Counter: a long bar with tankards on it."),
  obj(2021, "furniture_table", (s) => {
    s.poly([[8, 30], [56, 30], [60, 40], [4, 40]], "P", (x, y) => (y < 34 ? 4 : 3));
    s.line(4, 41, 60, 41, "P", 1);
    for (const x of [10, 52]) for (let y = 42; y < 60; y++) { s.px(x, y, "P", 2, false); s.px(x + 1, y, "P", 1, false); }
    s.ell(24, 32, 3, 2, "A", {}); s.ell(38, 33, 2, 3, "E", {}); s.px(38, 31, "E", 5);
  }, "Table: planks on trestles, a tankard and a candle."),
  obj(2022, "furniture_stool", (s) => {
    s.ell(32, 40, 10, 4, "P", {}); for (const x of [25, 39]) s.line(x, 43, x - 2, 58, "P", 2); s.line(32, 44, 32, 58, "P", 1);
  }, "Stool."),
  obj(2023, "furniture_barrel", (s) => {
    s.mass([["e", 32, 42, 13, 17]], "P", { k: 1.5, relief: 1.4 });
    for (const y of [30, 52]) for (let x = 19; x <= 45; x++) if (s.at(x, y)?.slot === "P") { s.px(x, y, "S", 3, false); s.px(x, y + 1, "S", 1, false); }
    s.ell(32, 26, 11, 3, "P", { tone: 1 });
  }, "Barrel with iron hoops."),
  obj(2024, "furniture_forge", (s) => {
    T.stones(s, 6, 30, 58, 60, T.rnd(2024), { rowH: [7, 9], w: [9, 14], cracks: 0.2 });
    for (let x = 10; x < 54; x++) { s.px(x, 30, "E", 5, false); s.px(x, 31, "E", 4, false); }
    for (const fx of [16, 26, 36, 46]) s.poly([[fx - 4, 30], [fx, 16 - (fx % 3) * 2], [fx + 4, 30]], "E", (px, py) => (py < 22 ? 5 : 4));
  }, "Forge: a stone hearth of glowing coals."),
  obj(2025, "furniture_anvil", (s) => {
    s.poly([[8, 30], [52, 30], [58, 34], [46, 38], [40, 38], [42, 48], [24, 48], [26, 38], [12, 36]], "S", (x, y) => (y < 33 ? 4 : x < 30 ? 3 : 2));
    s.mass([["c", [[33, 50, 9], [33, 56, 10]]]], "P", { k: 1.4 });
    for (const y of [52, 56]) s.line(24, y, 42, y, "P", 1, false);
  }, "Anvil on a stump."),
  obj(2026, "furniture_board", (s) => {
    for (const x of [12, 50]) for (let y = 14; y < 60; y++) { s.px(x, y, "P", 2, false); s.px(x + 1, y, "P", 1, false); }
    s.poly([[8, 14], [56, 14], [56, 44], [8, 44]], "P", (x, y) => (y < 17 ? 4 : 2));
    for (const [x, y, w, h] of [[12, 18, 12, 14], [27, 20, 11, 12], [41, 18, 11, 16], [16, 34, 14, 8], [34, 34, 12, 8]]) {
      for (let yy = y; yy < y + h; yy++) for (let xx = x; xx < x + w; xx++) s.px(xx, yy, "A", yy === y ? 4 : 3, false);
      for (let yy = y + 3; yy < y + h - 1; yy += 2) s.line(x + 2, yy, x + w - 3, yy, "A", 1, false);
      s.px(x + w / 2 | 0, y, "S", 4);
    }
  }, "The guild board in the tavern: orders pinned on a plank board."),
];
module.exports = models;
