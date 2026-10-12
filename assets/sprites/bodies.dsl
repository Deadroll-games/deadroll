# Bodies. 3/4 view facing right, feet on y=30.
# Human keeps the reference pose. Elf torso and legs are 1 px slimmer.

sprite 1 shadow
size 32 32
layer shadow
flags no_outline
RECT 10 31 12 1 P

sprite 2 human_arm_back
size 32 32
layer arm_back
RECT 9 16 2 3 K
RECT 9 19 3 2 K

sprite 3 human_legs
size 32 32
layer legs
RECT 12 23 3 8 K
RECT 16 23 3 8 K

sprite 4 human_torso
size 32 32
layer torso
RECT 12 15 7 8 K
RECT 14 14 3 2 K

sprite 5 human_head
size 32 32
layer head
RECT 12 6 7 8 K
RECT 19 8 2 4 K
RECT 14 8 2 2 D
RECT 17 8 2 2 D
PIXEL 14 10 Y
PIXEL 17 10 Y
PIXEL 16 12 D
PIXEL 17 12 D

sprite 6 human_hair
size 32 32
layer hair
RECT 11 7 2 6 H
RECT 12 5 7 2 H

sprite 7 human_arm_front
size 32 32
layer arm_front
RECT 19 16 2 3 K
RECT 20 19 3 2 K

sprite 20 elf_arm_back
size 32 32
layer arm_back
RECT 9 16 2 3 K
RECT 9 19 3 2 K

sprite 21 elf_legs
size 32 32
layer legs
RECT 12 23 2 8 K
RECT 16 23 2 8 K

sprite 22 elf_torso
size 32 32
layer torso
RECT 12 15 6 8 K
RECT 14 14 3 2 K

sprite 23 elf_head
size 32 32
layer head
# ear in K attached to the back of the head, tip points back and up
RECT 12 6 7 8 K
RECT 10 8 2 3 K
RECT 9 6 2 2 K
PIXEL 9 5 K
RECT 13 8 2 2 D
RECT 16 8 2 2 D
RECT 13 10 2 2 Y
RECT 16 10 2 2 Y
RECT 15 12 2 2 D

sprite 24 elf_hair
size 32 32
layer hair
# cap, then a long strand along the back edge of the body (x 10-11)
RECT 12 4 6 3 H
RECT 10 11 2 14 H

sprite 25 elf_arm_front
size 32 32
layer arm_front
RECT 18 16 2 4 K
RECT 20 19 3 2 K
