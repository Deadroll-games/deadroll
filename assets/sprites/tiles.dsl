# Map tiles, one cell each. Item slots only.

# Flagstones. Grout meets the next cell so the floor stays seamless.
sprite 500 tile_floor
size 64 64
layer tile
flags no_outline
RECT 0 0 64 64 P
RECT 0 0 64 4 S
RECT 0 32 64 4 S
RECT 0 4 4 28 S
RECT 32 4 4 28 S
RECT 16 36 4 28 S
RECT 48 36 4 28 S
RECT 10 16 2 2 A
RECT 44 12 2 2 A
RECT 24 48 2 2 A
RECT 54 44 2 2 P
SHADE 54 44 P 5
RECT 12 52 2 2 P
SHADE 12 52 P 3

# Block wall. Lighter top plane, brick joints on the front.
sprite 501 tile_wall
size 64 64
layer tile
RECT 2 2 60 12 A
RECT 2 14 60 4 S
RECT 2 18 60 44 P
RECT 2 32 60 4 S
RECT 2 46 60 4 S
RECT 12 18 4 14 S
RECT 32 18 4 14 S
RECT 52 18 4 14 S
RECT 2 36 4 10 S
RECT 22 36 4 10 S
RECT 42 36 4 10 S
RECT 12 50 4 12 S
RECT 32 50 4 12 S
RECT 52 50 4 12 S

# Water. The edge stays flat so neighboring pools meet.
sprite 502 tile_water
size 64 64
layer tile
flags no_outline
RECT 0 0 64 64 P
ELLIPSE 6 8 24 10 A
ELLIPSE 30 22 26 10 A
ELLIPSE 10 38 28 10 A
RECT 36 12 16 4 A

# Wooden door. Arch and slab in the primary, planks and a handle.
sprite 503 tile_door
size 64 64
layer tile
ELLIPSE 8 4 48 28 P
RECT 8 18 48 42 P
RECT 18 18 4 42 S
RECT 30 18 4 42 S
RECT 42 18 4 42 S
RECT 8 36 48 4 S
RECT 36 30 4 6 A

# Floor with stairs down and an arrow.
sprite 504 tile_entrance
size 64 64
layer tile
flags no_outline
RECT 0 0 64 64 P
RECT 0 0 64 4 S
RECT 0 32 64 4 S
RECT 0 4 4 28 S
RECT 32 4 4 28 S
RECT 16 36 4 28 S
RECT 48 36 4 28 S
RECT 12 12 40 4 S
RECT 16 20 32 4 S
RECT 20 28 24 4 S
RECT 24 36 16 4 S
POLY A 28 32 36 32 36 40 44 40 32 52 20 40 28 40

# Floor with stairs up into a gate. The opening glows.
sprite 505 tile_exit
size 64 64
layer tile
flags no_outline
RECT 0 0 64 64 P
RECT 0 0 64 4 S
RECT 0 32 64 4 S
RECT 0 4 4 28 S
RECT 32 4 4 28 S
RECT 16 36 4 28 S
RECT 48 36 4 28 S
RECT 12 52 40 4 S
RECT 16 44 32 4 S
RECT 20 16 6 28 P
RECT 38 16 6 28 P
RECT 20 16 24 6 P
RECT 26 22 12 22 E
RECT 30 30 2 2 E
SHADE 30 30 E 5

# Floor plate with spikes. Item slots only, no dark-detail slot.
sprite 506 tile_trap
size 64 64
layer tile
flags no_outline
RECT 0 0 64 64 P
RECT 0 0 64 4 S
RECT 0 32 64 4 S
RECT 0 4 4 28 S
RECT 32 4 4 28 S
RECT 16 36 4 28 S
RECT 48 36 4 28 S
RECT 14 16 40 32 S
POLY A 18 32 26 32 22 24
POLY A 30 32 38 32 34 24
POLY A 42 32 50 32 46 24
POLY A 24 44 32 44 28 36
POLY A 36 44 44 44 40 36
