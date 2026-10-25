// Icons 32x32 (layer icon, item slots, outlined): resources from monsters (600..), ores, ingots and
// stones (650..), the generic recipe / skill blank / reagent (680..), consumables (690..). One shape
// per kind of thing: the colors come from the render request (tools/art ICON_LOOKS, the app's icon
// table), so one ore sprite serves iron, copper and silver.
const T = require("./tiles_lib");
const icon = (id, name, comment, build) => ({ id, name, size: 32, layer: "icon", comment: [comment], build: (s) => { s.curve([0.6, 0.3, -0.06]); build(s); } });

const models = [
  // Pelt: a rolled fur hide tied with cord; the leather side shows in a spiral at the end.
  icon(600, "icon_pelt", "Pelt: a rolled fur hide tied with cord, the leather side spiralling at the end.", (s) => {
    s.cap([[9, 20, 7], [24, 11, 7]], "P", {});
    for (let i = 0; i < 12; i++) { const t = i / 11, x = 9 + t * 15, y = 14 + (1 - t) * 9; s.poly([[x - 1.5, y], [x - 0.5, y - 3], [x + 1.5, y]], "P", 4); }
    s.ell(8, 21, 5.6, 6.2, "S", {});
    for (let t = 0; t < Math.PI * 4; t += 0.2) { const r = 0.5 + t * 0.35; s.px(Math.round(8 + Math.cos(t) * r * 0.9), Math.round(21 + Math.sin(t) * r), "S", 1, false); }
    for (const [x, y] of [[15, 12], [17, 21]]) s.line(x, y, x + 4, y - 4, "A", 3);
    for (const [x, y] of [[13, 23], [20, 18], [24, 13]]) s.px(x, y, "P", 1, false);
  }),
  // Fang: a long curved tooth, the root dark.
  icon(601, "icon_fang", "Fangs: two long curved teeth.", (s) => {
    for (const [x, y, t] of [[11, 6, -1], [18, 4, 0]]) {
      s.cap([[x, y, 3.4], [x + 2, y + 10, 2.8], [x + 6, y + 18, 1.6], [x + 10, y + 22, 0.6]], "P", { tone: t });
      s.ell(x, y + 1, 3.4, 2, "S", { tone: t });
    }
    s.line(20, 8, 24, 20, "P", 5, false);
  }),
  // Sinew: a sheaf of long thin dried fibres, curved, tied in the middle.
  icon(602, "icon_sinew", "Sinews: a sheaf of long dried fibres, tied in the middle.", (s) => {
    for (let k = 0; k < 6; k++) {
      const pts = [];
      for (let t = 0; t <= 1; t += 0.05) pts.push([5 + t * 22 - k * 0.5, 6 + t * 16 + (k - 2.5) * 1.5 + Math.sin(t * Math.PI) * (k - 2.5) * 0.8]);
      for (const [x, y] of pts) { s.px(Math.round(x), Math.round(y), "P", k % 2 ? 2 : 4, false); s.px(Math.round(x), Math.round(y) + 1, "P", k % 2 ? 1 : 3, false); }
    }
    s.poly([[13, 11], [18, 13], [17, 22], [12, 20]], "S", (x) => (x < 15 ? 3 : 1));
    s.line(13, 12, 17, 14, "S", 4, false);
  }),
  // Claw: a hooked claw with a bit of the toe.
  icon(603, "icon_claw", "Claws: a hooked claw on a piece of the toe.", (s) => {
    s.ell(9, 22, 5, 5, "S", {});
    s.cap([[11, 20, 4], [16, 12, 3], [22, 8, 2], [27, 10, 1], [28, 14, 0.5]], "P", {});
    s.line(14, 14, 24, 8, "P", 5, false);
  }),
  // Fat: a pale lump wrapped in a cloth tied with string.
  icon(604, "icon_fat", "Fat: a pale lump in a cloth tied with string.", (s) => {
    s.mass([["e", 16, 19, 11, 9], ["e", 13, 14, 6, 5]], "P", { k: 2, relief: 1.6 });
    s.poly([[6, 20], [26, 20], [25, 27], [7, 27]], "S", (x, y) => (y < 22 ? 3 : 2));
    s.line(6, 21, 26, 21, "A", 3); s.px(16, 21, "A", 5); s.line(16, 21, 18, 25, "A", 2);
  }),
  // Wood: a cut log, the end grain rings showing.
  icon(605, "icon_log", "Wood: a cut log with the end grain showing.", (s) => {
    s.cap([[10, 20, 6.5], [24, 12, 6.5]], "P", {});
    s.ell(9, 21, 5.6, 6, "S", {}); s.ell(9, 21, 3.4, 3.8, "S", { tone: -1 }); s.ell(9, 21, 1.2, 1.4, "S", { tone: 1 });
    for (const [x, y] of [[16, 12], [20, 18], [24, 10]]) s.line(x, y, x + 4, y - 2, "P", 1, false);
  }),
  // Bark: a curled slab of rough bark, the pale inner side.
  icon(606, "icon_bark", "Bark: a curled slab of rough bark, the pale inner side.", (s) => {
    s.poly([[6, 8], [24, 4], [27, 22], [10, 28]], "P", T.facet(16, 16, 10, 12, 0));
    s.poly([[22, 5], [26, 6], [28, 22], [26, 23]], "S", 3);
    for (const x of [10, 14, 18, 22]) s.line(x, 7 + (x - 10) / 4, x - 2, 26 - (x - 10) / 3, "P", 0, false);
  }),
  // Resin: an amber drop with a light inside (E glows).
  icon(607, "icon_resin", "Resin: a drop of amber with a light inside.", (s) => {
    s.poly([[16, 4], [22, 14], [24, 20], [21, 26], [11, 26], [8, 20], [10, 14]], "E", (x, y) => (y < 12 ? 4 : x < 14 ? 3 : 2));
    s.px(13, 15, "E", 5); s.px(14, 16, "E", 5); s.px(18, 22, "E", 1); s.px(19, 21, "E", 1);
  }),
  // Heartwood: a split log showing a glowing core (E).
  icon(608, "icon_heartwood", "Heartwood: a split log with a glowing core.", (s) => {
    s.cap([[8, 22, 6], [24, 10, 6]], "P", { tone: -1 });
    s.cap([[10, 20, 2.4], [22, 12, 2.4]], "E", {});
    s.line(11, 19, 21, 12, "E", 5, false);
    s.ell(7, 23, 5, 5.4, "S", {}); s.ell(7, 23, 2.2, 2.4, "E", { tone: 1 });
  }),
  // Pollen: a small cloth pouch spilling glowing dust (E).
  icon(609, "icon_pollen", "Magic pollen: a cloth pouch spilling glowing dust.", (s) => {
    s.mass([["e", 14, 19, 8, 8]], "S", { k: 1.4 });
    s.poly([[11, 10], [17, 10], [16, 13], [12, 13]], "S", 2);
    s.line(10, 13, 18, 13, "A", 3);
    for (const [x, y, c] of [[22, 16, 5], [24, 12, 4], [26, 18, 3], [21, 22, 4], [27, 9, 3], [24, 24, 2]]) s.px(x, y, "E", c);
  }),
  // Elemental stone: a cut crystal glowing in its element (E).
  icon(610, "icon_stone", "Elemental stone: a cut crystal in its element's color.", (s) => {
    s.poly([[16, 3], [25, 12], [22, 28], [10, 28], [7, 12]], "E", (x, y) => (x < 16 && y < 16 ? 4 : x < 16 ? 3 : y < 14 ? 3 : 2));
    s.line(16, 3, 16, 28, "E", 1, false); s.line(7, 12, 25, 12, "E", 5, false);
    s.px(12, 8, "E", 5);
  }),
  // Wings: a torn insect wing with veins (E for the membrane).
  icon(611, "icon_wing", "Fairy wings: a torn insect wing with dark veins.", (s) => {
    s.poly([[6, 26], [10, 10], [18, 3], [26, 6], [24, 14], [16, 22]], "E", (x, y) => (y < 12 ? 4 : 3));
    for (const [a, b] of [[[6, 26], [18, 4]], [[6, 26], [25, 8]], [[6, 26], [20, 18]]]) s.line(a[0], a[1], b[0], b[1], "O", 0, false);
    s.erase(20, 10); s.erase(21, 11);
  }),
  // Scrap metal: a bent steel plate with rust (A), a broken gear, a bent nail.
  icon(612, "icon_scrap", "Scrap metal: a bent plate with rust, a broken gear, a bent nail.", (s) => {
    s.poly([[4, 20], [14, 12], [20, 16], [12, 26]], "P", T.facet(12, 19, 7, 6, 0));
    s.line(14, 12, 12, 26, "P", 1, false);
    for (let t = 0; t < Math.PI * 1.6; t += 0.15) {
      const x = 21 + Math.cos(t) * 6, y = 12 + Math.sin(t) * 6;
      s.ell(x, y, 1.6, 1.6, "P", { tone: -1 });
    }
    s.ell(21, 12, 3.2, 3.2, "P", {}); s.ell(21, 12, 1.2, 1.2, "O", {});
    s.line(18, 24, 27, 27, "P", 4); s.line(27, 27, 28, 23, "P", 3);
    for (const [x, y] of [[9, 19], [12, 16], [7, 22], [23, 8], [17, 14]]) s.px(x, y, "A", 2 + ((x + y) % 2));
  }),
  // Rags: a bundle of cloth and leather scraps.
  icon(613, "icon_rags", "Cloth and leather scraps: a bundle of rags.", (s) => {
    s.poly([[5, 14], [16, 9], [26, 14], [24, 26], [8, 26]], "S", T.facet(16, 18, 10, 9, 0));
    s.poly([[8, 12], [20, 6], [22, 12], [12, 18]], "P", T.facet(15, 12, 7, 5, 0));
    s.line(8, 22, 24, 18, "S", 1); s.line(12, 26, 14, 20, "S", 1);
  }),
  // Trinkets: a string of teeth with a crooked coin.
  icon(614, "icon_trinket", "Goblin trinkets: a string of teeth and a crooked coin.", (s) => {
    for (let t = 0; t <= 1; t += 0.05) { const x = 4 + t * 24, y = 8 + Math.sin(t * Math.PI) * 12; s.px(Math.round(x), Math.round(y), "S", 2); }
    for (const t of [0.15, 0.32, 0.68, 0.85]) { const x = 4 + t * 24, y = 8 + Math.sin(t * Math.PI) * 12; s.cap([[x, y + 1, 1.4], [x, y + 5, 0.5]], "P", {}); }
    s.ell(16, 22, 4.4, 4, "A", {}); s.px(15, 21, "A", 5); s.line(14, 22, 18, 23, "A", 1);
  }),
  // Bone: a big thick bone with knobbed ends.
  icon(615, "icon_bone", "Troll bone: a thick bone with knobbed ends.", (s) => {
    s.cap([[8, 24, 3], [24, 8, 3]], "P", {});
    for (const [x, y] of [[6, 22], [9, 26], [22, 6], [26, 10]]) s.ell(x, y, 3.2, 3.2, "P", {});
    s.line(10, 21, 21, 10, "P", 5, false);
  }),
  // Blood: a stoppered vial of dark thick liquid (E).
  icon(616, "icon_vial", "Troll blood: a stoppered vial of dark liquid.", (s) => {
    s.poly([[12, 8], [20, 8], [20, 12], [24, 18], [23, 27], [9, 27], [8, 18], [12, 12]], "A", 1);
    s.poly([[10, 17], [22, 17], [22, 26], [10, 26]], "E", (x, y) => (x < 13 ? 3 : 2));
    s.poly([[13, 4], [19, 4], [19, 8], [13, 8]], "S", 3);
    s.px(11, 14, "A", 5); s.line(10, 19, 10, 24, "E", 4);
  }),
  // Ore: a pile of rock with veins of metal (E colors the metal).
  icon(650, "icon_ore", "Ore: a pile of rock with veins of metal (E is the metal).", (s) => {
    s.poly([[4, 26], [6, 18], [12, 12], [20, 10], [26, 16], [28, 26]], "P", T.facet(16, 18, 12, 8, 0));
    s.poly([[8, 26], [10, 20], [16, 18], [18, 26]], "P", T.facet(13, 22, 5, 4, -1));
    for (const [x, y] of [[13, 15], [20, 14], [22, 21], [11, 22]]) { s.px(x, y, "E", 5); s.px(x + 1, y, "E", 3); s.px(x, y + 1, "E", 2); }
  }),
  // Ingot: a cast bar with sloped sides (P colors the metal), a stamp.
  icon(651, "icon_ingot", "Ingot: a cast bar (P is the metal), a stamp on top.", (s) => {
    s.poly([[5, 20], [22, 20], [27, 26], [9, 26]], "P", (x, y) => (y < 22 ? 2 : 1));
    s.poly([[8, 14], [24, 14], [22, 20], [5, 20]], "P", (x, y) => (y < 16 ? 4 : 3));
    s.poly([[24, 14], [28, 20], [27, 26], [22, 20]], "P", 1);
    s.ell(15, 17, 2.2, 1.4, "P", { tone: -1 }); s.line(9, 15, 22, 15, "P", 5, false);
  }),
  // Dual stone: a crystal split between two elements (E and U).
  icon(652, "icon_dual_stone", "Dual-elemental stone: a crystal split between two elements (E and A).", (s) => {
    s.poly([[16, 3], [25, 12], [22, 28], [16, 28]], "A", (x, y) => (y < 14 ? 3 : 2));
    s.poly([[16, 3], [16, 28], [10, 28], [7, 12]], "E", (x, y) => (y < 14 ? 4 : 3));
    s.line(7, 12, 25, 12, "E", 5, false); s.px(12, 8, "E", 5);
  }),
  // Recipe: a rolled scroll tied with a cord and a wax seal.
  icon(680, "icon_recipe", "Recipe: a rolled scroll tied with a cord, a wax seal.", (s) => {
    s.cap([[7, 23, 4], [25, 9, 4]], "P", {});
    s.ell(7, 23, 3, 4, "P", { tone: -1 }); s.ell(25, 9, 3, 4, "P", { tone: 1 });
    s.line(14, 21, 18, 13, "S", 2); s.ell(17, 18, 2.6, 2.6, "A", {}); s.px(16, 17, "A", 5);
  }),
  // Skill blank: a dark stone tablet with an empty carved frame (U waits for a mechanic).
  icon(681, "icon_blank", "Skill blank: a stone tablet with an empty carved frame.", (s) => {
    s.poly([[7, 5], [25, 5], [26, 27], [6, 27]], "P", T.facet(16, 16, 10, 11, 0));
    for (let x = 10; x < 23; x++) { s.px(x, 9, "P", 0, false); s.px(x, 23, "P", 4, false); }
    for (let y = 9; y < 24; y++) { s.px(10, y, "P", 0, false); s.px(22, y, "P", 4, false); }
    s.px(16, 16, "U", 3);
  }),
  // Reagent: a small corked flask of powder with a label.
  icon(682, "icon_reagent", "Reagent: a corked flask of powder with a label.", (s) => {
    s.poly([[11, 10], [21, 10], [24, 16], [24, 27], [8, 27], [8, 16]], "A", 1);
    s.poly([[9, 18], [23, 18], [23, 26], [9, 26]], "E", (x, y) => (y < 20 ? 4 : 2));
    s.poly([[13, 5], [19, 5], [19, 10], [13, 10]], "S", 3);
    s.poly([[11, 12], [21, 12], [21, 16], [11, 16]], "S", 4);
    s.px(10, 13, "A", 5);
  }),
  // Potion: a round flask with a long neck, the liquid (E) glowing.
  icon(690, "icon_potion", "Potion: a round flask, the liquid glowing (E).", (s) => {
    s.ell(16, 20, 9, 8, "A", { tone: -2 });
    s.ell(16, 21, 8, 6.4, "E", {});
    s.poly([[13, 5], [19, 5], [19, 13], [13, 13]], "A", 2);
    s.poly([[12, 2], [20, 2], [20, 5], [12, 5]], "S", 3);
    s.px(11, 17, "E", 5); s.px(12, 16, "E", 5); s.line(10, 22, 12, 26, "A", 4);
  }),
  // Food: a hard bread loaf and a strip of dried meat.
  icon(691, "icon_food", "Food: a hard bread loaf and a strip of dried meat.", (s) => {
    s.mass([["e", 14, 16, 10, 7]], "P", { k: 1.4, relief: 1.6 });
    for (const x of [9, 14, 19]) s.line(x, 12, x + 2, 18, "P", 1, false);
    s.poly([[14, 22], [27, 18], [28, 22], [15, 27]], "S", (x, y) => (y < 22 ? 3 : 2));
    s.line(17, 23, 26, 20, "S", 4, false);
  }),
  // Torch: a wrapped stick with a flame (E).
  icon(692, "icon_torch", "Torch: a stick wrapped in pitch-soaked rag, a flame (E).", (s) => {
    s.cap([[10, 28, 1.6], [18, 12, 1.8]], "P", {});
    s.ell(19, 11, 3.4, 3, "S", {});
    s.poly([[19, 1], [24, 6], [23, 10], [16, 10], [15, 6]], "E", (x, y) => (y < 5 ? 5 : y < 8 ? 4 : 3));
    s.px(19, 5, "E", 5);
  }),
];
module.exports = models;
