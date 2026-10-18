# Support kit for the reference human body. 3/4 view facing right.
# Feet on y=61. Anchors: main hand (42,38), neck (30,30), back (24,30), waist (30,44).

sprite 300 support_cowl
size 64 64
layer helmet
# cloth cowl over the hair, circlet band, face open
RECT 26 4 12 4 P
RECT 24 8 16 4 P
RECT 24 12 16 4 A
RECT 20 16 4 12 P
RECT 36 26 6 4 P
RECT 20 24 6 8 P

sprite 301 support_tabard
size 64 64
layer armor_body
# light tabard, split hem, sash in A at the waist
RECT 24 28 14 4 P
RECT 22 32 18 12 P
RECT 24 44 6 10 P
RECT 32 44 6 10 P
RECT 24 42 14 4 A
RECT 26 32 4 10 S

sprite 302 support_boots
size 64 64
layer boots
# soft boots, soles on y=61, cuff in S
RECT 24 54 6 8 P
RECT 32 54 6 8 P
RECT 38 58 4 4 P
RECT 24 54 6 4 S
RECT 32 54 6 4 S

sprite 303 support_mace
size 64 64
layer mainhand
# mace in the front hand, 6x6 element core on top
RECT 42 12 6 6 E
RECT 42 18 6 4 P
RECT 44 22 4 16 S
RECT 42 38 8 4 A
RECT 44 42 4 4 S
RECT 44 46 4 4 P

sprite 304 support_bag
size 64 64
layer back_item
# small shoulder bag, strap runs diagonally over the back
LINE 24 32 16 40 S
LINE 24 33 16 41 S
RECT 12 42 10 10 P
RECT 12 42 10 4 S
RECT 16 48 4 2 A

sprite 305 support_amulet
size 64 64
layer necklace
flags no_outline
# small accent pendant at the neck
RECT 28 28 6 4 A
RECT 30 32 4 4 A
RECT 30 32 2 2 A
SHADE 30 32 A 2
