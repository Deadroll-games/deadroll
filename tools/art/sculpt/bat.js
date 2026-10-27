// Giant cave bat, 96, facing left, hovering: wings spread wide with finger bones and a scalloped,
// torn membrane (H), a furry body (K), big ears, a snarling snout with fangs, glowing eyes,
// small clawed feet. The lowest wing point is at about y = 80.
module.exports = {
  id: 427, name: "bat", size: 96,
  comment: [
    "Giant cave bat, facing left, hovering: wings spread with finger bones and a torn scalloped",
    "membrane, big ears, a snarling snout with fangs, glowing eyes, clawed feet.",
  ],
  build(s) {
    s.curve([0.62, 0.32, -0.06]);
    // Wing: wrist, finger tips, the membrane scalloped between fingers. far = darker, behind.
    const wing = (shoulder, wrist, tips, body, tone) => {
      const pts = [shoulder, wrist, ...tips, body];
      // Membrane: between consecutive fingers, the edge sags toward the body.
      for (let i = 0; i < tips.length; i++) {
        const a = i === 0 ? wrist : tips[i - 1], b = tips[i];
        const sag = [(a[0] + b[0]) / 2 + (body[0] - (a[0] + b[0]) / 2) * 0.25, (a[1] + b[1]) / 2 + (body[1] - (a[1] + b[1]) / 2) * 0.25];
        s.poly([wrist, a, sag, b], "H", (x, y) => Math.max(1, Math.min(4, 2 + tone + (y < wrist[1] ? 1 : 0))));
      }
      const last = tips[tips.length - 1];
      s.poly([wrist, last, [(last[0] + body[0]) / 2, (last[1] + body[1]) / 2 + 4], body, shoulder], "H",
        () => Math.max(1, 2 + tone));
      // Arm and finger bones.
      s.cap([[shoulder[0], shoulder[1], 2.2], [wrist[0], wrist[1], 1.6]], "K", { tone });
      for (const t of tips) {
        // A lit band of membrane along each finger, then the bone.
        const n = Math.max(Math.abs(t[0] - wrist[0]), Math.abs(t[1] - wrist[1]));
        for (let k = 0; k <= n; k++) {
          const x = Math.round(wrist[0] + ((t[0] - wrist[0]) * k) / n), y = Math.round(wrist[1] + ((t[1] - wrist[1]) * k) / n) + 1;
          const q = s.at(x, y);
          if (q && q.slot === "H") q.c = Math.min(4, q.c + 1);
        }
        s.line(wrist[0], wrist[1], t[0], t[1], "K", Math.max(1, 3 + tone), false);
      }
      s.poly([[wrist[0] - 1, wrist[1]], [wrist[0] - 3, wrist[1] - 4], [wrist[0] + 1, wrist[1] - 1]], "R", 4); // thumb claw
      return pts;
    };
    // Far wing (right, behind), then the body, then the near wing (left).
    wing([52, 50], [70, 36], [[82, 26], [90, 38], [90, 52], [80, 64]], [56, 62], -1);
    // Body: furry torso and hips, small hind legs with claws.
    s.mass([["e", 49, 56, 5.6, 7.5], ["e", 50, 63, 4, 4]], "K", { k: 1.5, relief: 1.5 });
    s.cap([[48, 65, 1.3], [47, 69, 1]], "K", { tone: -1 }); s.cap([[52, 65, 1.3], [53, 69, 1]], "K", { tone: -1 });
    for (const [x, y] of [[46, 70], [47, 70], [53, 70], [54, 70]]) s.px(x, y, "R", 3);
    wing([45, 50], [26, 36], [[14, 26], [6, 38], [6, 52], [16, 64]], [42, 62], 0);
    // Head: round skull, big pointed ears, a pushed-in snout.
    s.mass([["e", 45, 45, 6, 5.6], ["e", 40, 48, 3.4, 3]], "K", { k: 1.2, seam: true });
    s.poly([[44, 41], [41, 31], [47, 39]], "K", (x) => (x < 43 ? 4 : 2));
    s.poly([[48, 40], [52, 30], [52, 41]], "K", 1);
    s.seams();
    // Inner ears, eyes, nostrils, the snarl with fangs.
    s.line(43, 39, 42, 34, "D", 2); s.line(50, 39, 51, 34, "D", 1);
    s.px(42, 44, "Y", 4); s.px(43, 44, "Y", 5); s.px(47, 44, "Y", 3);
    s.px(37, 47, "D", 1); s.px(38, 46, "D", 1);
    s.patch(37, 49, [
      ".mmmmm.",
      "m4mmm4m",
      ".4...4.",
    ]);
    // Fur ruff on the chest and the torn holes in the membrane.
    for (const [x, y] of [[46, 52], [49, 51], [52, 53], [47, 56]]) s.px(x, y, "K", 4);
    for (const [x, y] of [[14, 40], [15, 40], [24, 52], [82, 44], [74, 56], [75, 56]]) s.erase(x, y);
  },
};
