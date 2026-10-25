// A bat-like wing: arm from the shoulder to the wrist, finger bones fanning from the wrist, the
// membrane scalloped between fingers and sagging toward the body, lit bands along the fingers.
// bone/membrane slots are parameters (bat: K bones on H membrane; gargoyle: both stone).
module.exports = (s, shoulder, wrist, tips, body, tone, o = {}) => {
  const bone = o.bone || "K", skin = o.skin || "H";
  for (let i = 0; i < tips.length; i++) {
    const a = i === 0 ? wrist : tips[i - 1], b = tips[i];
    const sag = [(a[0] + b[0]) / 2 + (body[0] - (a[0] + b[0]) / 2) * 0.25, (a[1] + b[1]) / 2 + (body[1] - (a[1] + b[1]) / 2) * 0.25];
    s.poly([wrist, a, sag, b], skin, (x, y) => Math.max(1, Math.min(4, 2 + tone + (y < wrist[1] ? 1 : 0))));
  }
  const last = tips[tips.length - 1];
  s.poly([wrist, last, [(last[0] + body[0]) / 2, (last[1] + body[1]) / 2 + 4], body, shoulder], skin, () => Math.max(1, 2 + tone));
  s.cap([[shoulder[0], shoulder[1], o.arm || 2.2], [wrist[0], wrist[1], (o.arm || 2.2) * 0.75]], bone, { tone });
  for (const t of tips) {
    const n = Math.max(Math.abs(t[0] - wrist[0]), Math.abs(t[1] - wrist[1]));
    for (let k = 0; k <= n; k++) {
      const x = Math.round(wrist[0] + ((t[0] - wrist[0]) * k) / n), y = Math.round(wrist[1] + ((t[1] - wrist[1]) * k) / n) + 1;
      const q = s.at(x, y);
      if (q && q.slot === skin) q.c = Math.min(4, q.c + 1);
    }
    s.line(wrist[0], wrist[1], t[0], t[1], bone, Math.max(1, 3 + tone), false);
  }
  if (o.claw !== false) s.poly([[wrist[0] - 1, wrist[1]], [wrist[0] - 3, wrist[1] - 4], [wrist[0] + 1, wrist[1] - 1]], o.clawSlot || "R", 4);
};
