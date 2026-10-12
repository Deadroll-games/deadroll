# Monsters and the tombstone. 3/4 view facing right.
# 32px sprites stand with feet on y=30. The chief is 48px, feet on y=46.

sprite 400 wolf
size 32 32
layer monster
# bushy tail, base buried in the rump
POLY H 13 17 4 16 4 11 9 9 14 14
# long torso
ELLIPSE 7 14 16 9 H
# head and snout toward the right
ELLIPSE 17 12 9 8 H
ELLIPSE 23 15 6 5 H
# ear
ELLIPSE 19 9 4 5 H
# four legs, far pair then near pair
RECT 8 21 3 10 H
RECT 12 22 3 9 H
RECT 17 21 3 10 H
RECT 21 22 3 9 H
# pale belly
ELLIPSE 11 18 9 5 K
# eyes, nose, mouth
PIXEL 21 15 Y
PIXEL 23 15 Y
RECT 27 16 2 2 D
RECT 24 18 4 2 D

sprite 401 goblin
size 32 32
layer monster
# big pointed ears first, head on top of them
POLY K 8 12 12 14 12 18
POLY K 24 12 20 14 20 18
ELLIPSE 11 11 10 9 K
# body, arms, legs with feet on y=30
RECT 12 19 8 7 K
RECT 9 20 3 5 K
RECT 20 20 4 3 K
RECT 12 25 3 6 K
RECT 17 25 3 6 K
# rags
RECT 12 21 8 4 H
# club: haft in the near hand, knob above
RECT 24 14 2 12 H
ELLIPSE 22 8 6 7 H
# eyes and mouth
PIXEL 14 14 Y
PIXEL 17 14 Y
RECT 14 17 4 1 D

sprite 402 goblin_chief
size 48 48
layer monster
# left horn, mirror builds the right one
POLY H 12 14 19 14 17 6 14 3 12 7
MIRROR_X 48
# head over the horn bases, crown band on top
ELLIPSE 15 10 18 14 K
RECT 20 9 8 2 H
# wide shoulders, arms, belt, legs, forward foot (soles on y=46)
POLY K 12 36 9 26 15 23 33 23 39 26 36 36
RECT 5 26 5 12 K
RECT 38 26 5 12 K
RECT 15 33 18 5 H
RECT 15 36 7 11 K
RECT 26 36 7 11 K
RECT 31 44 5 3 K
# brow, eyes, mouth
RECT 18 15 12 2 D
PIXEL 20 17 Y
PIXEL 27 17 Y
RECT 20 20 8 2 D
# axe: haft in the right hand, dark blade
RECT 40 12 3 26 H
POLY D 43 9 46 12 46 21 43 24

sprite 403 tombstone
size 32 32
layer monster
# stone slab with a rounded top
ELLIPSE 11 9 10 14 K
RECT 11 16 10 12 K
# one mound, left half mirrored about the center
POLY H 16 25 5 28 7 31 16 31
MIRROR_X 32
# cross cut into the slab
RECT 15 13 2 8 D
RECT 13 16 6 2 D
