// A torn insect wing in slot R: dark rim, lighter cells, veins from the root, notches torn out.
// pts: outline (first point = the root at the back). tone: -1 for the far pair. tears: [x, y] holes.
module.exports = (s, pts, tone, tears = []) => {
  const [rx, ry] = pts[0];
  const inside = (x, y) => {
    let c = false;
    for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
      const [xi, yi] = pts[i], [xj, yj] = pts[j];
      if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) c = !c;
    }
    return c;
  };
  // Cells: light in the middle of the wing, darker toward the rim (thin membrane catching light).
  s.poly(pts, "R", (x, y) => {
    let edge = 0;
    for (const [dx, dy] of [[2, 0], [-2, 0], [0, 2], [0, -2]]) if (!inside(x + dx + 0.5, y + dy + 0.5)) edge++;
    return Math.max(1, Math.min(4, (edge ? 2 : 3) + tone + (y < ry - 12 ? 1 : 0)));
  });
  // Veins: from the root to every outline point but the neighbors of the root, and a cross vein.
  for (let i = 2; i < pts.length - 1; i++) s.line(rx, ry, pts[i][0], pts[i][1], "R", Math.max(0, 1 + tone), false);
  // Rim.
  for (let i = 0; i < pts.length; i++) {
    const [a, b] = [pts[i], pts[(i + 1) % pts.length]];
    s.line(Math.round(a[0]), Math.round(a[1]), Math.round(b[0]), Math.round(b[1]), "R", Math.max(0, 1 + tone), false);
  }
  // Tears: small holes through the membrane.
  for (const [x, y] of tears) { s.erase(x, y); s.erase(x + 1, y); s.erase(x, y + 1); }
};
