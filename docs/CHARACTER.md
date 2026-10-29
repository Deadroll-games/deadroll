# CHARACTER: the Deadroll character

Version 0.1, draft. This document is about the character: stats, derived stats, level and experience, races, classes and their traits, character creation. **There are no numbers in this document**, they will appear at balancing.

Related documents:

- `DAMAGE_TYPES_AND_MIXES.md`: aspects, resonances, reactions. The **rules** of how predispositions work are there; **which** predispositions races and classes have is here.
- `COMBAT_MATH.md`: how stats take part in the hit calculation.
- `ART_DESCRIPTION.md`: how the character is drawn.

---

## 1. Principles

1. **The character lives for years.** It is an NFT passport + a PDA with state. It moves from party to party, grows, takes injuries and can die permanently.
2. **Race and class set inclinations, not a cage.** Any character can learn any skill. Race and class give traits that make some paths stronger than others.
3. **The choice at creation is permanent.** Race, class and the chosen predisposition do not change. Otherwise the choice would be meaningless.
4. **Growth is deterministic.** Race and class traits grow through leveling up a skill branch and through special items, not through chance. Randomness exists only in items, at crafting.
5. **Numbers are whole and positive only** (`COMBAT_MATH.md`, sections 1-3).

---

## 2. Core stats

Four stats in the first version. The player distributes them at creation; they grow with level.

| Stat             | What it strengthens                                                                                  |
| ---------------- | ---------------------------------------------------------------------------------------------------- |
| **Strength**     | martial strike damage; chance to apply states and debuffs with martial skills                        |
| **Dexterity**    | accuracy, evasion, martial projectile damage, **speed**; chance to apply tempo effects (Haste, Slow) |
| **Intelligence** | magical damage; chance to apply states and debuffs with magical skills; **initiative**               |
| **Endurance**    | HP, state resistance, duration of own buffs on self                                                  |

The exact contribution of each stat is set at balancing.

There is no carried weight: the character carries items by slots (`ITEMS_AND_CRAFT.md`, section 6.1). Strength does not limit what can be worn or carried.

### 2.1 Which stat scales a state

The aspect decides which state. The power source decides which stat scales it.

Every skill has a power source tag, magical or martial (`SKILL_BUILDER.md`, section 2). The chance to apply a state or a debuff grows with the stat of that source, whatever the aspect:

| What                                                              | Scaled by    |
| ----------------------------------------------------------------- | ------------ |
| States and debuffs of a martial skill, any aspect                 | Strength     |
| States and debuffs of a magical skill, any aspect                 | Intelligence |
| Tempo effects: Haste, Slow and similar future effects, any source | Dexterity    |
| Duration of own buffs on self                                     | Endurance    |

Examples:

- a blood mage (magical, Blood) makes Bleeding more likely through Intelligence;
- a warrior with a fire blade (martial, Fire) sets targets on fire through Strength;
- a Cold archer (martial, Cold) chills through Strength; a Cold mage chills through Intelligence.

The basic attack uses the power source of the character's class (section 8.1).

Rules:

- The rule covers every chance roll of a state: the native state of each packet, states from resonances, and the effect of a skill.
- The stat bonus belongs to the Increase group of the chance (`COMBAT_MATH.md`, section 4). The target's state resistance is its counterweight, and the result stays within the chance limits (`COMBAT_MATH.md`, section 10).
- Any combination of aspect and source is playable. Race and class set inclinations; the source sets which stat to invest in.

---

## 3. Derived stats

| Stat                   | What it does                                                                             | Sources                                                                     |
| ---------------------- | ---------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| **HP**                 | health pool; 0 = death                                                                   | class base, Endurance, level, items                                         |
| **Armor**              | reduces physical damage with diminishing returns                                         | items, class, traits                                                        |
| **Evasion**            | lowers the chance to hit the character; the threshold of the hit roll                    | Dexterity, items, traits                                                    |
| **Accuracy**           | raises the chance to hit; the bonus added to the d20 of the hit roll                     | Dexterity, items, traits                                                    |
| **Resistances**        | per aspect, physical ones included (Piercing, Blood, Blunt); lowers damage of the aspect | items, Resistance effect                                                    |
| **State resistance**   | lowers the chance of states being applied                                                | Endurance, items                                                            |
| **Resistance to a state** | lowers the chance of one state; adds to the general state resistance                  | items                                                                       |
| **State chance**       | raises the chance to apply states and debuffs (section 2.1)                              | Strength or Intelligence by the skill's source, Dexterity for tempo effects |
| **Speed**              | how many action points the character gets per turn                                       | race, class, Dexterity, items, effects (section 3.1)                        |
| **Action Points (AP)** | spent on steps, skills, the basic attack, interactions (`PARTY_AND_TURNS.md`, section 5) | computed from speed                                                         |
| **Initiative**         | how often the character's turn comes in combat (`PARTY_AND_TURNS.md`, section 6)         | Intelligence, items, effects (for example, Cold Mind)                       |
| **Crit chance**        | in addition to a natural 20 on the d20                                                   | items, traits, skills                                                       |

Resistances exist for every aspect, physical ones included. Physical damage is cut by **armor** first and then by the resistance to its aspect (`COMBAT_MATH.md`, section 6.5). Race gives a resistance only through the passive Resistance skill, which adds its own predisposition (`SKILL_BUILDER.md`, section 6.2). Resistances to future families (mental, sacred) will be added together with them.

Accuracy and evasion are an opposing pair (`COMBAT_MATH.md`, section 3). Level and the damage stats (Strength, Intelligence) do not add accuracy.

Crit chance comes only from the listed sources. No core stat gives crit chance; without them, only a natural 20 crits.

### 3.1 Speed

Speed is built up in steps, in this order:

1. **Race** sets the base speed: elves are faster than humans, humans are faster than dwarves.
2. **Class** adjusts it: a warrior is slower than an archer, but faster than a mage. The mage makes up for this with initiative.
3. **Dexterity** gives an increase.
4. **Items:**
   - heavy armor (for example, an iron suit of armor) gives a **counterweight** to speed;
   - light trousers and footwear can give an increase; not every item affects speed, for example a shirt does not.
5. **Effects** (haste, slow, reactions) are applied last.

Counterweights divide rather than subtract (`COMBAT_MATH.md`, section 3): speed never drops to zero. A full stop is possible only through hard control (freeze, stun).

### 3.2 Speed and initiative

|                | Gives                                               |
| -------------- | --------------------------------------------------- |
| **Speed**      | how far to go and what to get done in a single turn |
| **Initiative** | how often the turn comes                            |

A fast character covers more ground per turn; a high-initiative character takes turns more often. In combat there is **no more than one active skill per turn**, so speed gives mobility, not double damage.

---

## 4. HP and combat length

HP is made up of several sources: class base, Endurance, gain per level, items.

The HP scale is derived not directly but from the **target combat length** (`COMBAT_MATH.md`):

- combat length is measured in **party rounds**, not in the hits of a single character: a party of three makes three hits per round;
- an ordinary enemy of its own level should last several party rounds and be able to respond;
- as level grows, **HP grows slightly faster than damage**: combats gradually get longer, and players feel growth through new skills, items and combinations;
- **a short combat is an event**: a crit, a resonance, allies' abilities stacking.

The simulator picks the specific curve.

---

## 5. Level and experience

- A character starts at **level 1**.
- In the first version there are levels 1-4: this is enough for two map tiers (1-2 and 3-4).
- **Experience is given for every killed monster** (full to each living character). **Planned:** for every won arena, in full to each living character, and nothing after a retreat (`PARTY_AND_TURNS.md`, sections 4.1 and 7.5). It **accumulates in the temporary cache** and is added to the character only on victory over the boss or on a peaceful exit (`PARTY_AND_TURNS.md`, section 7).
- On **escape**, the accumulated experience is lost.
- After a **peaceful exit**, a penalty to the experience received applies to the next dungeon.
- A character whose level is noticeably higher than the map tier gets **less experience or none**, and rewards are not rolled for them.
- Per level the character gets: stat points, HP gain, points for the skill branch (future).

---

## 6. Race and class traits

### 6.1 What a trait is

A trait is a permanent bonus from a race or class. Traits come in different kinds; damage is only one of them.

| Trait kind              | How it works                                          | Example                                                          |
| ----------------------- | ----------------------------------------------------- | ---------------------------------------------------------------- |
| **Added aspect damage** | a flat damage packet of its own aspect on every hit   | human: Piercing 1-2                                              |
| **Tag-based increase**  | a percentage to damage or effects with a matching tag | mage: % to magical damage                                        |
| **Defensive**           | a bonus to armor, resistance, HP                      |                                                                  |
| **Utility**             | a chance or radius outside combat                     | scout: chance to find a trap or hidden stash within a radius     |
| **Crafting**            | affects the crafting result                           | dwarf: +1 tier to one affix when crafting items of a chosen type |

### 6.2 Predisposition

If a trait is tied to an aspect (such as "added Piercing damage"), that aspect becomes the character's **predisposition**:

- it takes part in resonances as a **tint** (a d100 roll on each matching skill);
- it takes part in reactions, when other people's effects land on the character.

**The added damage of the predisposition always works**, on every hit. The resonance on top of it triggers on a roll.

### 6.3 Trait tiers

Race and class traits have a tier (from 14 to 1, `DAMAGE_TYPES_AND_MIXES.md`, section 4). It:

- starts low;
- grows **deterministically**: leveling up a skill branch, special items;
- is never random, so recreating a character for a lucky tier is pointless.

---

## 7. Races

### 7.1 First version

| Race         | Choice at creation                            | Starting trait                                                               |
| ------------ | --------------------------------------------- | ---------------------------------------------------------------------------- |
| **Human**    | one physical aspect: Piercing, Blunt or Blood | added damage of the chosen aspect, a small flat packet; predisposition to it |
| **Wood Elf** | one elemental aspect: Fire, Cold or Lightning | added damage of the chosen aspect, a small flat packet; predisposition to it |

Race **does not limit** the choice of skills. It affects play through the predisposition: added damage on every hit, a tint in resonances, and reactions to other people's effects. A human Fire mage with a Piercing predisposition plays with Fire spells, and the race colors them with the "Stinging Flame" resonance.

### 7.2 Future

| Race         | Idea                                                                                                     |
| ------------ | -------------------------------------------------------------------------------------------------------- |
| **Dark Elf** | predisposition to Blood and curses                                                                       |
| **Dwarf**    | crafting trait: an item of a chosen type (weapon, armor or other) gets +1 tier to one affix when crafted |

New races are unlocked through lore, not through patch notes.

---

## 8. Classes

### 8.1 First version

| Class       | Tag-based increase                         | Other traits                           | Starting skills                                                         |
| ----------- | ------------------------------------------ | -------------------------------------- | ----------------------------------------------------------------------- |
| **Warrior** | % to martial damage                        | high base HP and armor                 | martial only                                                            |
| **Mage**    | % to magical damage                        |                                        | magical only (strike and projectile)                                    |
| **Support** | % to the power of buffs, debuffs and auras | summoning creatures that auras empower | only with the purpose buff or debuff, including auras; any power source |

In v1 a class has no aspect of its own: the carriers of tints are the race and the weapon. Classes have no healing for now: Support is a buffer and debuffer by inclination. Healing will come with a separate Healer class, opened later through lore.

### 8.2 Future

| Class      | Idea                                                                                              |
| ---------- | ------------------------------------------------------------------------------------------------- |
| **Scout**  | utility traits: chance and radius of detecting traps and hidden stashes, grow with passive skills |
| **Healer** | the first class unlocked through lore                                                             |

---

## 9. Character creation

1. Choose a race.
2. Choose a predisposition within the race (human: physical aspect; elf: elemental).
3. Choose a class.
4. Distribute stat points.
5. Receive the class's starter soulbound items (granted by `create_character`, kept in the character PDA).
6. Mint the character NFT and create the PDA.

The choices in steps 1-3 are permanent.

---

## 10. What is stored in the character's PDA

No sizes, only the composition:

| Group      | Fields                                                               |
| ---------- | -------------------------------------------------------------------- |
| Identity   | owner, NFT, name, race, class, chosen predisposition                 |
| Stats      | 4 core stats, unspent points                                         |
| Progress   | level, experience                                                    |
| Traits     | tiers of race and class traits                                       |
| Equipment  | worn items by slot (`ART_DESCRIPTION.md`, section 6.3); equipment and soulbound items live in the character PDA |
| Inventory  | starter soulbound items (granted in `create_character`), stackable resources |
| Appearance | layer sprite ids, race skin ramp                                     |
| State      | current HP, injuries (for example, a lost arm), alive or tombstone   |
| Rendering  | `state_version` (`STORAGE_AND_DELIVERY.md`)                          |

Derived stats (section 3) are not stored but computed from stats, traits and equipment.

---

## 11. Death

- HP = 0 in combat: the character dies permanently.
- The character's NFT turns into a **tombstone**: name, level, history.
- Item NFTs are burned; the tombstone keeps their names and affixes as a memorial.
- The bag and the starter soulbound items burn together with the character.
- **The mechanics of the character's skills are erased** together with it, including the lenses placed in them; the skill names and descriptions stay on the tombstone (`SKILL_BUILDER.md`, section 11).

Future: the Healer class and resurrection relics (once).
