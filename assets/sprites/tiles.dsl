# Map tiles, one cell each. Item slots only.

# Flagstones. Grout meets the next cell so the floor stays seamless.
sprite 500 tile_floor
size 32 32
layer tile
flags no_outline
RECT 0 0 32 32 P
RECT 0 0 32 2 S
RECT 0 16 32 2 S
RECT 0 2 2 14 S
RECT 16 2 2 14 S
RECT 8 18 2 14 S
RECT 24 18 2 14 S
PIXEL 5 8 A
PIXEL 22 6 A
PIXEL 12 24 A
SHADE 27 22 P 3
SHADE 6 26 P 2

# Block wall. Lighter top plane, brick joints on the front.
sprite 501 tile_wall
size 32 32
layer tile
RECT 1 1 30 6 A
RECT 1 7 30 2 S
RECT 1 9 30 22 P
RECT 1 16 30 2 S
RECT 1 23 30 2 S
RECT 6 9 2 7 S
RECT 16 9 2 7 S
RECT 26 9 2 7 S
RECT 1 18 2 5 S
RECT 11 18 2 5 S
RECT 21 18 2 5 S
RECT 6 25 2 6 S
RECT 16 25 2 6 S
RECT 26 25 2 6 S

# Water. The edge stays flat so neighboring pools meet.
sprite 502 tile_water
size 32 32
layer tile
flags no_outline
RECT 0 0 32 32 P
ELLIPSE 3 4 12 5 A
ELLIPSE 15 11 13 5 A
ELLIPSE 5 19 14 5 A
RECT 18 6 8 2 A

# Wooden door. Arch and slab in the primary, planks and a handle.
sprite 503 tile_door
size 32 32
layer tile
ELLIPSE 4 2 24 14 P
RECT 4 9 24 21 P
RECT 9 9 2 21 S
RECT 15 9 2 21 S
RECT 21 9 2 21 S
RECT 4 18 24 2 S
RECT 18 15 2 3 A

# Floor with stairs down and an arrow.
sprite 504 tile_entrance
size 32 32
layer tile
flags no_outline
RECT 0 0 32 32 P
RECT 0 0 32 2 S
RECT 0 16 32 2 S
RECT 0 2 2 14 S
RECT 16 2 2 14 S
RECT 8 18 2 14 S
RECT 24 18 2 14 S
RECT 6 6 20 2 S
RECT 8 10 16 2 S
RECT 10 14 12 2 S
RECT 12 18 8 2 S
POLY A 14 16 18 16 18 20 22 20 16 26 10 20 14 20

# Floor with stairs up into a gate. The opening glows.
sprite 505 tile_exit
size 32 32
layer tile
flags no_outline
RECT 0 0 32 32 P
RECT 0 0 32 2 S
RECT 0 16 32 2 S
RECT 0 2 2 14 S
RECT 16 2 2 14 S
RECT 8 18 2 14 S
RECT 24 18 2 14 S
RECT 6 26 20 2 S
RECT 8 22 16 2 S
RECT 10 8 3 14 P
RECT 19 8 3 14 P
RECT 10 8 12 3 P
RECT 13 11 6 11 E
SHADE 15 15 E 3

# Floor plate with spikes. Item slots only, no dark-detail slot.
sprite 506 tile_trap
size 32 32
layer tile
flags no_outline
RECT 0 0 32 32 P
RECT 0 0 32 2 S
RECT 0 16 32 2 S
RECT 0 2 2 14 S
RECT 16 2 2 14 S
RECT 8 18 2 14 S
RECT 24 18 2 14 S
RECT 7 8 20 16 S
POLY A 9 16 13 16 11 12
POLY A 15 16 19 16 17 12
POLY A 21 16 25 16 23 12
POLY A 12 22 16 22 14 18
POLY A 18 22 22 22 20 18
