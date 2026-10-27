// One biome's tile set in a fixed id block (FORMAT.md 1.5): base+0..15 walls by neighbor mask,
// +16..19 stone floor, +20..22 planks, +23..25 ground, +26 door, +27..28 shallow water, +29 deep
// water, +30 stairs, +31..32 rubble, +33 chasm, +34 void. cfg: { base, prefix, style, door, ground? }.
const T = require("./tiles_lib");

module.exports = (cfg) => {
  const { base, prefix, style } = cfg;
  const models = [];
  const add = (off, name, build, comment) =>
    models.push({ id: base + off, name: `${prefix}_${name}`, size: 64, layer: "tile", flags: "no_outline", comment: comment ? [comment] : [], build });
  for (let m = 0; m < 16; m++) add(m, `wall_${m}`, T.wall(style, m, base * 10 + m), m === 0 ? `${cfg.title}: wall shapes by the neighbor mask (bit 0 N, 1 E, 2 S, 3 W walls).` : undefined);
  for (let v = 0; v < 4; v++) add(16 + v, `floor_${v}`, T.floor(style, base * 10 + 20 + v, v), v === 0 ? "Floor, four variants." : undefined);
  for (let v = 0; v < 3; v++) add(20 + v, `wood_${v}`, T.planks(base + 20 + v, v), v === 0 ? "Planks." : undefined);
  const ground = cfg.ground || ((seed, v) => T.ground(seed, v));
  for (let v = 0; v < 3; v++) add(23 + v, `dirt_${v}`, ground(base + 23 + v, v), v === 0 ? "Ground." : undefined);
  add(26, "door", cfg.door, "Door in a wall line.");
  add(27, "water_0", T.water(base + 27, false), "Shallow water.");
  add(28, "water_1", T.water(base + 28, false));
  add(29, "deep", T.water(base + 29, true), "Deep water.");
  add(30, "stairs", T.stairs(base + 30), "Stairs going down.");
  add(31, "rubble_0", T.rubble(T.floor(style, base + 31, 0), base + 1031), "Rubble.");
  add(32, "rubble_1", T.rubble(ground(base + 32, 0), base + 1032));
  add(33, "chasm", T.chasm(base + 33), "Chasm.");
  add(34, "void", T.solid(style, base + 34), "Void.");
  return models;
};
