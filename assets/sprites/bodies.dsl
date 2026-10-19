# Bodies, 96x96. 3/4 view facing right, feet on y = 91, figure about 70 px tall.
# Pixel by pixel (BITMAP, crates/render/FORMAT.md 1.2.1): digits 0-5 skin K, a-f hair H,
# g-l eyes Y, m-r dark D. Every race shares the silhouette and anchors below the neck:
# HAND_FRONT (63, 62), HAND_BACK (33, 62), HEAD_CENTER (48, 27), NECK (48, 37),
# BACK (44, 42), WAIST (48, 60), FEET (48, 91).

sprite 1 shadow
size 96 96
layer shadow
flags no_outline
ELLIPSE 33 88 30 6 P

sprite 2 human_arm_back
size 96 96
layer arm_back
BITMAP 31 36 9 30
....2332.
...233332
..2333332
..2333321
..233332.
..233332.
..233332.
..23332..
..23332..
..23332..
..23332..
..23332..
..23332..
..23332..
..23332..
.233322..
.23332...
.23332...
.23332...
.2332....
.2332....
.2332....
.2332....
.2332....
23332....
233432...
2334321..
2343321..
233221...
.2221....

sprite 3 human_legs
size 96 96
layer legs
BITMAP 40 55 17 36
pqqqqqqqqqqpppppo
pqqqqqqqqqppppppo
pqqqqqqqqqppppppo
pqqqqqqqqpppppppo
pqqqqqqqqpppppppo
pqqqqqqqoopppppo.
pqqqqqqo.pppppppo
ppqqqqpo.qpppppoo
oppppoo..ooooooo.
.3443322.3444332.
.3443322.3444332.
.3443322.3444332.
.3443322.3444332.
.3443322.3444332.
.2443322.3444332.
.2443322.3444332.
.2343322.3444322.
.2343322.3444322.
.2343322.3445322.
.2344432.3455432.
.2343322.3444322.
.2343322.3443322.
23443322.3443322.
2344332..344332..
2344332..344332..
2344332..344332..
.234432..344322..
.23432...34432...
.23432...34432...
.23432...34432...
.2343....3443....
.2343....3443....
.23443...34443...
.234443..3444432.
.2344432.34444432
.2222221.22222221

sprite 4 human_torso
size 96 96
layer torso
BITMAP 36 32 28 26
.........2333332............
.........2343332............
.........3443332............
........234443322...........
.....344444433332222..3444..
..3444554444444333332144443.
.344444444444444333314455433
3444444444443443333314454433
3444444444443443333314444433
3444444444443443333314444432
34444444444434433333144443..
344444444444344333333144432.
344444444444344333333144432.
.34444444443343333333144432.
.34444444443343333333144432.
..3333444443343333322144432.
..3444444443343333332144432.
...344444443343333332144432.
...344444443343333332144432.
....34444443343333322.......
....34444443343333322.......
.....344444334333322........
.....344444334333322........
.....344444334333322........
.....334444334333222........
.....233333333332222........

sprite 5 human_head
size 96 96
layer head
BITMAP 41 20 15 14
....3444433....
..34444444433..
.3444444444432.
.3444444444443.
34444444444443.
344443bb44bbb3.
342323kj44lj33.
342334334444443
342334444444443
23323444444322.
.233444443oo32.
.233344444432..
..22333344432..
.....2233332...

sprite 6 human_hair
size 96 96
layer hair
BITMAP 40 17 15 13
....cdddddc....
..cdeeeedddcc..
.cdefeeeedddcb.
cdeeeeedddddcbb
cdeeedddcccbb..
cdedd.c.cdc.c..
cddc...........
cddc...........
cdc............
cdc............
cdc............
cdb............
.cb............

sprite 7 human_arm_front
size 96 96
layer arm_front
BITMAP 57 50 9 16
3444322..
3444322..
.344432..
.3444432.
.3444432.
.3444432.
.344432..
.344432..
.344432..
..34432..
.3444432.
.34454432
.34444432
.3444432.
.344432..
..3432...

sprite 20 elf_arm_back
size 96 96
layer arm_back
copy human_arm_back

sprite 21 elf_legs
size 96 96
layer legs
copy human_legs

sprite 22 elf_torso
size 96 96
layer torso
copy human_torso

sprite 23 elf_head
size 96 96
layer head
# The shared skull and face with a long pointed ear swept back and up.
BITMAP 38 18 19 16
2..................
33.................
.34.....3444433....
.244.34444444433...
..24444444444444432
...2344444444444443
...3232344444444443
...3323343bb44bbb3.
...33342323kj44lj33
...3342334334444443
....342334444444443
....23323444444322.
.....233444443oo32.
.....233344444432..
......2233344432...
.........22333322..

sprite 24 elf_hair
size 96 96
layer hair
# Long fair hair from a middle part, tucked behind the ear and down the back.
BITMAP 34 15 22 23
........cddddddc......
......cdeffeeeedc.....
....cdeeffeeeeeddc....
...cdeeefeeeeeddddcb..
..cdeeeedeeeeedddccbb.
..cdeee..cdeeddcc.cdcb
.cdeed....cddc..c.cdc.
.cdeed......c.........
.cdee.................
cdeed.................
cdeed.................
cdeed.................
cdeed.................
cdeed.................
cddec.................
cdddc.................
cddc..................
cddc..................
.cdc..................
.cdc..................
.ccb..................
..cb..................
..b...................

sprite 25 elf_arm_front
size 96 96
layer arm_front
copy human_arm_front
