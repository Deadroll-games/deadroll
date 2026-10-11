---
name: pixel-sprite
description: Use when drawing characters, monsters, items or tiles as sprite DSL files in assets/sprites. Sprites are geometry (rects, pixels, ellipses) with role slots, auto-shaded and outlined by crates/render, never hand-painted bitmaps.
---

# Pixel sprites from geometry

Full standard: `docs/ART_DESCRIPTION.md`. Exact text and binary format: `crates/render/FORMAT.md`.
You write **one text file per sprite** in `assets/sprites/<name>.dsl`. `tools/art` compiles it, renders a preview PNG, and `tools/scripts/upload-sprites.ts` packs it into the on-chain `SpriteBook`. The shading and outline below are done by the rasterizer: you only place shapes and slots.

## Workflow

1. Copy the reference layers below as the base pose (same anchors for every body item).
2. Write the `.dsl` file, one layer per block.
3. `cargo run -p art -- assets/sprites/<name>.dsl --scale 8 --out /tmp/<name>.png` and look at the PNG.
4. Fix until the checklist passes. Touch only your own `.dsl` file.

## Canvas and pose

- Character and monster layers: **32x32**, 3/4 view facing right, feet on y = 30.
- Anchors: main hand (21,19), off hand (10,19), head center (15,10), neck (15,15), waist (15,22), feet (15,30).
- Tiles: 32x32 per map cell, floors without outline.
- Render with `imageSmoothingEnabled = false` and integer scaling only.

## Slots, not colors

Commands set a **role slot**; colors come from material ramps (dark -> light):
- items/armor: `P` primary material, `S` secondary (handle, leather), `A` accent (gold, gems), `E` element core;
- body: `K` skin, `H` hair, eyes and mouth are fixed dark colors.

## Commands

`rect(x,y,w,h,slot)`, `pixel(x,y,slot)`, `ellipse(cx,cy,rx,ry,slot)`. Later commands overwrite earlier ones. Keep 1 px margin from the canvas edge for the outline.

## Auto shading (light from top-left)

For each pixel of slot `c`, a neighbour is "own" if it has the same slot in the same layer:
```
L,T,R,B = left/top/right/bottom neighbour is NOT own; n = ramp length
if (L||T) && !(R||B): shade = (L&&T) ? n-1 : min(2,n-1)
else if (R||B) && !(L||T): shade = 0
else: shade = 1
```

## Outline

Per layer: an empty pixel with a non-empty 4-neighbour in the same layer becomes outline `#120c16`. Draw outline over lower layers, then the layer fill.

## Layer order (bottom -> top)

shadow, cape, back arm, back glove, off-hand, legs, pants, boots, torso, body armor, necklace, head, hair, helmet, front arm, front glove, main hand, wounds overlay, element overlay, effects.

## Reference layers (warrior, 32x32)

```
shadow   rect(10,31,12,1,'D')                         (no outline, black 35%)
cape     rect(10,16,6,11,P) rect(9,19,1,8,P)           ramp cape
backArm  rect(9,16,2,3,K) rect(9,19,3,2,K)
shield   rect(4,15,5,1,P) rect(3,16,7,6,P) rect(4,22,5,1,P) rect(5,23,3,1,P) rect(6,24,1,1,P) rect(5,18,3,3,A)
legs     rect(12,23,3,8,K) rect(16,23,3,8,K)
pants    rect(12,23,7,2,P) rect(12,25,3,3,P) rect(16,25,3,3,P)
boots    rect(12,28,3,3,P) rect(16,28,3,3,P) rect(19,29,1,2,P) rect(12,28,3,1,S) rect(16,28,3,1,S)
torso    rect(12,15,7,8,K) rect(14,14,3,1,K)
armor    rect(11,15,9,7,P) rect(18,14,3,3,P) rect(10,14,2,3,P) rect(11,22,9,1,S) pixel(15,22,A)
head     rect(12,6,7,8,K) eyes (14,10),(17,10) mouth (16,12),(17,12)
hair     rect(11,7,2,6,H) rect(12,5,7,2,H)
helmet   rect(13,3,5,1,P) rect(12,4,7,1,P) rect(11,5,9,3,P) rect(11,8,9,1,A) rect(11,9,2,3,P)
frontArm rect(19,16,2,3,K) rect(20,19,3,2,K)
sword    rect(22,5,2,12,P) pixel(22,4,P) rect(20,17,6,1,A)
```

## Ramps used in the demo

steel `#3a4250 #5c6676 #8a94a4 #c8d0dc`, skin `#7a4a32 #a86c4a #d09268 #ecbc92`,
robe `#1e1638 #2e2257 #43337f #5c48a8`, green `#14321e #1f4d2c #2f6e3e #4a9455`,
leather `#3a2416 #5e3c24 #86583a #a8764e`, gold `#8a5e10 #d9a832 #f6e08a`,
cold `#1d3f63 #2a5f8a #4f8fc0 #cfe8ff`, fire `#5a1208 #a8300c #e8741c #ffd860`.

## Checklist

- [ ] Silhouette reads when filled with one color
- [ ] Weapon sticks out of the body silhouette
- [ ] No detail thinner than 2 px except eyes, bowstrings, chains
- [ ] Same anchors as the reference body
- [ ] Dead character: draw a tombstone sprite + gray ramps
