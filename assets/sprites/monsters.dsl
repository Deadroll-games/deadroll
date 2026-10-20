# Monsters and the tombstone. 3/4 view facing right.
# 64px sprites stand with feet on y=61. The chief is 96px, feet on y=93.

sprite 400 wolf
size 64 64
layer monster
# bushy tail, base buried in the rump
POLY H 26 34 8 32 8 22 18 18 28 28
# long torso
ELLIPSE 14 28 32 18 H
# head and snout toward the right
ELLIPSE 34 24 18 16 H
ELLIPSE 46 30 12 10 H
# ear
ELLIPSE 38 18 8 10 H
# four legs, far pair then near pair
RECT 16 42 6 20 H
RECT 24 44 6 18 H
RECT 34 42 6 20 H
RECT 42 44 6 18 H
# pale belly
ELLIPSE 22 36 18 10 K
# eyes, nose, mouth
RECT 42 30 2 2 Y
RECT 46 30 2 2 Y
RECT 54 32 4 4 D
RECT 48 36 8 4 D

sprite 401 goblin
size 64 64
layer monster
# big pointed ears first, head on top of them
POLY K 16 24 24 28 24 36
POLY K 48 24 40 28 40 36
ELLIPSE 22 22 20 18 K
# body, arms, legs with feet on y=61
RECT 24 38 16 14 K
RECT 18 40 6 10 K
RECT 40 40 8 6 K
RECT 24 50 6 12 K
RECT 34 50 6 12 K
# rags
RECT 24 42 16 8 H
# club: haft in the near hand, knob above
RECT 48 28 4 24 H
ELLIPSE 44 16 12 14 H
# eyes and mouth
RECT 28 28 2 2 Y
RECT 34 28 2 2 Y
RECT 28 34 8 2 D

sprite 402 goblin_chief
size 96 96
layer monster
# left horn, mirror builds the right one
POLY H 24 28 38 28 34 12 28 6 24 14
MIRROR_X 96
# head over the horn bases, crown band on top
ELLIPSE 30 20 36 28 K
RECT 40 18 16 4 H
# wide shoulders, arms, belt, legs, forward foot (soles on y=93)
POLY K 24 72 18 52 30 46 66 46 78 52 72 72
RECT 10 52 10 24 K
RECT 76 52 10 24 K
RECT 30 66 36 10 H
RECT 30 72 14 22 K
RECT 52 72 14 22 K
RECT 62 88 10 6 K
# brow, eyes, mouth
RECT 36 30 24 4 D
RECT 40 34 2 2 Y
RECT 54 34 2 2 Y
RECT 40 40 16 4 D
# axe: haft in the right hand, dark blade
RECT 80 24 6 52 H
POLY D 86 18 92 24 92 42 86 48

sprite 403 tombstone
size 64 64
layer monster
# stone slab with a rounded top
ELLIPSE 22 18 20 28 K
RECT 22 32 20 24 K
# one mound, left half mirrored about the center
POLY H 32 50 10 56 14 62 32 62
MIRROR_X 64
# cross cut into the slab
RECT 30 26 4 16 D
RECT 26 32 12 4 D
