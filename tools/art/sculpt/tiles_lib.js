// Map tile generators, 64x64, item slots (P S A E U O), no outline: tiles meet edge to edge.
// A style object gives each biome its look (see caves.js, forest.js, crypt.js). Deterministic:
// every random choice comes from `rnd(seed)`.

const rnd = (seed) => {
  let x = (seed * 2654435761) >>> 0 || 1;
  return () => { x ^= x << 13; x >>>= 0; x ^= x >>> 17; x ^= x << 5; x >>>= 0; return x / 4294967296; };
};

// Stepped planar light from the top left, as in rock.js: faces read chipped, not rounded.
const facet = (cx, cy, w, h, tone) => (x, y) => {
  const v = -(x - cx) / w - (y - cy) / h;
  const c = v > 0.55 ? 4 : v > 0.1 ? 3 : v > -0.45 ? 2 : 1;
  return Math.max(1, Math.min(4, c + tone));
};

// Irregular stones packed in rows inside [x0, x1) x [y0, y1): each a jittered quad with facet light,
// grout (O) between. rows: stone row heights; w: [min, max] stone widths.
function stones(s, x0, y0, x1, y1, r, o = {}) {
  const slot = o.slot || "P", rowH = o.rowH || [9, 12], wMin = o.w?.[0] || 10, wMax = o.w?.[1] || 18;
  for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) s.px(x, y, "O", 0, false);
  let y = y0, row = 0;
  while (y < y1 - 3) {
    const h = Math.min(y1 - y, Math.round(rowH[0] + r() * (rowH[1] - rowH[0])));
    let x = x0 + (row % 2 ? -Math.round(wMin / 2) : 0);
    while (x < x1) {
      const w = Math.round(wMin + r() * (wMax - wMin));
      const ax = Math.max(x0, x) + 1, bx = Math.min(x1, x + w) - 1, ay = y + 1, by = Math.min(y1, y + h) - 1;
      if (bx - ax >= 3 && by - ay >= 3) {
        const j = () => Math.round((r() - 0.5) * 2);
        const pts = [[ax + j(), ay + j()], [bx + j(), ay + j()], [bx + j(), by + j()], [ax + j(), by + j()]]
          .map(([px, py]) => [Math.max(x0, Math.min(x1, px)), Math.max(y0, Math.min(y1, py))]);
        const tone = (o.tone || 0) + (r() < 0.25 ? -1 : 0);
        const f = facet((ax + bx) / 2, (ay + by) / 2, (bx - ax) / 2 + 1, (by - ay) / 2 + 1, tone);
        // A flat (floor) stone: only the top-left rim is lit, the rest one even tone.
        s.poly(pts, slot, o.flat ? () => (tone < (o.tone || 0) ? 2 : 3) : f);
        // A chip or crack on some stones.
        if (r() < (o.cracks ?? 0.35)) {
          const cx = ax + 1 + Math.floor(r() * (bx - ax - 2)), cy = ay + 1 + Math.floor(r() * (by - ay - 2));
          s.line(cx, cy, cx + (r() < 0.5 ? 2 : -2), cy + 2, "O", 0, false);
        }
      }
      x += w;
    }
    y += h; row++;
  }
}

// Scatter detail pixels (moss, pebbles, bones) in a region, only over a given slot.
function scatter(s, x0, y0, x1, y1, r, n, slot, c, onlyOver) {
  for (let i = 0; i < n; i++) {
    const x = x0 + Math.floor(r() * (x1 - x0)), y = y0 + Math.floor(r() * (y1 - y0));
    const p = s.at(x, y);
    if (onlyOver && (!p || p.slot !== onlyOver)) continue;
    s.px(x, y, slot, typeof c === "function" ? c() : c, false);
  }
}

// A wall cell for the neighbor mask (bit 0 N, 1 E, 2 S, 3 W are walls). The top of the wall fills the
// cell; when the south is open, the lower part is the front face of the wall (3/4 view).
function wall(style, mask, seed) {
  return (s) => {
    const r = rnd(seed), N = mask & 1, E = mask & 2, S = mask & 4, W = mask & 8;
    const faceTop = S ? 64 : style.capH;
    // Cap: the top of the wall (S slot), rough, with lit and shadowed rims where it ends.
    for (let y = 0; y < faceTop; y++) for (let x = 0; x < 64; x++) s.px(x, y, "O", 0, false);
    style.cap(s, r, 0, 0, 64, faceTop);
    const P = style.slot || "P";
    if (!N) for (let x = 0; x < 64; x++) { s.px(x, 0, P, 4, false); s.px(x, 1, P, 3, false); }
    if (!W) for (let y = 0; y < faceTop; y++) { s.px(0, y, P, 3, false); s.px(1, y, P, 2, false); }
    if (!E) for (let y = 0; y < faceTop; y++) { s.px(63, y, P, 0, false); s.px(62, y, P, 1, false); }
    if (!S) {
      for (let x = 0; x < 64; x++) { s.px(x, faceTop - 1, P, 4, false); }
      // Front face: masonry, darker toward the floor, with a shadow band at the foot.
      style.face(s, r, 0, faceTop, 64, 64, mask);
      if (!W) for (let y = faceTop; y < 64; y++) s.px(0, y, "O", 0, false);
      if (!E) for (let y = faceTop; y < 64; y++) { s.px(63, y, "O", 0, false); }
    }
  };
}

// A floor of worn flagstones: dirt (S) in the joints, stones (P) lit from the top left, extras drawn by
// style.floorDetail. Stones keep inside the cell so cells tile without a seam.
function floor(style, seed, variant) {
  return (s) => {
    const r = rnd(seed);
    for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) s.px(x, y, "S", 1 + ((x * 7 + y * 3) % 5 === 0 ? 1 : 0), false);
    stones(s, 1, 1, 63, 63, r, { rowH: style.floorRowH || [14, 22], w: style.floorW || [16, 28], tone: style.floorTone ?? 0, cracks: 0.5, flat: true });
    // Flat light: a lit rim on the top and left edges of each slab, a shadow on the bottom and right.
    const edge = [];
    for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) {
      const p = s.at(x, y);
      if (!p || p.slot !== "P") continue;
      const out = (dx, dy) => { const q = s.at(x + dx, y + dy); return !q || q.slot !== "P" || q.sid !== p.sid; };
      if (out(0, -1) || out(-1, 0)) edge.push([p, Math.min(4, p.c + 1)]);
      else if (out(0, 1) || out(1, 0)) edge.push([p, Math.max(1, p.c - 1)]);
    }
    for (const [p, c] of edge) p.c = c;
    // Joints are dirt, not grout.
    for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) { const p = s.at(x, y); if (p && p.slot === "O") { p.slot = "S"; p.c = 1; } }
    if (style.floorDetail) style.floorDetail(s, r, variant);
  };
}

// Wooden planks across the cell: staggered ends, gaps (O), grain, nails (A), rot holes.
function planks(seed, variant) {
  return (s) => {
    const r = rnd(seed);
    for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) s.px(x, y, "O", 0, false);
    for (let row = 0; row < 8; row++) {
      const y0 = row * 8 + 1, y1 = y0 + 6;
      let x = row % 2 ? -Math.floor(r() * 20) : 0;
      while (x < 64) {
        const w = 22 + Math.floor(r() * 18), a = Math.max(0, x), b = Math.min(64, x + w - 1);
        const tone = r() < 0.3 ? -1 : 0;
        for (let y = y0; y < y1; y++) for (let px = a; px < b; px++) s.px(px, y, "P", Math.max(1, (y === y0 ? 4 : y === y1 - 1 ? 2 : 3) + tone), false);
        for (let k = 0; k < 2; k++) { const gy = y0 + 2 + Math.floor(r() * 3), gx = a + Math.floor(r() * Math.max(1, b - a - 8)); s.line(gx, gy, gx + 5 + Math.floor(r() * 6), gy, "P", 2 + tone, false); }
        if (b - a > 6) { s.px(a + 2, y0 + 1, "A", 4, false); s.px(a + 2, y1 - 2, "A", 2, false); }
        x += w;
      }
    }
    // Rot: dark broken holes, more on later variants.
    for (let i = 0; i < variant * 2; i++) {
      const cx = 6 + Math.floor(r() * 52), cy = 6 + Math.floor(r() * 52);
      s.ell(cx, cy, 2 + r() * 2, 1.5 + r(), "O", {});
      s.px(cx - 2, cy - 1, "P", 4, false);
    }
  };
}

// Bare ground (S) with stones (P), cracks, small bones (A) and a dark puddle by variant.
function ground(seed, variant) {
  return (s) => {
    const r = rnd(seed);
    for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) {
      const n = ((x * 13 + y * 7 + (x >> 2) * (y >> 3)) % 11);
      s.px(x, y, "S", n === 0 ? 3 : n < 3 ? 1 : 2, false);
    }
    for (let i = 0; i < 9; i++) {
      const cx = 3 + r() * 58, cy = 3 + r() * 58, w = 1.5 + r() * 3, h = 1 + r() * 2;
      s.poly([[cx - w, cy], [cx - w * 0.3, cy - h], [cx + w, cy - h * 0.5], [cx + w * 0.6, cy + h], [cx - w * 0.6, cy + h]], "P", (x, y) => (y < cy ? 3 : 2));
    }
    if (variant === 1) { s.line(10, 20, 22, 26, "A", 4); s.line(10, 21, 22, 27, "A", 2); s.px(9, 20, "A", 4); s.px(23, 26, "A", 4); s.ell(44, 44, 3, 2.4, "A", {}); s.px(43, 44, "O", 0, false); s.px(45, 44, "O", 0, false); }
    if (variant === 2) { s.ell(36, 34, 10, 6, "O", {}); s.ell(36, 34, 9, 5, "E", { tone: -1 }); s.line(30, 32, 36, 31, "E", 4); }
    for (let i = 0; i < 2; i++) { let x = 4 + Math.floor(r() * 56), y = 4 + Math.floor(r() * 56); for (let k = 0; k < 8; k++) { s.px(x, y, "S", 0, false); x += r() < 0.6 ? 1 : 0; y += r() < 0.6 ? 1 : 0; } }
  };
}

// Water over a visible bottom (shallow) or without one (deep): wave crests (A) repeat every 32 px
// so cells tile without a seam.
function water(seed, deep) {
  return (s) => {
    const r = rnd(seed);
    for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) s.px(x, y, "P", deep ? 1 : 2, false);
    if (!deep) for (let i = 0; i < 6; i++) {
      const cx = 6 + r() * 52, cy = 6 + r() * 52, w = 4 + r() * 6, h = 2 + r() * 3;
      s.ell(cx, cy, w, h, "P", { tone: -1 });
      s.px(Math.round(cx - w / 2), Math.round(cy - h / 2), "P", 3, false);
    }
    else for (let i = 0; i < 5; i++) s.ell(4 + r() * 56, 4 + r() * 56, 6 + r() * 6, 3, "P", { tone: -2 });
    for (let k = 0; k < 4; k++) for (let j = 0; j < 4; j++) {
      const x0 = j * 16 + (k % 2) * 8, y0 = k * 16 + 6;
      for (let t = 0; t < 7; t++) s.px((x0 + t) % 64, y0 + (t > 1 && t < 5 ? -1 : 0), "A", t === 3 ? 4 : 3, false);
    }
  };
}

// A void: solid unexcavated rock, the same dark top as a wall cap, all over the cell.
function solid(style, seed) {
  return (s) => { const r = rnd(seed); style.cap(s, r, 0, 0, 64, 64); };
}

// A chasm: a black pit; its far (top) rim of rock with a lit edge, the walls dropping into darkness.
function chasm(seed) {
  return (s) => {
    const r = rnd(seed);
    for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) s.px(x, y, "O", 0, false);
    for (let y = 0; y < 22; y++) for (let x = 0; x < 64; x++) {
      const d = y - 10 - Math.round(Math.sin(x / 5 + seed) * 2);
      if (d < 0) s.px(x, y, "P", 2, false);
      else if (d < 9) s.px(x, y, "P", d < 2 ? 3 : d < 5 ? 1 : 0, false);
    }
    for (let i = 0; i < 6; i++) { const x = Math.floor(r() * 62); s.line(x, 12, x + 1, 20, "P", 0, false); }
    for (let i = 0; i < 4; i++) s.px(Math.floor(r() * 64), 30 + Math.floor(r() * 30), "P", 0, false);
  };
}

// Stairs going down: steps getting darker toward the bottom between two side walls.
function stairs(seed) {
  return (s) => {
    for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) s.px(x, y, "O", 0, false);
    for (let k = 0; k < 7; k++) {
      const y0 = 4 + k * 8, inset = 6 + k * 2, tone = Math.max(1, 3 - Math.floor(k / 2));
      for (let x = inset; x < 64 - inset; x++) { s.px(x, y0, "P", Math.min(4, tone + 1), false); for (let y = y0 + 1; y < y0 + 5; y++) s.px(x, y, "P", tone, false); for (let y = y0 + 5; y < y0 + 8; y++) s.px(x, y, "P", Math.max(0, tone - 2), false); }
    }
    for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) {
      const k = Math.floor((y - 4) / 8), inset = 6 + Math.max(0, k) * 2;
      if (x < inset - 1 || x >= 64 - inset + 1) s.px(x, y, "S", x < 32 ? 2 : 1, false);
    }
  };
}

// Rubble: a floor (the given builder) under heaps of faceted broken stones.
function rubble(base, seed) {
  return (s) => {
    base(s);
    const r = rnd(seed);
    for (let i = 0; i < 14; i++) {
      const cx = 6 + r() * 52, cy = 8 + r() * 50, w = 2.5 + r() * 5, h = 2 + r() * 3.5;
      const pts = []; for (let k = 0; k < 6; k++) { const a = (k / 6) * Math.PI * 2 + r(); pts.push([cx + Math.cos(a) * w * (0.7 + r() * 0.4), cy + Math.sin(a) * h * (0.7 + r() * 0.4)]); }
      s.ell(cx + 1, cy + h * 0.8, w, 1.4, "O", {});
      s.poly(pts, "P", facet(cx, cy, w, h, 0));
    }
  };
}

module.exports = { rnd, facet, stones, scatter, wall, floor, planks, ground, water, solid, chasm, stairs, rubble };
