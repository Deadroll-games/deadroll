# Mage kit, 3/4 view facing right. Feet sit on y=61.
# Anchors: main hand (21,19), head (15,10), back (12,15), waist (15,22).

sprite 200 mage_hood
size 64 64
layer helmet
# cowl over the hair, face open: nothing covers x 14-19 at y 8-12
RECT 28 4 8 4 P
RECT 24 8 16 4 P
RECT 22 12 18 4 A
RECT 22 16 4 10 P
RECT 36 26 4 6 P
RECT 20 26 6 8 P

sprite 201 mage_robe
size 64 64
layer armor_body
# long robe, hem reaches y=57, short front slit
RECT 16 30 8 14 P
RECT 36 28 6 8 P
RECT 24 28 12 4 P
RECT 22 32 16 12 P
RECT 20 44 22 8 P
RECT 20 52 10 6 P
RECT 34 52 8 6 P
RECT 20 42 20 4 S
RECT 28 42 6 4 A
RECT 20 54 10 4 A
RECT 34 54 8 4 A

sprite 202 mage_pants
size 64 64
layer pants
# sits under the robe, peeks through the slit
RECT 24 46 14 4 P
RECT 24 50 6 8 P
RECT 30 50 8 8 P

sprite 203 mage_boots
size 64 64
layer boots
RECT 24 56 6 6 P
RECT 32 56 6 6 P
RECT 38 58 4 4 P

sprite 204 mage_staff
size 64 64
layer mainhand
# wooden shaft in S, element core in E above the head
RECT 42 2 8 8 E
RECT 42 10 8 4 S
RECT 44 14 4 48 S
RECT 40 38 8 4 A

sprite 205 mage_satchel
size 64 64
layer back_item
# small pouch on the back hip, diagonal strap from the shoulder
LINE 18 34 12 40 S
LINE 18 35 12 41 S
RECT 12 42 6 10 P
RECT 12 42 6 2 S
