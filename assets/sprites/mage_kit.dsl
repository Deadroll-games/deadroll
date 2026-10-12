# Mage kit, 3/4 view facing right. Feet sit on y=30.
# Anchors: main hand (21,19), head (15,10), back (12,15), waist (15,22).

sprite 200 mage_hood
size 32 32
layer helmet
# cowl over the hair, face open: nothing covers x 14-19 at y 8-12
RECT 14 2 4 2 P
RECT 12 4 8 2 P
RECT 11 6 9 2 A
RECT 11 8 2 5 P
RECT 18 13 2 3 P
RECT 10 13 3 4 P

sprite 201 mage_robe
size 32 32
layer armor_body
# long robe, hem reaches y=28, short front slit
RECT 8 15 4 7 P
RECT 18 14 3 4 P
RECT 12 14 6 2 P
RECT 11 16 8 6 P
RECT 10 22 11 4 P
RECT 10 26 5 3 P
RECT 17 26 4 3 P
RECT 10 21 10 2 S
RECT 14 21 3 2 A
RECT 10 27 5 2 A
RECT 17 27 4 2 A

sprite 202 mage_pants
size 32 32
layer pants
# sits under the robe, peeks through the slit
RECT 12 23 7 2 P
RECT 12 25 3 4 P
RECT 15 25 4 4 P

sprite 203 mage_boots
size 32 32
layer boots
RECT 12 28 3 3 P
RECT 16 28 3 3 P
RECT 19 29 2 2 P

sprite 204 mage_staff
size 32 32
layer mainhand
# wooden shaft in S, element core in E above the head
RECT 21 1 4 4 E
RECT 21 5 4 2 S
RECT 22 7 2 24 S
RECT 20 19 4 2 A

sprite 205 mage_satchel
size 32 32
layer back_item
# small pouch on the back hip, diagonal strap from the shoulder
LINE 9 17 6 20 S
RECT 6 21 3 5 P
RECT 6 21 3 1 S