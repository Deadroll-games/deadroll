// Sculpt a sprite from 3D-ish primitives and print a BITMAP of explicit ramp colors.
// node sculpt.js <model.js>  -> prints the DSL sprite on stdout.
// A model exports { id, name, size, layer, flags, build(s) } where `s` is the sculptor below.
const path = require("path");

// Explicit color letters per slot (FORMAT.md 1.3, body meaning on monster layers).
const LET = {
  K: "012345", H: "abcdef", Y: "ghijkl", D: "mnopqr", R: "stuvwx",
  P: "012345", S: "abcdef", A: "ghijkl", E: "mnopqr", U: "stuvwx",
};
const L = (() => { const v = [-0.55, -0.65, 0.52]; const n = Math.hypot(...v); return v.map((x) => x / n); })();

function sculptor(W, H, margin = 1) {
  // cell: { slot, c (0..5), z (shape order), sid (shape id), lock }
  const g = Array.from({ length: W * H }, () => null);
  let sid = 0;
  const seamIds = new Set(); // shapes whose edges get a dark seam (limbs); none flagged = all
  const at = (x, y) => (x >= 0 && y >= 0 && x < W && y < H ? g[y * W + x] : undefined);
  // Default light curve: grim, most pixels in 1-2.
  let curve = [0.78, 0.5, 0.12]; // thresholds for colors 4, 3, 2 (below -> 1)
  const shadeOf = (i, tone) => {
    let c = i > curve[0] ? 4 : i > curve[1] ? 3 : i > curve[2] ? 2 : 1;
    c += tone || 0;
    return Math.max(1, Math.min(4, c));
  };
  const put = (x, y, slot, c, id) => {
    x = Math.round(x); y = Math.round(y);
    if (x < margin || y < margin || x >= W - margin || y >= H - margin) return;
    const cur = g[y * W + x];
    if (cur && cur.lock) return;
    g[y * W + x] = { slot, c, sid: id };
  };
  // Model transform: x -> dx + ax + (x - ax) * k, same for y; radii * k.
  let T = { k: 1, ax: 0, ay: 0, dx: 0, dy: 0 };
  const tx = (x) => T.dx + T.ax + (x - T.ax) * T.k, ty = (y) => T.dy + T.ay + (y - T.ay) * T.k;
  const s = {
    W, H, g, at,
    // Scale about (ax, ay) and shift by (dx, dy) everything drawn after this call.
    transform(k = 1, ax = 0, ay = 0, dx = 0, dy = 0) { T = { k, ax, ay, dx, dy }; },
    curve(c) { curve = c; },
    // Ellipsoid: center, radii, slot; o.tone shifts colors, o.flat squashes depth (0..1).
    ell(cx, cy, rx, ry, slot, o = {}) {
      const id = ++sid; if (o.seam) seamIds.add(id);
      cx = tx(cx); cy = ty(cy); rx *= T.k; ry *= T.k;
      for (let y = Math.floor(cy - ry); y <= Math.ceil(cy + ry); y++)
        for (let x = Math.floor(cx - rx); x <= Math.ceil(cx + rx); x++) {
          const nx = (x + 0.5 - cx) / rx, ny = (y + 0.5 - cy) / ry, d = nx * nx + ny * ny;
          if (d > 1) continue;
          if (o.clip && !o.clip(x, y)) continue;
          // Ragged edge: tufts. Cut edge pixels in a pattern along the angle around the center.
          if (o.ragged && d > 1 - o.ragged) {
            const a = Math.atan2(ny, nx), k = o.tufts || 14;
            const phase = (((a / (2 * Math.PI)) * k) % 1 + 1) % 1;
            if ((d - (1 - o.ragged)) / o.ragged > 1 - phase) continue;
          }
          const nz = Math.sqrt(1 - d) * (o.flat ?? 1);
          const n = Math.hypot(nx, ny, nz) || 1;
          const i = (nx * L[0] + ny * L[1] + nz * L[2]) / n;
          put(x, y, slot, shadeOf(i, o.tone), id);
        }
      return id;
    },
    // Capsule along a polyline of points [x, y, r]: limbs, necks, tails.
    cap(pts, slot, o = {}) {
      const id = ++sid; if (o.seam) seamIds.add(id);
      pts = pts.map(([x, y, r]) => [tx(x), ty(y), r * T.k]);
      const xs = pts.map((p) => p[0]), ys = pts.map((p) => p[1]), rm = Math.max(...pts.map((p) => p[2]));
      for (let y = Math.floor(Math.min(...ys) - rm); y <= Math.ceil(Math.max(...ys) + rm); y++)
        for (let x = Math.floor(Math.min(...xs) - rm); x <= Math.ceil(Math.max(...xs) + rm); x++) {
          let best = null;
          for (let k = 0; k + 1 < pts.length; k++) {
            const [ax, ay, ar] = pts[k], [bx, by, br] = pts[k + 1];
            const vx = bx - ax, vy = by - ay, len2 = vx * vx + vy * vy || 1;
            let t = ((x + 0.5 - ax) * vx + (y + 0.5 - ay) * vy) / len2;
            t = Math.max(0, Math.min(1, t));
            const px = ax + vx * t, py = ay + vy * t, r = ar + (br - ar) * t;
            const dx = x + 0.5 - px, dy = y + 0.5 - py, dist = Math.hypot(dx, dy);
            if (dist <= r && (!best || dist / r < best.q)) best = { q: dist / r, dx: dx / r, dy: dy / r };
          }
          if (!best) continue;
          if (o.clip && !o.clip(x, y)) continue;
          const d = Math.min(1, best.dx * best.dx + best.dy * best.dy);
          const nz = Math.sqrt(1 - d);
          const i = best.dx * L[0] + best.dy * L[1] + nz * L[2];
          put(x, y, slot, shadeOf(i, o.tone), id);
        }
      return id;
    },
    // One smooth body from several primitives: heights blend by a smooth max, so the light has no
    // seams where parts meet. prims: ["e", cx, cy, rx, ry] or ["c", [[x, y, r], ...]].
    // o.k blend softness (px), o.tone, o.ragged/o.tufts per "e" as in ell (prim[5] = options).
    mass(prims, slot, o = {}) {
      const id = ++sid; if (o.seam) seamIds.add(id);
      const k = o.k ?? 3;
      const P = prims.map((p) => p[0] === "e"
        ? ["e", tx(p[1]), ty(p[2]), p[3] * T.k, p[4] * T.k, p[5] || {}]
        : ["c", p[1].map(([x, y, r]) => [tx(x), ty(y), r * T.k]), p[2] || {}]);
      const height = (x, y) => {
        let sum = 0, any = false;
        for (const p of P) {
          let h = -1;
          if (p[0] === "e") {
            const nx = (x - p[1]) / p[3], ny = (y - p[2]) / p[4], d = nx * nx + ny * ny;
            if (d <= 1) {
              const op = p[5];
              let cut = false;
              if (op.ragged && d > 1 - op.ragged) {
                const a = Math.atan2(ny, nx), kk = op.tufts || 14;
                const phase = (((a / (2 * Math.PI)) * kk) % 1 + 1) % 1;
                cut = (d - (1 - op.ragged)) / op.ragged > 1 - phase;
              }
              if (!cut) h = Math.min(p[3], p[4]) * Math.sqrt(1 - d);
            }
          } else {
            const pts = p[1];
            let best = -1;
            for (let i = 0; i + 1 < pts.length; i++) {
              const [ax, ay, ar] = pts[i], [bx, by, br] = pts[i + 1];
              const vx = bx - ax, vy = by - ay, len2 = vx * vx + vy * vy || 1;
              let t = ((x - ax) * vx + (y - ay) * vy) / len2;
              t = Math.max(0, Math.min(1, t));
              const r = ar + (br - ar) * t, dist = Math.hypot(x - ax - vx * t, y - ay - vy * t);
              if (dist <= r) best = Math.max(best, r * Math.sqrt(1 - (dist / r) ** 2));
            }
            h = best;
          }
          if (h >= 0) { any = true; sum += Math.exp(h / k); }
        }
        return any ? k * Math.log(sum) : -1;
      };
      let x0 = 1e9, y0 = 1e9, x1 = -1e9, y1 = -1e9;
      for (const p of P) {
        const b = p[0] === "e" ? [[p[1], p[2], Math.max(p[3], p[4])]] : p[1];
        for (const [x, y, r] of b) { x0 = Math.min(x0, x - r); y0 = Math.min(y0, y - r); x1 = Math.max(x1, x + r); y1 = Math.max(y1, y + r); }
      }
      for (let y = Math.floor(y0); y <= Math.ceil(y1); y++)
        for (let x = Math.floor(x0); x <= Math.ceil(x1); x++) {
          const cx = x + 0.5, cy = y + 0.5, h = height(cx, cy);
          if (h < 0) continue;
          const hx = (height(cx + 1, cy) < 0 ? 0 : height(cx + 1, cy)) - (height(cx - 1, cy) < 0 ? 0 : height(cx - 1, cy));
          const hy = (height(cx, cy + 1) < 0 ? 0 : height(cx, cy + 1)) - (height(cx, cy - 1) < 0 ? 0 : height(cx, cy - 1));
          const relief = o.relief ?? 1;
          const nx = (-hx / 2) * relief, ny = (-hy / 2) * relief, n = Math.hypot(nx, ny, 1);
          const i = (nx * L[0] + ny * L[1] + 1 * L[2]) / n;
          put(x, y, slot, shadeOf(i, o.tone), id);
        }
      return id;
    },
    // Flat polygon with a fixed color (ears, spikes, blades, rags).
    poly(pts, slot, c, o = {}) {
      const id = ++sid; if (o.seam) seamIds.add(id);
      pts = pts.map(([x, y]) => [tx(x), ty(y)]);
      const xs = pts.map((p) => p[0]), ys = pts.map((p) => p[1]);
      for (let y = Math.floor(Math.min(...ys)); y <= Math.ceil(Math.max(...ys)); y++)
        for (let x = Math.floor(Math.min(...xs)); x <= Math.ceil(Math.max(...xs)); x++) {
          const px = x + 0.5, py = y + 0.5;
          let inside = false;
          for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
            const [xi, yi] = pts[i], [xj, yj] = pts[j];
            if (yi > py !== yj > py && px < ((xj - xi) * (py - yi)) / (yj - yi) + xi) inside = !inside;
          }
          const mx = T.ax + (x - T.dx - T.ax) / T.k, my = T.ay + (y - T.dy - T.ay) / T.k;
          if (inside) put(x, y, slot, typeof c === "function" ? c(Math.round(mx), Math.round(my)) : c, id);
        }
      return id;
    },
    px(x, y, slot, c, lock = true) {
      x = Math.round(tx(x)); y = Math.round(ty(y));
      if (x < margin || y < margin || x >= W - margin || y >= H - margin) return;
      g[y * W + x] = { slot, c, sid: -1, lock };
    },
    // 1 px line (Bresenham) with a fixed color.
    line(x0, y0, x1, y1, slot, c, lock = true) {
      [x0, y0, x1, y1] = [tx(x0), ty(y0), tx(x1), ty(y1)].map(Math.round);
      const saved = T; T = { k: 1, ax: 0, ay: 0, dx: 0, dy: 0 };
      const dx = Math.abs(x1 - x0), dy = -Math.abs(y1 - y0), sx = x0 < x1 ? 1 : -1, sy = y0 < y1 ? 1 : -1;
      let err = dx + dy;
      for (;;) {
        s.px(x0, y0, slot, c, lock);
        if (x0 === x1 && y0 === y1) break;
        const e2 = 2 * err;
        if (e2 >= dy) { err += dy; x0 += sx; }
        if (e2 <= dx) { err += dx; y0 += sy; }
      }
      T = saved;
    },
    // Hand-drawn pixels: rows of BITMAP letters (digits K, a-f H, g-l Y, m-r D, s-x R, O, `.` keep,
    // `_` erase) with the top left at (x, y) in model coordinates, one pixel per letter.
    patch(x, y, rows) {
      const slots = [["K", "012345"], ["H", "abcdef"], ["Y", "ghijkl"], ["D", "mnopqr"], ["R", "stuvwx"]];
      rows.forEach((row, j) => [...row].forEach((ch, i) => {
        if (ch === ".") return;
        if (ch === "_") { s.erase(x + i, y + j); return; }
        if (ch === "O") { s.px(x + i, y + j, "O", 0); return; }
        for (const [slot, letters] of slots) {
          const c = letters.indexOf(ch);
          if (c >= 0) { s.px(x + i, y + j, slot, c); return; }
        }
        throw new Error(`patch: bad letter ${ch}`);
      }));
    },
    // Change the color of existing pixels of a slot by `f(x, y, c) -> c`.
    recolor(f, onlySlot) {
      for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
        const p = g[y * W + x];
        if (!p || p.slot === "O" || (onlySlot && p.slot !== onlySlot)) continue;
        p.c = Math.max(0, Math.min(5, f(x, y, p.c, p)));
      }
    },
    erase(x, y) { x = Math.round(tx(x)); y = Math.round(ty(y)); if (at(x, y) !== undefined) g[y * W + x] = null; },
    // Separate overlapping shapes: a pixel of a farther shape next to a nearer one goes darker.
    seams(minGap = 1) {
      const out = [];
      for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
        const p = g[y * W + x];
        if (!p || p.sid < 0) continue;
        for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
          const q = at(x + dx, y + dy);
          const flagged = seamIds.size === 0 || seamIds.has(q?.sid) || seamIds.has(p.sid);
          if (q && flagged && q.sid > p.sid + minGap - 1 && q.sid > 0 && q.slot === p.slot) { out.push(p); break; }
        }
      }
      for (const p of out) p.c = Math.max(0, p.c - 2);
    },
  };
  return s;
}

function emit(model, s) {
  const { W, H, g } = s;
  let x0 = W, y0 = H, x1 = -1, y1 = -1;
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) if (g[y * W + x]) {
    x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x); y1 = Math.max(y1, y);
  }
  const rows = [];
  for (let y = y0; y <= y1; y++) {
    let r = "";
    for (let x = x0; x <= x1; x++) {
      const p = g[y * W + x];
      r += !p ? "." : p.slot === "O" ? "O" : LET[p.slot][p.c];
    }
    rows.push(r);
  }
  const head = [
    ...(model.comment || []).map((c) => `# ${c}`),
    `sprite ${model.id} ${model.name}`,
    `size ${W} ${H}`,
    `layer ${model.layer || "monster"}`,
    ...(model.flags ? [`flags ${model.flags}`] : []),
    `BITMAP ${x0} ${y0} ${x1 - x0 + 1} ${y1 - y0 + 1}`,
  ];
  return head.concat(rows).join("\n") + "\n";
}

module.exports = { sculptor, emit };

if (require.main === module) {
  const model = require(path.resolve(process.argv[2]));
  const models = Array.isArray(model) ? model : [model];
  const out = [];
  for (const m of models) {
    const s = sculptor(m.size, m.size, /no_outline/.test(m.flags || "") ? 0 : 1);
    m.build(s);
    out.push(emit(m, s));
  }
  process.stdout.write(out.join("\n"));
}
