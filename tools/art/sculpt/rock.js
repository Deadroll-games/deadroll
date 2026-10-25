// A faceted boulder: an irregular polygon around (cx, cy) with stepped planar light from the top
// left, so stone reads as chipped faces, not clay. seed varies the outline; tone shifts the colors.
module.exports = (s, cx, cy, w, h, slot, o = {}) => {
  const n = o.sides || 8, seed = o.seed || 1, tone = o.tone || 0;
  const pts = [];
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + 0.3;
    const j = 0.82 + (((seed * 9301 + i * 49297) % 233280) / 233280) * 0.3;
    pts.push([cx + Math.cos(a) * w * j, cy + Math.sin(a) * h * j]);
  }
  s.poly(pts, slot, (x, y) => {
    const v = -(x - cx) / w - (y - cy) / h;
    const c = v > 0.55 ? 4 : v > 0.1 ? 3 : v > -0.45 ? 2 : 1;
    return Math.max(1, Math.min(4, c + tone));
  }, o);
  return pts;
};
