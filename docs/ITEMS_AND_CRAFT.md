# ITEMS_AND_CRAFT: items, inventory and crafting in Deadroll

Version 0.1, draft. The document covers resources, items, their affixes and levels, the bag and storage, professions, orders to artisans and starter items. **There are no numbers in this document**, they will appear at balancing.

Related documents:

- `ART_DESCRIPTION.md`: how an item is drawn from materials, enchantments, runes and sockets.
- `CHARACTER.md`: character level, race traits (Dwarf and crafting).
- `PARTY_AND_TURNS.md`: rewards, temporary cache.
- `BESTIARY.md`: resources from monsters.
- `CITY_DESCRIPTION.md`: Artisans' Guild, storage.

---

## 1. Principles

1. **Nothing drops finished.** Monsters, chests and bosses give only resources, recipes and blanks. Every item in the game is made by an artisan.
2. **An NFT appears only at crafting.** Resources are counters, not NFTs. The program mints an item at the moment of crafting.
3. **An item's value is in its level and affixes**, not in a colored rarity. There are no normal, magic or rare items.
4. **Artisans are real players.** The NPC artisan exists only as a fallback for the start of the game.
5. **Everything a character carries is at risk.** On death the contents of the bag burn, while the storage in the city is kept.

---

## 2. Three kinds of things

| Kind            | Examples                                                               | How it is stored                                  | Transferable      |
| --------------- | ---------------------------------------------------------------------- | ------------------------------------------------- | ----------------- |
| **Resources**   | hides, wood, scrap metal, pollen, elemental stones, ingots, handles    | counters (id → amount) in the bag or storage PDA  | later via market  |
| **Items**       | weapons, armor, jewelry                                                | **Metaplex Core NFT** + state PDA                 | yes               |
| **Soulbound**   | class starter items, skills                                            | records in the character PDA                      | no                |

Recipes (scrolls) and consumables are stored as resources: counters by id.

---

## 3. Item

### 3.1 What it consists of

| Part                | Source                                      | What it defines                                                                                                       |
| ------------------- | ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| **Base**            | recipe                                      | item type (axe, staff, leather pants), equipment slot, base parameters                                                |
| **Materials**       | order resources                             | appearance (ramps, `ART_DESCRIPTION.md`) and base parameters: an iron axe and an onyx axe differ in damage and color |
| **Item level**      | artisan level and materials                 | the cap on affix tiers and the character level requirement (section 4)                                                |
| **Affixes**         | roll at crafting                            | up to 4 from crafting, up to 6 with sharpening and enchantment (section 5)                                            |
| **Aspect**          | materials (for example, a fairy's elemental stone) | the item becomes an aspect carrier for resonances                                                              |
| **Runes and sockets** | separate operations                       | their own additional affixes, which **do not combine** with the item's affixes                                        |

### 3.2 No colored rarity

There are no normal, magic, rare or unique items. Two items differ in level, number of affixes and their tiers. This makes every item describable by its substance, not by the color of its frame.

---

## 4. Item level

### 4.1 Where it comes from

Item level is determined at crafting:

- by the **artisan's profession level**;
- by the **materials**: more valuable resources raise the level.

### 4.2 What it limits

**The cap on affix tiers.** Tiers run from 14 (weak) to 1 (strongest). The lower the item level, the weaker the best available tier:

- on a level 1 item the best possible tier is 12;
- stronger tiers open up as item level grows (the next threshold is around level 4-5);
- the exact curve is set at balancing.

**The character requirement.** A character can equip an item if their character level is at least **item level minus 4**: a level 5 item can be worn by a level 1 character. As levels grow, the difference changes along a curve.

---

## 5. Affixes

### 5.1 Count

| Method                          | Affixes on the item                    |
| ------------------------------- | -------------------------------------- |
| Crafting from ordinary materials | up to 4                               |
| Sharpening, enchantment         | up to 6                                |
| Runes, sockets                  | their own affixes on top of this, separate |

### 5.2 Roll

- The set of possible affixes is defined by the item's **base**.
- Materials and artisan perks shift which affixes drop more often (an elemental stone pulls toward elemental affixes).
- Among the affixes are resistances to an aspect, a general state resistance and a resistance to one separate state. The last one adds to the general state resistance (`CHARACTER.md`, section 3).
- **The tier of each affix is rolled at crafting** via VRF, within the cap set by the item level.
- This is the only place in the game where tiers are random (`CHARACTER.md`, section 6.3).

### 5.3 Local and global affixes

| Kind         | What it affects                              | Example                                                                                         |
| ------------ | -------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| **Global**   | the character's stat as a whole              | +% fire damage of the character                                                                 |
| **Local**    | only the values of **this same item**        | +% speed, which multiplies only the +N speed already on this item                               |

Example: the pants had "+N speed" and 4 affixes. A speed enchantment added a fifth affix, "+% speed". This percentage multiplies the pants' **N**, not the character's total speed.

This way the affixes of one item reinforce each other but do not blow up the character's stats: a local percentage is always tied to a specific small base.

Therefore it is best for the affix text itself to say explicitly which of the two it is: a character-wide stat (such as the character's fire damage) or the item's own value (such as speed). Example: "+% item speed" (meaning the speed that this item provides).

### 5.4 Runes and sockets

- They give **their own** affixes and effects (rune glow, stones in sockets, `ART_DESCRIPTION.md`, section 9.4).
- They do not combine with the item's affixes: the item's local percentages do not apply to them.

---

## 6. Inventory and storage

### 6.1 The character's bag

What the character carries:

| What         | Limit                                                           |
| ------------ | --------------------------------------------------------------- |
| Items        | 20                                                              |
| Consumables  | 5 slots; identical consumables stack in one slot                |
| Resources    | 40 slots; one kind of resource per slot, up to 10 units per slot |
| Skills       | 50, a separate limit (`SKILL_BUILDER.md`, section 11)           |

Worn items sit in 11 equipment slots: MainHand, OffHand, Helmet, Body, Pants, Boots, Gloves, Necklace, Cloak and two ring slots (Ring1, Ring2).

There is no weight, only slots.

### 6.2 Storage in the city

- A chest in the city where **items, resources and consumables** can be stored.
- Storage is **shared by all of the player's characters** and survives the death of any of them.
- **There is no limit at the start.**
- **Skills cannot be put into storage**: they are always with the character.
- Moving things between the bag and storage is possible **only in the city**, not during a party.

### 6.3 How it works on-chain

- **Resources** in the bag are counters in the character's bag PDA, in storage they are counters in the storage PDA.
- **Items** are NFTs in the player's wallet. "An item in the bag" means that it is **frozen** by the program through the Metaplex Core plugin "Permanent Freeze Delegate": while it is in the bag, it cannot be sold or transferred. "An item in storage" is unfrozen and freely transferable.

### 6.4 Death

- **Everything that was in the bag** at the moment of death burns: items (the program burns them through the plugin "Permanent Burn Delegate"), resources, consumables.
- The character's soulbound things and skills burn.
- **Storage is kept.**
- The tombstone keeps the names and affixes of the burned items as a memorial.

---

## 7. Professions

### 7.1 How to get one

- The character completes the **first dungeon**, then meets an artisan in the **Artisans' Guild**, who offers to choose a first profession.
- **One profession per character.**
- The profession can be **changed**: the progress of the old profession (level, experience, mastery tree) is **reset**, and the new one starts from zero. The artisan's listings for the old profession are closed.

### 7.2 MVP professions

| Profession       | What it does                                          |
| ---------------- | ----------------------------------------------------- |
| **Blacksmith**   | metal processing (scrap metal → ingots), armor        |
| **Weaponsmith**  | weapon parts (handles) and weapons                    |

Future ones:

| Profession                                      | What it does                                         |
| ----------------------------------------------- | ---------------------------------------------------- |
| **Alchemist**                                   | potions, grenades                                    |
| **Hunter** (working name) or weaponsmith        | arrows, if they become consumables                   |
| **Dismantler**                                  | dismantling items into resources                     |
| **Armorer**                                     | heavy armor                                          |
| **Jeweler**                                     | jewelry, stones for sockets                          |
| **Skillsmith**                                  | lenses for skills (`SKILL_BUILDER.md`, section 9)    |

### 7.3 Profession progress

- The artisan gains **profession experience** for every item made.
- **Profession level** determines:
  - what level of items the artisan can make;
  - the **minimum** level of items the artisan produces: a high-level artisan cannot make items of the lowest levels. This way beginner artisans always have customers and can level up.
- **Mastery tree:** points for profession levels are invested in perks. Example of a weaponsmith perk: a chance of a higher tier of an elemental affix at crafting. Players choose artisans for the affixes they need.

### 7.4 Race and crafting

Race traits affect crafting (`CHARACTER.md`, section 7.2): for example, a Dwarf weaponsmith gets +1 tier on one affix for a chosen item type. Together with the mastery tree this gives value to high-level artisans of certain races.

### 7.5 Recipes

- **Basic recipes** are known to the profession from the start.
- **Recipe scrolls** (from bosses) are **consumed** for one craft. An artisan's value is in their level and tree, not in a collection of recipes.

---

## 8. Artisans' Guild and orders

### 8.1 Artisans' Guild

A building in the city (`CITY_DESCRIPTION.md`):

- player artisans **post listings** for their services;
- the **NPC artisan** also works here (section 8.4).

### 8.2 Artisan listing

A listing is an **on-chain record** that works without the artisan's participation, like a smart contract function:

| Field                  | Meaning                                                                                                   |
| ---------------------- | --------------------------------------------------------------------------------------------------------- |
| Artisan                | the character and their profession                                                                        |
| **Stat snapshot**      | profession level, mastery tree perks and race traits **at the moment the listing is created**            |
| Recipes                | which basic recipes the artisan offers                                                                    |
| Price                  | in SOL per craft, from 0                                                                                  |

- Crafting uses the **snapshot**, not the artisan's current state. To apply new levels and perks, the artisan **updates the listing**.
- The artisan can **withdraw** the listing at any moment.
- The listing closes automatically if the artisan's character **died** or **changed profession**.
- A high-level artisan with valuable perks and a suitable race can set a higher price. The project takes nothing from this price.

### 8.3 Crafting

The artisan **does not need to accept anything**. Any player goes to a listing and starts the craft themselves:

1. The player chooses a listing and a recipe, and deposits the resources and the artisan's price.
2. One transaction:
   - the player's resources are burned;
   - VRF rolls the affixes and their tiers, taking into account the artisan's **snapshot**;
   - the program mints the item NFT to the **player**;
   - the price is transferred to the artisan;
   - the artisan receives **profession experience**.
3. The artisan can be offline: their listing works by itself.

Profession experience from crafts via a listing is credited to the artisan's character, but the **snapshot** in the listing does not change until the artisan updates it.

### 8.4 NPC artisan

Needed so the game works while there are few player artisans:

- crafts **weapons and equipment**, no jewelry;
- is **not limited** by item level;
- the item gets **no more than 3 affixes**;
- such an item **cannot be upgraded**: sharpening, enchantment, runes and sockets do not apply to it;
- no price, no perks.

---

## 9. Axe chain (MVP)

Demonstrates both professions and the link to the bestiary:

```
treant wood  ──(weaponsmith)──>  handle
goblin scrap metal  ──(blacksmith)──>  iron ingot
handle + iron ingot  ──(weaponsmith)──>  axe
```

Intermediate products (handle, ingot) are **resources**, not NFTs. An NFT appears only at the last step.

---

## 10. Starter soulbound items

| Class         | Weapon | Armor                |
| ------------- | ------ | -------------------- |
| **Warrior**   | sword  | leather armor        |
| **Mage**      | staff  | robe                 |
| **Support**   | wand   | light leather armor  |

Starter weapon aspects: the sword is Piercing, the staff is Blunt, the wand is Lightning, each at `aspect_tier` 14.

- Stored in the character PDA, not NFTs.
- Cannot be sold, transferred, put into storage or dismantled.
- Do not take up bag slots.
- Disappear when the character dies.
