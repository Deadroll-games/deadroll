# Support kit for the reference human body. 3/4 view facing right.
# Feet on y=30. Anchors: main hand (21,19), neck (15,15), back (12,15), waist (15,22).

sprite 300 support_cowl
size 32 32
layer helmet
# cloth cowl over the hair, circlet band, face open
RECT 13 2 6 2 P
RECT 12 4 8 2 P
RECT 11 8 3 6 P
RECT 18 8 3 5 P
RECT 10 12 3 4 P
RECT 12 6 8 2 A

sprite 301 support_tabard
size 32 32
layer armor_body
# light tabard, split hem, sash in A at the waist
RECT 12 14 7 2 P
RECT 11 16 9 6 P
RECT 12 22 3 5 P
RECT 16 22 3 5 P
RECT 12 21 7 2 A
RECT 13 16 2 5 S

sprite 302 support_boots
size 32 32
layer boots
# soft boots, soles on y=30, cuff in S
RECT 12 27 3 4 P
RECT 16 27 3 4 P
RECT 19 29 2 2 P
RECT 12 27 3 2 S
RECT 16 27 3 2 S

sprite 303 support_mace
size 32 32
layer mainhand
# mace in the front hand, 3x3 element core on top
RECT 21 6 3 3 E
RECT 21 9 3 2 P
RECT 22 11 2 8 S
RECT 21 19 4 2 A
RECT 22 21 2 2 S
RECT 22 23 2 2 P

sprite 304 support_bag
size 32 32
layer back_item
# shoulder bag hanging from the back
RECT 7 15 6 2 S
RECT 6 16 3 6 S
RECT 4 21 8 7 P
RECT 4 21 8 2 S
RECT 6 24 3 2 A

sprite 305 support_amulet
size 32 32
layer necklace
flags no_outline
# small accent pendant at the neck
RECT 14 14 3 2 A
RECT 15 16 2 2 A
SHADE 15 16 A 2
