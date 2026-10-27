// A feathered wing in one slot, built in tiers of round-shaded feathers so it reads as plumage:
// primaries (long, fanned at the tip, at the back), secondaries (along the arm), coverts (short
// scallops in front). root: shoulder; wrist: the bend; tip: the far end of the arm; back: the
// trailing direction [x, y] (feathers grow that way); len: the secondary feather length.
module.exports = (s, root, wrist, tip, len, slot, tone = 0, o = {}) => {
  const back = o.back || [1, 0.4];
  const bl = Math.hypot(back[0], back[1]), bx = back[0] / bl, by = back[1] / bl;
  const lerp = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
  const feather = (p, dx, dy, l, w, t) => s.cap([[p[0], p[1], w], [p[0] + dx * l, p[1] + dy * l, w * 0.45]], slot, { tone: tone + t });
  // Primaries: fanned from the hand (wrist -> tip), long, rotating from `back` toward the arm line.
  const ax = tip[0] - wrist[0], ay = tip[1] - wrist[1], al = Math.hypot(ax, ay);
  for (let i = 0; i < 6; i++) {
    const t = i / 5, p = lerp(wrist, tip, 0.2 + 0.8 * t);
    const dx = bx * (1 - t * 0.7) + (ax / al) * t * 0.7, dy = by * (1 - t * 0.7) + (ay / al) * t * 0.7;
    const n = Math.hypot(dx, dy);
    feather(p, dx / n, dy / n, len * (1.3 + t * 0.5), 2.6, i % 2 ? -1 : 0);
  }
  // Secondaries along the arm (root -> wrist).
  for (let i = 0; i < 6; i++) {
    const p = lerp(root, wrist, 0.1 + 0.9 * (i / 5));
    feather(p, bx, by, len * (0.9 + 0.2 * (i % 2)), 2.8, i % 2 ? 0 : -1);
  }
  // Coverts: short scallops in front along the whole arm.
  for (let i = 0; i < 10; i++) {
    const t = i / 9, p = t < 0.6 ? lerp(root, wrist, t / 0.6) : lerp(wrist, tip, ((t - 0.6) / 0.4) * 0.5);
    feather(p, bx, by, len * 0.35, 2.2, 1);
  }
  // The leading edge of the arm.
  s.cap([[root[0], root[1], 3], [wrist[0], wrist[1], 2.4], [tip[0], tip[1], 1.4]], slot, { tone: tone + 1 });
};
