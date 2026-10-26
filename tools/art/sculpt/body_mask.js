// The shared body silhouette (human_body in assets/sprites/bodies/bodies.dsl, every race has the same
// shape below the neck): inBody(x, y) on the 96 canvas. Worn items are built from it so they fit.
const fs = require("fs");
const path = require("path");

const src = fs.readFileSync(path.join(__dirname, "../../../assets/sprites/bodies/bodies.dsl"), "utf8").split("\n");
const at = src.findIndex((l) => l.startsWith("sprite 10 human_body"));
const bi = src.findIndex((l, i) => i > at && l.startsWith("BITMAP"));
const [, bx, by, bw, bh] = src[bi].split(/\s+/).map(Number);
const rows = src.slice(bi + 1, bi + 1 + bh);
const filled = new Set();
rows.forEach((row, j) => [...row].forEach((ch, i) => { if (ch !== "." && ch !== "_") filled.add((by + j) * 96 + bx + i); }));

const inBody = (x, y) => filled.has(y * 96 + x);
// Grow the body by `d` pixels (armor is thicker than skin).
const grown = (d) => (x, y) => {
  for (let dy = -d; dy <= d; dy++) for (let dx = -d; dx <= d; dx++) if (Math.abs(dx) + Math.abs(dy) <= d && inBody(x + dx, y + dy)) return true;
  return false;
};
// Left and right edge of the body on a row (for shading across the form).
const span = (y) => { let a = -1, b = -1; for (let x = 0; x < 96; x++) if (inBody(x, y)) { if (a < 0) a = x; b = x; } return [a, b]; };
module.exports = { inBody, grown, span };
