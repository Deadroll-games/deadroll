// Worn items drawn from the presets, waiting for ItemBase values (assets/sprites_pending/items,
// ids 1000+, never packed). Base shapes only: P is the item's material (steel, leather, wood: the
// craft picks it), S straps and grips, A trim and rivets, E the element core, U the rune zone.
// They face right like the bodies and sit on the shared body anchors (body_mask.js).
const B = require("./body_mask");
const T = require("./tiles_lib");
const item = (id, name, layer, comment, build) => ({ id, name, size: 96, layer, comment: [comment], build: (s) => { s.curve([0.6, 0.3, -0.06]); build(s); } });

// Shade a region given by inside(x, y) across the body: lit left, shadowed right, a lit top rim.
const plate = (s, inside, slot, y0, y1, o = {}) => {
  for (let y = y0; y <= y1; y++) {
    let a = -1, b = -1;
    for (let x = 0; x < 96; x++) if (inside(x, y)) { if (a < 0) a = x; b = x; }
    if (a < 0) continue;
    for (let x = a; x <= b; x++) {
      if (!inside(x, y)) continue;
      const t = (x - a) / Math.max(1, b - a);
      const top = !inside(x, y - 1);
      s.px(x, y, slot, top ? 4 : t < 0.25 ? 3 : t < 0.7 ? 2 : 1, false);
    }
  }
};

// A blade along a straight axis from the guard to the tip: width w, a rune strip (U) of 2 px down
// the middle, a lit left edge. Returns nothing; pixels in P and U.
const blade = (s, gx, gy, tx, ty, w, curve = 0) => {
  const n = Math.max(Math.abs(tx - gx), Math.abs(ty - gy)) * 2;
  const L = Math.hypot(tx - gx, ty - gy), nx = -(ty - gy) / L, ny = (tx - gx) / L;
  for (let i = 0; i <= n; i++) {
    const t = i / n, half = (w / 2) * (t < 0.85 ? 1 : (1 - t) / 0.15);
    const cx = gx + (tx - gx) * t + nx * Math.sin(t * Math.PI) * curve, cy = gy + (ty - gy) * t + ny * Math.sin(t * Math.PI) * curve;
    for (let k = -half; k <= half; k += 0.5) {
      const x = Math.round(cx + nx * k), y = Math.round(cy + ny * k);
      const edge = k < -half + 1 ? 4 : k > half - 1 ? 1 : 3;
      s.px(x, y, "P", edge, false);
    }
  }
  // Rune strip: the 2 middle pixels of the blade between 15% and 75% of its length.
  for (let i = 0; i <= n; i++) {
    const t = i / n;
    if (t < 0.15 || t > 0.75) continue;
    const cx = gx + (tx - gx) * t + nx * Math.sin(t * Math.PI) * curve, cy = gy + (ty - gy) * t + ny * Math.sin(t * Math.PI) * curve;
    s.px(Math.round(cx), Math.round(cy), "U", 3, false);
  }
};

// A grip from the hand back to the pommel, a guard across the axis at the hand.
const hilt = (s, hx, hy, px, py, guard) => {
  s.line(hx, hy, px, py, "S", 2); s.line(hx + 1, hy, px + 1, py, "S", 1);
  s.ell(px, py, 1.8, 1.8, "A", {});
  const [ax, ay, bx, by] = guard;
  s.line(ax, ay, bx, by, "A", 3); s.line(ax + 1, ay, bx + 1, by, "A", 1);
};

const models = [
  item(1000, "pending_longsword", "mainhand", "Longsword: a long straight blade up and forward, a cross guard at the fist, a rune strip.", (s) => {
    blade(s, 66, 58, 90, 30, 3.5);
    hilt(s, 63, 62, 58, 68, [61, 55, 70, 62]);
  }),
  item(1001, "pending_dagger", "mainhand", "Dagger: a short curved blade forward, a swept guard.", (s) => {
    blade(s, 66, 59, 82, 46, 3, -2.5);
    hilt(s, 63, 62, 60, 66, [62, 56, 68, 61]);
  }),
  item(1002, "pending_greatsword", "mainhand", "Greatsword: a broad two-handed blade raised up and forward, a long grip, a wide guard.", (s) => {
    blade(s, 67, 56, 92, 14, 5);
    hilt(s, 63, 62, 56, 72, [58, 51, 73, 60]);
  }),
  item(1003, "pending_round_shield", "offhand", "Round shield: planks in the primary material, an iron rim and boss, on the back arm before the body.", (s) => {
    s.ell(38, 52, 12, 13, "S", {});
    s.ell(38, 52, 10.5, 11.5, "P", {});
    for (const x of [33, 38, 43]) s.line(x, 41, x, 63, "P", 1, false);
    s.ell(38, 52, 3.2, 3.4, "A", {}); s.px(37, 51, "A", 5);
    for (const [x, y] of [[30, 46], [46, 46], [30, 58], [46, 58]]) s.px(x, y, "A", 4);
  }),
  item(1004, "pending_spellbook", "offhand", "Spellbook: a thick book held at the back hand, metal corners, a gem on the cover (E).", (s) => {
    s.poly([[29, 54], [43, 50], [45, 66], [31, 70]], "P", T.facet(37, 60, 7, 9, 0));
    s.poly([[43, 50], [46, 51], [48, 67], [45, 66]], "S", 4);
    for (const [x, y] of [[30, 55], [42, 51], [44, 65], [32, 69]]) s.px(x, y, "A", 4);
    s.ell(37, 60, 2.2, 2.4, "E", {}); s.px(36, 59, "E", 5);
    s.line(31, 58, 43, 55, "A", 2, false);
  }),
  item(1005, "pending_great_helm", "helmet", "Great helm: a closed helmet over the whole head, a visor slit, breathing holes, a crest ridge.", (s) => {
    const head = B.grown(2);
    plate(s, (x, y) => head(x, y) && y <= 35, "P", 14, 35);
    for (let x = 41; x <= 56; x++) { s.px(x, 26, "O", 0, false); s.px(x, 27, "O", 0, false); }
    for (let y = 16; y <= 35; y++) if (head(48, y)) s.px(48, y, "P", 4, false);
    for (const [x, y] of [[44, 31], [46, 32], [44, 33]]) s.px(x, y, "O", 0, false);
    for (let x = 40; x <= 57; x++) if (head(x, 35)) s.px(x, 35, "A", 2, false);
  }),
  item(1006, "pending_plate_cuirass", "armor_body", "Plate cuirass: breastplate with a ridge, pauldrons, belly lames, a leather skirt edge.", (s) => {
    const body = B.grown(1);
    const torso = (x, y) => body(x, y) && y >= 35 && y <= 61 && (y < 47 || (x >= 40 && x <= 57));
    plate(s, torso, "P", 35, 61);
    for (const [cx, cy] of [[36, 41], [60, 42]]) { s.ell(cx, cy, 6, 5, "P", {}); for (let x = cx - 5; x <= cx + 5; x++) s.px(x, cy + 3, "P", 1, false); }
    for (let y = 37; y <= 50; y++) s.px(48, y, "P", 4, false);
    for (const y of [52, 55, 58]) for (let x = 40; x <= 57; x++) if (torso(x, y)) { s.px(x, y, "P", 0, false); s.px(x, y + 1, "P", 3, false); }
    for (let x = 41; x <= 56; x++) s.px(x, 36, "S", 2, false);
    for (const [x, y] of [[42, 40], [54, 40], [42, 52], [54, 52]]) s.px(x, y, "A", 4);
  }),
  item(1007, "pending_plate_greaves", "pants", "Plate greaves: plates down both legs, knee cops, leather straps at the thigh.", (s) => {
    const legs = B.grown(1);
    const inLegs = (x, y) => legs(x, y) && y >= 61 && y <= 86 && x >= 40 && x <= 57;
    plate(s, inLegs, "P", 61, 86);
    for (const cx of [44, 52]) { s.ell(cx, 72, 3.6, 2.6, "P", { tone: 1 }); s.px(cx - 1, 71, "P", 5); }
    for (const y of [64, 80]) for (let x = 40; x <= 57; x++) if (inLegs(x, y)) s.px(x, y, "S", 2, false);
  }),
  item(1008, "pending_leather_boots", "boots", "Leather boots: high boots with a turned cuff and buckles.", (s) => {
    const feet = B.grown(1);
    const inFeet = (x, y) => feet(x, y) && y >= 78 && y <= 91;
    plate(s, inFeet, "P", 78, 91);
    for (let x = 40; x <= 58; x++) { if (inFeet(x, 79)) s.px(x, 79, "P", 4, false); if (inFeet(x, 91)) s.px(x, 91, "S", 1, false); }
    for (const [x, y] of [[44, 84], [52, 84]]) { s.px(x, y, "A", 4); s.px(x + 1, y, "A", 2); }
  }),
  item(1009, "pending_gauntlet_front", "glove_front", "Gauntlet, front hand: plate fingers over the fist and a flared cuff.", (s) => {
    const arm = B.grown(1);
    plate(s, (x, y) => arm(x, y) && y >= 56 && x >= 56, "P", 56, 67);
    for (const y of [61, 63, 65]) s.line(59, y, 64, y, "P", 1, false);
    for (let x = 56; x <= 66; x++) if (arm(x, 57)) s.px(x, 57, "A", 3, false);
  }),
  item(1010, "pending_gauntlet_back", "glove_back", "Gauntlet, back hand: plate fingers and a flared cuff.", (s) => {
    const arm = B.grown(1);
    plate(s, (x, y) => arm(x, y) && y >= 56 && x <= 40, "P", 56, 67);
    for (const y of [61, 63, 65]) s.line(32, y, 37, y, "P", 1, false);
    for (let x = 30; x <= 40; x++) if (arm(x, 57)) s.px(x, 57, "A", 3, false);
  }),
];
// Icons (32x32) of the same items: one shape each, the material colors P as on the body.
const icon = (id, name, comment, build) => ({ id, name, size: 32, layer: "icon", comment: [comment], build: (s) => { s.curve([0.6, 0.3, -0.06]); build(s); } });
models.push(
  icon(1100, "pending_icon_longsword", "Longsword icon.", (s) => { blade(s, 12, 20, 28, 4, 3.4); hilt(s, 10, 22, 5, 27, [8, 17, 15, 24]); }),
  icon(1101, "pending_icon_dagger", "Dagger icon.", (s) => { blade(s, 13, 19, 26, 6, 3.4, -2); hilt(s, 11, 21, 6, 26, [9, 16, 16, 23]); }),
  icon(1102, "pending_icon_greatsword", "Greatsword icon.", (s) => { blade(s, 12, 20, 29, 3, 4.6); hilt(s, 10, 22, 3, 29, [6, 15, 17, 26]); }),
  icon(1103, "pending_icon_round_shield", "Round shield icon.", (s) => {
    s.ell(16, 16, 13, 13, "S", {}); s.ell(16, 16, 11.4, 11.4, "P", {});
    for (const x of [11, 16, 21]) s.line(x, 6, x, 26, "P", 1, false);
    s.ell(16, 16, 3.4, 3.4, "A", {}); s.px(15, 15, "A", 5);
  }),
  icon(1104, "pending_icon_spellbook", "Spellbook icon.", (s) => {
    s.poly([[5, 9], [24, 5], [27, 25], [8, 29]], "P", T.facet(16, 17, 10, 12, 0));
    s.poly([[24, 5], [28, 7], [30, 26], [27, 25]], "S", 4);
    for (const [x, y] of [[6, 10], [23, 6], [26, 24], [9, 28]]) s.px(x, y, "A", 4);
    s.ell(16, 17, 3, 3, "E", {}); s.px(15, 16, "E", 5);
  }),
  icon(1105, "pending_icon_great_helm", "Great helm icon.", (s) => {
    s.mass([["e", 16, 15, 11, 12], ["e", 16, 22, 10, 7]], "P", { k: 2, relief: 1.6 });
    for (let x = 6; x <= 26; x++) { s.px(x, 15, "O", 0, false); s.px(x, 16, "O", 0, false); }
    s.line(16, 3, 16, 14, "P", 4, false);
    for (const [x, y] of [[11, 21], [13, 23], [11, 25]]) s.px(x, y, "O", 0, false);
  }),
  icon(1106, "pending_icon_plate_cuirass", "Plate cuirass icon.", (s) => {
    s.poly([[9, 5], [23, 5], [27, 10], [25, 27], [7, 27], [5, 10]], "P", T.facet(16, 15, 11, 11, 0));
    for (const [cx, cy] of [[5, 9], [27, 9]]) s.ell(cx, cy, 4, 3.4, "P", {});
    s.ell(16, 5, 4, 2, "O", {});
    s.line(16, 7, 16, 18, "P", 4, false);
    for (const y of [20, 23]) s.line(8, y, 24, y, "P", 0, false);
  }),
  icon(1107, "pending_icon_plate_greaves", "Plate greaves icon.", (s) => {
    for (const x of [10, 22]) { s.cap([[x, 4, 4], [x, 26, 3.2]], "P", {}); s.ell(x, 14, 3.6, 2.6, "P", { tone: 1 }); s.line(x - 3, 7, x + 3, 7, "S", 2); }
  }),
  icon(1108, "pending_icon_leather_boots", "Leather boots icon: two boots side by side, toes right.", (s) => {
    for (const [x, t] of [[15, -1], [4, 0]]) {
      s.poly([[x, 5], [x + 8, 5], [x + 8, 21], [x + 13, 23], [x + 14, 28], [x, 28]], "P", (px, py) => Math.max(1, (py < 8 ? 4 : px < x + 3 ? 3 : 2) + t));
      s.line(x, 8, x + 8, 8, "P", Math.max(0, 1 + t), false);
      s.line(x, 28, x + 14, 28, "S", 1, false);
      s.px(x + 5, 17, "A", 4 + t); s.px(x + 6, 17, "A", 2 + t);
    }
  }),
  icon(1109, "pending_icon_gauntlet", "Gauntlet icon: a plated hand, fingers spread, a flared cuff.", (s) => {
    s.poly([[6, 20], [14, 18], [16, 28], [5, 29]], "P", T.facet(10, 24, 5, 5, 0));
    s.mass([["e", 18, 16, 6, 5.5]], "P", { k: 1.4 });
    for (const [x0, y0, x1, y1] of [[14, 11, 12, 3], [17, 11, 17, 2], [20, 11, 22, 3], [23, 13, 27, 7]]) s.cap([[x0, y0, 1.6], [x1, y1, 1.2]], "P", {});
    s.cap([[13, 17, 1.6], [8, 13, 1.2]], "P", { tone: -1 });
    for (const y of [5, 8]) s.line(12, y, 22, y, "P", 1, false);
    s.line(5, 20, 15, 18, "A", 3);
  }));
module.exports = models;
