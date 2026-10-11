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
# pointed ears, far then near, clear of the club
POLY K 7 15 11 14 12 20 8 21
POLY K 15 15 19 9 17 15 16 20
# big head, short torso
ELLIPSE 11 15 10 9 K
ELLIPSE 12 22 8 6 K
# arms, the near one reaches the club
RECT 8 22 3 6 K
RECT 18 20 6 6 K
# short legs, feet on y=30
RECT 12 24 3 7 K
RECT 16 24 3 7 K
# rags
RECT 10 22 12 4 H
# club: haft in the hand, round head above and to the right
RECT 23 12 3 12 H
ELLIPSE 22 7 8 7 H
# eyes and mouth
PIXEL 14 18 Y
PIXEL 17 18 Y
RECT 14 20 4 2 D

sprite 402 goblin_chief
size 48 48
layer monster
# left horn with a flat tip, ridge, and ear; mirror builds the right side
POLY H 13 17 21 16 19 5 15 5 12 9
POLY H 16 15 19 14 18 7 16 7
POLY K 8 18 14 15 14 24 8 23
MIRROR_X 48
# heavy head
ELLIPSE 16 12 16 12 K
# wide shoulders, narrower waist
POLY K 16 34 7 25 16 22 32 22 41 25 32 34
# crown band over the horn bases
RECT 18 14 12 3 H
RECT 22 15 4 2 H
# brow, eyes, snarl
RECT 19 17 12 2 D
PIXEL 22 19 Y
PIXEL 28 19 Y
RECT 21 21 8 2 D
# hip rags
RECT 15 31 18 5 H
# legs centered, forward foot, soles on y=46
RECT 14 33 6 14 K
RECT 24 33 6 14 K
RECT 29 43 5 4 K
# far arm and the axe arm
RECT 4 24 8 12 K
RECT 33 22 8 12 K
# haft and a wide dark axe head
RECT 39 18 4 14 H
POLY D 34 12 46 8 46 20 36 22

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
