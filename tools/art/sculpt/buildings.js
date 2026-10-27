// City buildings for the site (docs/CITY_DESCRIPTION.md 4, 8): Tavern (4x3 cells), Artisans' Guild
// (3x3), Storage (3x2), each with roof headroom above its base, 64 px cells, 3/4 view like the
// dungeon tiles. Slots: P walls (stone, plaster), S timber and doors, A roof, E lit windows and fire.
// Exported: models (128 px pieces) and layouts (`layouts.<name>`) for `art compose`.
const T = require("./tiles_lib");
const { picture, pieces } = require("./site_lib");

// A front wall band of stone blocks with mortar.
const masonry = (s, x0, y0, x1, y1, seed, tone = 0) => T.stones(s, x0, y0, x1, y1, T.rnd(seed), { rowH: [8, 10], w: [10, 16], cracks: 0.25, tone });
// Plaster panels between timbers (P), the timber frame (S).
const halfTimber = (s, x0, y0, x1, y1, seed) => {
  const r = T.rnd(seed);
  for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) s.px(x, y, "P", ((x * 7 + y * 3) % 13 === 0 ? 2 : 3) - (x > (x0 + x1) / 2 ? 1 : 0), false);
  for (let x = x0; x <= x1; x += 24) for (let y = y0; y < y1; y++) { s.px(x, y, "S", 2, false); s.px(x + 1, y, "S", 3, false); s.px(x + 2, y, "S", 1, false); }
  for (const y of [y0, y1 - 3]) for (let x = x0; x < x1; x++) { s.px(x, y, "S", 3, false); s.px(x, y + 1, "S", 2, false); s.px(x, y + 2, "S", 1, false); }
  for (let x = x0; x + 24 <= x1; x += 24) { s.line(x + 2, y0 + 3, x + 22, y1 - 4, "S", 2, false); s.line(x + 3, y0 + 3, x + 23, y1 - 4, "S", 1, false); }
  T.scatter(s, x0, y0, x1, y1, r, 30, "P", 1, "P");
};
// A pitched roof seen from the front and above: rows of tiles across the whole width, staggered
// joints, lighter near the ridge and darker toward the eave; a ridge cap; the eave casts a shadow.
const roof = (s, x0, x1, top, eave, overhang = 6) => {
  const mid = (x0 + x1) / 2;
  for (let y = top; y <= eave; y++) {
    const w = (x1 - x0) / 2 + overhang * ((y - top) / (eave - top));
    const f = (y - top) / (eave - top), row = Math.floor((y - top) / 5), seam = (y - top) % 5 === 4;
    for (let x = Math.round(mid - w); x <= Math.round(mid + w); x++) {
      const joint = (x + (row % 2) * 5) % 10 === 0;
      const base = f < 0.3 ? 3 : f < 0.75 ? 2 : 1;
      const edge = x < mid - w + 3 ? 1 : x > mid + w - 3 ? -1 : 0;
      s.px(x, y, "A", seam || joint ? Math.max(0, base - 2) : Math.max(1, Math.min(4, base + edge + ((y - top) % 5 === 0 ? 1 : 0))), false);
    }
  }
  for (let x = Math.round(mid - (x1 - x0) / 2) - 1; x <= Math.round(mid + (x1 - x0) / 2) + 1; x++) { s.px(x, top - 1, "A", 4, false); s.px(x, top, "A", 3, false); }
  // The shadow of the eave on the wall below.
  for (let y = eave + 1; y <= eave + 3; y++) for (let x = x0; x <= x1; x++) { const q = s.at(x, y); if (q && q.slot !== "O") q.c = Math.max(0, q.c - 1); }
};
// A window: a lit pane (E) in a timber frame, a mullion cross; dark when lit is false.
const windowAt = (s, x, y, w, h, lit = true) => {
  for (let yy = y - 1; yy <= y + h; yy++) for (let xx = x - 1; xx <= x + w; xx++) s.px(xx, yy, "S", 1, false);
  for (let yy = y; yy < y + h; yy++) for (let xx = x; xx < x + w; xx++) s.px(xx, yy, lit ? "E" : "O", lit ? (yy < y + h / 2 ? 4 : 3) : 0, false);
  for (let xx = x; xx < x + w; xx++) s.px(xx, y + Math.floor(h / 2), "S", 2, false);
  for (let yy = y; yy < y + h; yy++) s.px(x + Math.floor(w / 2), yy, "S", 2, false);
};
// A heavy plank door with iron bands.
const doorAt = (s, x, y, w, h) => {
  for (let yy = y; yy < y + h; yy++) for (let xx = x; xx < x + w; xx++) s.px(xx, yy, "S", (xx - x) % 5 === 0 ? 1 : xx < x + w / 2 ? 3 : 2, false);
  for (let xx = x; xx < x + w; xx++) { s.px(xx, y, "S", 4, false); for (const yy of [y + 6, y + h - 7]) s.px(xx, yy, "O", 0, false); }
  s.px(x + w - 4, y + h / 2, "E", 4);
};
// A chimney with loose grey puffs of smoke drifting up and aside.
const chimney = (s, x, y, h) => {
  masonry(s, x, y, x + 10, y + h, x * 7 + y, -1);
  for (let xx = x - 1; xx < x + 11; xx++) { s.px(xx, y, "P", 4, false); s.px(xx, y + 1, "P", 3, false); }
  const r = T.rnd(x + y);
  for (let k = 0; k < 7; k++) {
    const cx = x + 5 + k * 2.4, cy = y - 5 - k * 4.4, rr = 2 + k * 0.5;
    for (let yy = Math.floor(cy - rr); yy <= cy + rr; yy++) for (let xx = Math.floor(cx - rr); xx <= cx + rr; xx++) {
      if ((xx - cx) ** 2 + (yy - cy) ** 2 > rr * rr || r() < 0.35 + k * 0.06) continue;
      if (yy < 0) continue;
      s.px(xx, yy, "P", (xx + yy) % 2 ? 2 : 3, false);
    }
  }
};

const tavern = picture(256, 256, (s) => {
  // Ground floor of stone, upper floor half-timbered, the roof, a side chimney.
  masonry(s, 20, 186, 236, 252, 11);
  halfTimber(s, 16, 130, 240, 186, 12);
  roof(s, 24, 232, 30, 134, 10);
  chimney(s, 196, 34, 40);
  for (const x of [36, 84, 156, 204]) windowAt(s, x, 144, 16, 22);
  for (const x of [40, 196]) windowAt(s, x, 200, 18, 20);
  windowAt(s, 84, 196, 14, 18, false);
  doorAt(s, 116, 200, 26, 52);
  // A lantern by the door, the sign on a bracket: a mug.
  s.line(150, 196, 168, 196, "S", 1); s.line(166, 196, 166, 200, "S", 1);
  s.poly([[158, 200], [176, 200], [176, 216], [158, 216]], "S", (x, y) => (y < 203 ? 4 : 3));
  s.patch(162, 203, ["..gggg..", ".gjjjjg.", ".gjkkjgg", ".gjkkjg.g", ".gjjjjgg.", ".gggggg.."].map((r) => r.padEnd(9, ".")));
  s.ell(110, 206, 2.4, 3, "E", {}); s.px(110, 205, "E", 5);
  // Dormer windows in the roof.
  for (const x of [72, 168]) { s.poly([[x - 12, 104], [x, 86], [x + 12, 104]], "A", 3); windowAt(s, x - 6, 96, 12, 10); }
});

const guild = picture(256, 256, (s) => {
  // A stone workshop with a forge glow under a broad arch, a tall forge chimney, an anvil sign.
  masonry(s, 36, 150, 220, 252, 21);
  roof(s, 40, 216, 62, 152, 8);
  chimney(s, 186, 40, 50);
  for (let y = 196; y < 252; y++) for (let x = 92; x < 164; x++) {
    const inArch = y >= 216 || (x - 128) * (x - 128) / 1296 + (y - 216) * (y - 216) / 400 <= 1;
    const rim = !inArch && (y >= 212 || (x - 128) * (x - 128) / 1600 + (y - 216) * (y - 216) / 576 <= 1);
    if (rim) s.px(x, y, "P", x < 128 ? 4 : 2, false);
    // Inside: dark under the vault, the forge glowing at the back, its light on the floor.
    if (inArch) s.px(x, y, y > 244 ? "E" : "O", y > 244 ? (Math.abs(x - 128) < 18 ? 3 : 2) : 0, false);
  }
  // The forge hearth at the back: a stone box with a bed of glowing coals and flames.
  for (let y = 226; y < 244; y++) for (let x = 108; x < 148; x++) s.px(x, y, "P", y < 229 ? 3 : 1, false);
  for (let x = 112; x < 144; x++) { s.px(x, 229, "E", 5, false); s.px(x, 230, "E", 4, false); }
  for (const fx of [116, 124, 132, 140]) s.poly([[fx - 3, 229], [fx, 219 - (fx % 3) * 2], [fx + 3, 229]], "E", (px, py) => (py < 223 ? 5 : 4));
  // The anvil in front of the fire, a silhouette.
  s.poly([[118, 236], [140, 236], [136, 240], [132, 240], [134, 246], [124, 246], [126, 240], [118, 239]], "O", 0);
  s.line(118, 235, 140, 235, "P", 4, false);
  for (const x of [56, 190]) windowAt(s, x, 168, 14, 22);
  // Anvil sign on a bracket.
  s.line(70, 196, 86, 196, "S", 1);
  s.poly([[74, 200], [90, 200], [86, 205], [80, 205], [82, 211], [76, 211], [78, 205], [74, 204]], "P", 2);
  // A heap of coal and an iron rack at the wall.
  s.ell(200, 246, 12, 5, "O", {}); for (let i = 0; i < 9; i++) s.px(192 + i * 2, 243 + (i % 3), "P", 1);
});

const storage = picture(256, 192, (s) => {
  // A squat warehouse: heavy stone, barred windows, iron-bound double doors, crates outside.
  masonry(s, 36, 98, 220, 188, 31, -1);
  roof(s, 40, 216, 30, 100, 6);
  for (const x of [56, 186]) {
    windowAt(s, x, 114, 14, 14, false);
    for (let xx = x; xx < x + 14; xx += 4) for (let y = 114; y < 128; y++) s.px(xx, y, "S", 3, false);
  }
  doorAt(s, 104, 140, 24, 48); doorAt(s, 128, 140, 24, 48);
  for (const [x, y, w] of [[60, 160, 22], [66, 144, 16], [178, 164, 20]]) {
    s.poly([[x, y], [x + w, y], [x + w, y + w * 0.9], [x, y + w * 0.9]], "S", (px, py) => (py < y + 3 ? 4 : px < x + w / 2 ? 3 : 2));
    s.line(x, y, x + w, y + w * 0.9, "S", 1); s.line(x + w, y, x, y + w * 0.9, "S", 1);
  }
});

const tav = pieces(tavern, 2100, "tavern");
const gld = pieces(guild, 2110, "artisans_guild");
const sto = pieces(storage, 2120, "storage");
module.exports = [...tav.models, ...gld.models, ...sto.models];
module.exports.layouts = {
  tavern: ["canvas 256 256", ...tav.layout],
  artisans_guild: ["canvas 256 256", ...gld.layout],
  storage: ["canvas 256 192", ...sto.layout],
};
