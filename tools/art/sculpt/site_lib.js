// Site art bigger than one sprite canvas (buildings, screens): drawn on one large grid, outlined as a
// whole, then cut into 128x128 pieces (no outline of their own) that `art compose` puts back
// together. Never packed: site sprites live in assets/site_sprites with ids 2000+.
const { sculptor } = require("./sculpt");

// Draws a W x H picture and adds a 1 px outline (O) around its silhouette.
function picture(W, H, build) {
  const s = sculptor(W, H, 0);
  build(s);
  const add = [];
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    if (s.g[y * W + x]) continue;
    for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
      const q = s.at(x + dx, y + dy);
      if (q && q.slot !== "O") { add.push(y * W + x); break; }
    }
  }
  for (const i of add) s.g[i] = { slot: "O", c: 0, sid: -1 };
  return s;
}

// Cuts a picture into piece x piece sprites. Returns { models, layout } where layout lines put the
// pieces back at (x0, y0).
function pieces(big, idBase, name, x0 = 0, y0 = 0, piece = 128) {
  const models = [], layout = [];
  let k = 0;
  for (let py = 0; py < big.H; py += piece) for (let px = 0; px < big.W; px += piece) {
    let any = false;
    for (let y = py; y < py + piece && !any; y++) for (let x = px; x < px + piece; x++) if (big.at(x, y)) { any = true; break; }
    if (!any) continue;
    const id = idBase + k, pname = `${name}_${k}`;
    models.push({
      id, name: pname, size: piece, layer: "portrait", flags: "no_outline", comment: k === 0 ? [`${name}: a picture cut into ${piece} px pieces (site art).`] : [],
      build: (s) => { for (let y = 0; y < piece; y++) for (let x = 0; x < piece; x++) { const p = big.at(px + x, py + y); if (p) s.g[y * piece + x] = { ...p }; } },
    });
    layout.push(`put ${pname} ${x0 + px} ${y0 + py}`);
    k++;
  }
  return { models, layout };
}

module.exports = { picture, pieces };
