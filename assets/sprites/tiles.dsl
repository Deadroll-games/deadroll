# Map tiles, one 64x64 cell each, drawn at the same pixel size as the 96 px characters.
# Item slots only. Flagstones are polygons (auto shaded and dithered), cracks and chips are
# BITMAP overlays (`.` keeps the stone under them).

# Flagstones. A 1 px grout line on every edge makes 2 px between cells, so the floor tiles.
sprite 500 tile_floor
size 64 64
layer tile
flags no_outline
RECT 0 0 64 64 O
POLY P 1 1 30 1 31 22 27 25 1 24
POLY P 33 1 63 1 63 20 36 24 33 21
POLY P 1 27 20 26 23 44 21 63 1 63
POLY P 25 28 46 26 45 46 44 63 23 63 25 46
POLY P 49 23 63 22 63 63 46 63 47 46
BITMAP 4 4 22 10
.2......4.........2...
......2.......4.......
...4.........2.......2
.........2.........4..
2....2.........2......
........4..2.......2..
..2...........4.......
.....2....2.........2.
.4..........2...2.....
.......2..........4...
BITMAP 37 3 22 10
.2......4.........2...
......2.......4.......
...4.........2.......2
.........2.........4..
2....2.........2......
........4..2.......2..
..2...........4.......
.....2....2.........2.
.4..........2...2.....
.......2..........4...
BITMAP 5 30 22 10
.2......4.........2...
......2.......4.......
...4.........2.......2
.........2.........4..
2....2.........2......
........4..2.......2..
..2...........4.......
.....2....2.........2.
.4..........2...2.....
.......2..........4...
BITMAP 28 31 22 10
.2......4.........2...
......2.......4.......
...4.........2.......2
.........2.........4..
2....2.........2......
........4..2.......2..
..2...........4.......
.....2....2.........2.
.4..........2...2.....
.......2..........4...
BITMAP 39 46 22 10
.2......4.........2...
......2.......4.......
...4.........2.......2
.........2.........4..
2....2.........2......
........4..2.......2..
..2...........4.......
.....2....2.........2.
.4..........2...2.....
.......2..........4...
BITMAP 5 50 22 10
.2......4.........2...
......2.......4.......
...4.........2.......2
.........2.........4..
2....2.........2......
........4..2.......2..
..2...........4.......
.....2....2.........2.
.4..........2...2.....
.......2..........4...
BITMAP 7 4 4 6
...1
..14
.14.
14..
.1..
..1.
BITMAP 50 30 4 7
..1.
.14.
14..
1...
.1..
..14
...1
BITMAP 28 48 8 4
1.......
41.....1
.41..114
..4111..
BITMAP 41 6 2 3
.2
24
2.
BITMAP 8 38 2 2
2.
42

# Block wall in 3/4: the top plane in the accent, the front face of uneven blocks with chips.
sprite 501 tile_wall
size 64 64
layer tile
RECT 2 2 60 13 A
RECT 2 15 60 47 O
RECT 3 16 17 10 P
RECT 22 16 22 10 P
RECT 46 16 15 10 P
RECT 3 28 9 10 P
RECT 14 28 21 10 P
RECT 37 28 24 10 P
RECT 3 40 18 10 P
RECT 23 40 17 10 P
RECT 42 40 19 10 P
RECT 3 52 12 9 P
RECT 17 52 23 9 P
RECT 42 52 19 9 P
BITMAP 2 2 60 12
kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk
kjjjjjjjjjjjjjjjjjjhjjjjjjjjjjjjjjjjjjjjhjjjjjjjjjjjjjjjjjji
kjjjjjjjjjjjjjjjjjjhjjjjjjjjjjjjjjjjjjjjhjjjjjjjjjjjjjjjjjji
kjjjjjjjjjjjjjjjjjjhjjjjjjjjjjjjjjjjjjjjhjjjjjjjjjjjjjjjjjji
kjjjjjjjjjjjjjjjjjjhjjjjjjjjjjjjjjjjjjjjhjjjjjjjjjjjjjjjjjji
hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh
kjjjjjjjjjjjhjjjjjjjjjjjjjjjjjjjhjjjjjjjjjjjjjjjjjjjjjjjhjji
kjjjjjjjjjjjhjjjjjjjjjjjjjjjjjjjhjjjjjjjjjjjjjjjjjjjjjjjhjji
kjjjjjjjjjjjhjjjjjjjjjjjjjjjjjjjhjjjjjjjjjjjjjjjjjjjjjjjhjji
kjjjjjjjjjjjhjjjjjjjjjjjjjjjjjjjhjjjjjjjjjjjjjjjjjjjjjjjhjji
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii
gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg
BITMAP 11 17 23 3
.1....................1
14...................14
.1....................1
BITMAP 21 31 2 2
21
.1
BITMAP 44 42 3 4
..1
.14
14.
.1.
BITMAP 4 54 2 2
.1
14

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

# Floor plate with spikes: the flagstones, a sunken iron plate and five spikes.
sprite 506 tile_trap
size 64 64
layer tile
flags no_outline
copy tile_floor
BITMAP 14 17 36 23
abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
bccccccccccccccccccccccccccccccccccd
bcddddddddddddddddddddddddddddddddde
bcddddddddddddddddddddddddddddddddde
bcddaaaaaadddddaaaaaadddddaaaaaaddde
bcddaaiaaadddddaaiaaadddddaaiaaaddde
bcddaaihaadddddaaihaadddddaaihaaddde
bcddajihaadddddajihaadddddajihaaddde
bcddajihhadddddajihhadddddajihhaddde
bcddkjihhadddddkjihhadddddkjihhaddde
bcddkjiihhdddddkjiihhdddddkjiihhddde
bcddccccccdddddccccccdddddccccccddde
bcddddddddddddddddddddddddddddddddde
bcddddddddaaaaaadddddaaaaaadddddddde
bcddddddddaaiaaadddddaaiaaadddddddde
bcddddddddaaihaadddddaaihaadddddddde
bcddddddddajihaadddddajihaadddddddde
bcddddddddajihhadddddajihhadddddddde
bcddddddddkjihhadddddkjihhadddddddde
bcddddddddkjiihhdddddkjiihhdddddddde
bcddddddddccccccdddddccccccdddddddde
bcddddddddddddddddddddddddddddddddde
beeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeef
