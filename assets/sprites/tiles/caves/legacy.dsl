# Map tiles, one 64x64 cell each, drawn at the same pixel size as the 96 px characters.
# Item slots only. Flagstones are polygons (auto shaded and dithered), cracks and chips are
# BITMAP overlays (`.` keeps the stone under them).

# Old tiles kept until the new Caves and Ruins set replaces them.

# Map tiles, one 64x64 cell each, drawn at the same pixel size as the 96 px characters.
# Item slots only. Flagstones are polygons (auto shaded and dithered), cracks and chips are
# BITMAP overlays (`.` keeps the stone under them).



# Floor plate with spikes: the flagstones, a sunken iron plate and five spikes.
sprite 506 tile_trap
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
