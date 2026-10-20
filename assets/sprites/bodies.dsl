# Bodies. 3/4 view facing right, feet on y=61.
# Human keeps the reference pose. Elf torso and legs are 2 px slimmer.

sprite 1 shadow
size 64 64
layer shadow
flags no_outline
RECT 20 62 24 2 P

sprite 2 human_arm_back
size 64 64
layer arm_back
RECT 18 32 4 6 K
RECT 18 38 6 4 K

sprite 3 human_legs
size 64 64
layer legs
RECT 24 46 6 16 K
RECT 32 46 6 16 K

sprite 4 human_torso
size 64 64
layer torso
RECT 24 30 14 16 K
RECT 28 28 6 4 K

sprite 5 human_head
size 64 64
layer head
RECT 24 12 14 16 K
RECT 38 16 4 8 K
RECT 28 16 4 4 D
RECT 34 16 4 4 D
RECT 28 20 2 2 Y
RECT 34 20 2 2 Y
RECT 32 24 2 2 D
RECT 34 24 2 2 D

sprite 6 human_hair
size 64 64
layer hair
RECT 22 14 4 12 H
RECT 24 10 14 4 H

sprite 7 human_arm_front
size 64 64
layer arm_front
RECT 38 32 4 6 K
RECT 40 38 6 4 K

sprite 20 elf_arm_back
size 64 64
layer arm_back
RECT 18 32 4 6 K
RECT 18 38 6 4 K

sprite 21 elf_legs
size 64 64
layer legs
RECT 24 46 4 16 K
RECT 32 46 4 16 K

sprite 22 elf_torso
size 64 64
layer torso
RECT 24 30 12 16 K
RECT 28 28 6 4 K

sprite 23 elf_head
size 64 64
layer head
# ear in K attached to the back of the head, tip points back and up
RECT 24 12 14 16 K
RECT 20 16 4 6 K
RECT 18 12 4 4 K
RECT 18 10 2 2 K
RECT 26 16 4 4 D
RECT 32 16 4 4 D
RECT 26 20 4 4 Y
RECT 32 20 4 4 Y
RECT 30 24 4 4 D

sprite 24 elf_hair
size 64 64
layer hair
# cap, then a long strand along the back edge of the body (x 10-11)
RECT 24 8 12 6 H
RECT 20 22 4 28 H

sprite 25 elf_arm_front
size 64 64
layer arm_front
RECT 36 32 4 8 K
RECT 40 38 6 4 K
