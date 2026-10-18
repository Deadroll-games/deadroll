# programs/items

Item NFTs, their state, affixes, artisan listings and crafting (`docs/ITEMS_AND_CRAFT.md`), none of
which is in the game yet. Program id `48FtbrDegNrAUHFza9HdJzZAvUo3uUNqs1kNnZxFsqJS`.

The class starter items (soulbound) and what a character wears live in the character's own account
(`docs/CHARACTER.md` 10, `docs/ITEMS_AND_CRAFT.md` 2 and 10): `deadroll_character` `Character.gear`,
changed by `character::equip` / `character::unequip`. The former `Loadout` account and its
instructions (`init_loadout`, `equip`, `unequip`) were removed in patch 1.

## Instructions

None yet. The program stays deployed under its id so the item NFT work can land here.

## Errors (append only)

`Unauthorized, NotOwner, CharacterLocked, CharacterDead, InvalidCharacter, NoSuchItem, WrongSlot,
LevelTooLow, SlotEmpty, SessionTokenMissing, InvalidSessionToken, SessionTokenExpired`: kept
reserved from the removed `Loadout` instructions.

## Commands

```
anchor build --tools-version v1.54 -p deadroll_items
```
