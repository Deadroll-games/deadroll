# BESTIARY: the Deadroll bestiary

Version 0.1, draft. A document about monsters: what a monster template consists of, monster skills, levels, elite versions, summoning, the starter set of monsters and bosses, body loot. **There are no numbers in this document**: stats are given as relative levels (low, medium, high), and the numbers will come at balancing.

Related documents:

- `DAMAGE_TYPES_AND_MIXES.md`: aspects, states, resonances, reactions, collisions, defenses.
- `SKILL_BUILDER.md`: the skill builder; monster skills are assembled with a similar builder.
- `MAP_DESCRIPTION.md`: monster placement on the map, map tier limits, budget.
- `PARTY_AND_TURNS.md`: combat, initiative timeline, rewards, temporary cache.

---

## 1. Principles

1. **A monster is a template.** The Bestiary defines what a monster can be; when placing monsters, the DM chooses the level, the skills from the pool and, if desired, an elite version.
2. **One combat engine for everyone.** Monsters use the same stats, damage packets, resonances, fusions, collisions and reactions as characters.
3. **Defenses come from physics.** A monster's resistances and traits are explained by what it is made of and where it lives.
4. **Something useful from every monster.** Body loot is resources of its biome or of the monster itself, needed for crafting.
5. **Monsters are controlled by the DM** through their session key (`GAME_COMMISSIONS.md`, section 7).

---

## 2. Monster template

| Block                     | What it defines                                                                                         |
| ------------------------- | ------------------------------------------------------------------------------------------------------- |
| **Role**                  | Swarm, Fast, Tank, Loner, Boss; determines how the monster enters a pack (section 2.1)                 |
| **Biome**                 | where it comes from: Forest, Caves and Ruins; flavor only, it does not tie the monster to a map biome (section 11) |
| **Size**                  | base cells: regular monsters 1×1, large monsters and bosses 2×2                                         |
| **Minimum map tier**      | from which map tier it can appear; there is no maximum (section 4)                                      |
| **Stats**                 | HP, armor, evasion, accuracy, speed, initiative: as relative levels                                     |
| **Natural attack**        | like a character's basic attack: damage range, aspect, power source, form                               |
| **Predisposition**        | an optional aspect: then the monster takes part in **reactions** and **resonances** like a character    |
| **Defenses by aspect**    | normal, resistance or immunity (section 10)                                                             |
| **Kin**                   | the kin the monster belongs to (Wolf, Goblin and so on); a skill marked kin-only heals and buffs only its own kin |
| **State immunities**      | for example, insensitivity to Blood magic or to Freeze                                                  |
| **Traits**                | special permanent rules: swarm, regeneration, hunt                                                      |
| **Skill pool**            | the skills the DM chooses from (section 3)                                                              |
| **Summon**                | bosses only: whom it can summon (section 6)                                                             |
| **Loot**                  | XP for combat, body resources (section 9)                                                               |

### 2.1 Roles and packs

Monsters stand on the map in **packs**, as in Divinity: Original Sin. A pack is a group of monsters that **enter combat together**. A pack may consist of a single monster.

| Role       | How many in one pack                                       | Example                         |
| ---------- | ---------------------------------------------------------- | ------------------------------- |
| **Swarm**  | several; swarm traits work inside the pack                 | Goblins                         |
| **Fast**   | several                                                    | Wolves                          |
| **Tank**   | several, within the map tier limits                        | Troll, Treant, Bear             |
| **Loner**  | **no more than one** in a pack                             | Wood Fairy                      |
| **Boss**   | its own boss pack; summoned helpers join it                | Goblin Chieftain, Wood Fairy King |

Pack rules:

- A pack can mix monsters of **different biomes and roles**: a Treant, a Wood Fairy and several Goblins make a normal pack.
- The size of a pack and the number of packs on a map are limited by the map tier (`MAP_DESCRIPTION.md`, section 10.2).
- How a pack enters combat is described in `PARTY_AND_TURNS.md`, section 4.

**Planned (arena update, `PARTY_AND_TURNS.md`, section 4.1):**

- On the map only one monster of a pack is visible, picked by the DM; the rest of the pack is hidden until the fight.
- The site shows the pack's threat level, declared openly by the DM and checked by the program when the pack is revealed.
- A pack has a facing set by the DM: coming from its back, the party moves first; from the front, the monsters do.
- Outside combat monsters stand still. A fight is one pack in its own arena.
- After a retreat the pack heals fully, stays on guard and gets stronger (`PARTY_AND_TURNS.md`, section 7.5).

---

## 3. Monster skills

### 3.1 Monster builder

Monster skills are assembled by a **builder similar to the player one**, but oriented to monsters:

- the same blocks: category, purpose, tags, aspects, effect, target;
- the same tag compatibility rules;
- **no class or race restrictions**;
- there are blocks that players do not have: **summon** (bosses only), swarm traits;
- skills are created by the **game team**, not by the DM. The DM only chooses from the pool.

### 3.2 Slots

| Type                | Active | Passive | Reactive | Special                    |
| ------------------- | ------ | ------- | -------- | -------------------------- |
| **Regular monster** | 3      | 1       |          |                            |
| **Elite monster**   | 4      | 1       |          | section 5                  |
| **Boss**            | 5      | 1       | 1        | built-in summon (section 6) |

The template's pool is larger than the number of slots, so that the DM has a choice and the same monster behaves differently on different maps.

---

## 4. Levels

- A template has a **minimum map tier** of the maps on which it can appear. For all MVP monsters this is the **first map tier**; the field is needed for future monsters that appear only on high-tier maps.
- **There is no maximum map tier.** A Wolf can also stand on a map of tier 5-6.
- The DM chooses the monster's level **within the map tier**.
- A monster's stats and skill power are **fitted to its level** by the same curve as for characters.

---

## 5. Elite versions

- The DM can make a regular monster **elite** for map budget points (`MAP_DESCRIPTION.md`, section 10).
- An elite monster gets:
  - **+50% HP** (`elite_hp_bp`);
  - **one additional active skill** from the pool (4 instead of 3).
- Elite versions give the DM a tool for making a unique encounter without new templates.
- The elite monster limit per map is set by the map tier: tier 1 allows one elite, tier 2 allows two (placeholders in `crates/mapcheck/src/limits.rs`).

---

## 6. Boss summoning

Instead of phases, bosses have a **built-in summon** of helpers.

**Later:** a boss calling allies during combat becomes a reactive skill with a limit of uses, for some bosses only, in a separate skills update.

- **At the start of combat** the boss appears at once with **two helpers**.
- **Once per combat** the boss can summon **two more**, regardless of whether the first ones are alive.
- If the cells next to the boss are occupied, helpers appear on the **nearest free passable** cells.
- Summoning during combat is a separate boss skill, in addition to the 5 active ones.
- Summoned helpers are **scaled-down versions** of templates from the Bestiary, with their own roles.
- Summoned helpers take part in the initiative timeline like regular monsters, and the DM controls them.
- Summoned helpers give **no body loot**, so that summoning cannot be farmed.

---

## 7. Starter monster set

### 7.1 Overview

| Monster           | Role                              | Biome           | Size | Natural attack                                                           |
| ----------------- | --------------------------------- | --------------- | ---- | ------------------------------------------------------------------------ |
| **Wolf**          | Fast                              | Forest          | 1×1  | bite: martial, strike, Piercing                                          |
| **Bear**          | Tank                              | Forest          | 2×2  | paw swipe: martial, strike, Blunt                                        |
| **Treant**        | Tank                              | Forest          | 2×2  | branch strike: martial, strike, Blunt                                    |
| **Wood Fairy**    | Loner (no more than one in a pack) | Forest         | 1×1  | elemental orb: magical, projectile, one elemental aspect                 |
| **Goblin**        | Swarm                             | Caves and Ruins | 1×1  | rusty blade strike: martial, strike, Piercing                            |
| **Troll**         | Tank                              | Caves and Ruins | 2×2  | club strike: martial, strike, Blunt                                      |

### 7.2 Wolf

| Block                | Value                                                                                                          |
| -------------------- | -------------------------------------------------------------------------------------------------------------- |
| Stats                | speed high, initiative medium, HP medium, armor low                                                            |
| Defenses             | Cold resistance (thick fur)                                                                                    |
| Trait                | **Hunt**: more damage to a target with less than half HP                                                       |
| Skill pool (ideas)   | lunge at a target, howl (Haste for nearby allied Wolves), hamstring (Slow), ragged bite (Bleeding)             |

### 7.3 Bear

| Block                | Value                                                                                                                                      |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Stats                | HP high, damage high, speed medium, initiative low                                                                                         |
| Defenses             | Cold resistance (fur and fat)                                                                                                              |
| Trait                | **Rage**: damage grows when the Bear's HP drops below half                                                                                 |
| Skill pool (ideas)   | crushing paw swipe (Stun), roar (Weakening of nearby enemies), lunge, bear hug (hold: the target cannot leave its cell)                    |

### 7.4 Treant

| Block                | Value                                                                                                                         |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Stats                | HP very high, damage high, speed low, initiative low                                                                          |
| Defenses             | Piercing and Blunt resistance (dense wood)                                                                                    |
| State immunity       | **Freeze**: dry wood has no water that could freeze                                                                           |
| Trait                | **Dry Wood**: Burning on a Treant lasts longer, but it cannot be frozen                                                       |
| Skill pool (ideas)   | branch strike along a line, roots (Slow on the target), bark shedding (Protection on self), log throw (projectile, Blunt)     |

Consequences of Freeze immunity: Cold damage to a Treant goes through, but resonances with the condition "the target can be frozen" (for example, Piercing Frost) do not trigger on it, and collisions involving Freeze (Thaw, Shatter, Ice Discharge) do not occur. Fire works against a Treant, not Cold.

### 7.5 Wood Fairy

| Block                | Value                                                                                                                        |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Stats                | initiative high, evasion high, HP low, magical damage medium                                                                 |
| Predisposition       | one elemental aspect: Fire, Cold or Lightning (the DM chooses when placing)                                                  |
| Defenses             | resistance to its own elemental aspect                                                                                       |
| Trait                | **Fluttering**: after an attack it can fly off one cell for free                                                             |
| Skill pool (ideas)   | elemental bolt, blinding pollen (Accuracy reduction), elemental mark (Vulnerability to its own aspect), Haste on self        |

### 7.6 Goblin

| Block                | Value                                                                                                                                        |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Stats                | HP low, initiative medium, damage low                                                                                                        |
| Defenses             | no special defenses                                                                                                                          |
| Trait                | **Swarm**: accuracy bonus if another Goblin is nearby                                                                                        |
| Skill pool (ideas)   | dirty strike (Weakening), thrown stone (projectile, Blunt), sneaky stab (crit against a target attacked by another Goblin), retreat         |

### 7.7 Troll

| Block                | Value                                                                                                            |
| -------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Stats                | HP very high, speed low, damage high                                                                             |
| Defenses             | Blunt and Piercing resistance (thick hide)                                                                       |
| Trait                | **Regeneration**: restores HP every turn; **Fire stops regeneration** while the Troll is under Burning           |
| Skill pool (ideas)   | crushing club strike (Stun), rock throw, roar (Weakening), ground slam (Slow on nearby enemies)                  |

---

## 8. Bosses

### 8.1 Goblin Chieftain

| Block                | Value                                                                                                                                              |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Biome                | Caves and Ruins                                                                                                                                    |
| Size                 | 2×2                                                                                                                                                |
| Attack               | heavy blade strike: martial, strike, Piercing                                                                                                      |
| Defenses             | Piercing resistance (trophy armor)                                                                                                                 |
| Trait (passive)      | **Leader**: nearby Goblins get a damage bonus                                                                                                      |
| Reactive             | when HP falls below a threshold: **Battle Cry**, a temporary Protection aura on self and nearby Goblins                                            |
| Summon               | **Goblin Warrior** (melee) and **Goblin Shaman** (support: heals and hastes Goblins)                                                               |
| Skill pool (ideas)   | shield cleave (target Armor reduction), net throw (Slow), spin strike (line), command (an allied Goblin acts earlier), dagger throw                |

### 8.2 Wood Fairy King

| Block                | Value                                                                                                                                                         |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Biome                | Forest                                                                                                                                                        |
| Size                 | 1×1 (large, but hovers)                                                                                                                                       |
| Attack               | elemental orb: magical, projectile                                                                                                                            |
| Predisposition       | **two elemental aspects** (the DM chooses the pair when placing): his skills produce fusions, and his attacks produce resonances                              |
| Defenses             | resistance to both of his aspects                                                                                                                             |
| Trait (passive)      | **Forest Crown**: nearby Wood Fairies get an Initiative bonus                                                                                                 |
| Reactive             | when hit by a crit: **Flash**, knocks back everyone nearby and removes debuffs from himself                                                                   |
| Summon               | **two Wood Fairies**: one attacker, one support (hastes and protects the King)                                                                                |
| Skill pool (ideas)   | dual-elemental bolt (fusion of two aspects), elemental storm along a line, blinding light, forest roots (Slow), King's mark (Vulnerability to his aspects)    |

---

## 9. Loot

### 9.1 Rules

- **Experience** goes into the temporary cache (`PARTY_AND_TURNS.md`, section 8): now for each killed monster, planned for each won arena.
- **Body resources** from each killed monster are rolled **separately for each** living participant, like the other rewards, and go into the temporary cache.
- The **level of body resources** always equals the **monster's level**.
- Resources correspond to the monster's **biome** or to the **monster itself**. For now body resources follow the table of section 9.2.
- Summoned helpers give no loot.
- There are no finished items: everything goes to crafting with artisans.

### 9.2 Resources of regular monsters (draft)

| Monster            | Resource                                    | Used for                                                          |
| ------------------ | ------------------------------------------- | ----------------------------------------------------------------- |
| **Wolf**           | wolf pelt                                   | light armor, pants, footwear                                      |
|                    | fangs                                       | piercing inserts, arrowheads                                      |
|                    | sinews                                      | bowstrings                                                        |
| **Bear**           | bear pelt                                   | warm armor (Cold resistance)                                      |
|                    | claws                                       | piercing inserts, weapon decorations                              |
|                    | bear fat                                    | reagent, leather impregnation                                     |
| **Treant**         | Treant wood                                 | grips for clubs and axes, bows, shafts, staves                    |
|                    | bark                                        | light shields, light armor                                        |
|                    | resin                                       | crafting glue, flammable reagent                                  |
|                    | Treant heartwood (rare)                     | base for nature staves                                            |
| **Wood Fairy**     | magic pollen                                | wands and staves, enchantments                                    |
|                    | **fairy elemental stone** (the fairy's aspect) | items with elemental affixes; it is what gives fairies their magic |
|                    | fairy wings (rare)                          | light magical fabrics                                             |
| **Goblin**         | scrap metal                                 | smelting into iron                                                |
|                    | scraps of cloth and leather                 | simple light armor                                                |
|                    | goblin trinkets                             | simple stones for accents                                         |
| **Troll**          | thick hide                                  | heavy leather armor                                               |
|                    | Troll bone                                  | grips and pommels of heavy weapons                                |
|                    | Troll blood (rare)                          | enchantment reagent (future regeneration)                         |

### 9.3 Boss loot

Bosses must be valuable. From a boss, in addition to the party's regular reward (`PARTY_AND_TURNS.md`, section 9), the following are rolled:

| Boss                     | Special loot                                                                                                                                                                              |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Goblin Chieftain**     | weapon and armor **recipe scrolls**, a large ingot of trophy metal, a goblin banner (material for future auras)                                                                           |
| **Wood Fairy King**      | magical weapon **recipe scrolls**, a **large dual-elemental stone** (for items with two elemental affixes), enchantment reagents, a chance of an **uncut skill**                          |

---

## 10. Defenses

- Defense levels by aspect: **normal, resistance, immunity** (`DAMAGE_TYPES_AND_MIXES.md`, section 9).
- Resistances, immunities and all combat numbers of monsters are taken from `assets/registry/bestiary.json` (the registry). The scale is tuned for players (patch 6). The Goblin Shaman has no resistances, and its attack is Lightning.
- Immunities to states and to specific kinds of magic are set separately (for example, future ghosts are insensitive to Blood magic and curses).
- Immunity to Freeze also protects from freezing by accumulated Chill.
- Traits such as "dry wood" or "regeneration that fire stops" are **trait rules**, not defense levels.

---

## 11. Biomes

| Biome                | Monsters                                 | Bosses          |
| -------------------- | ---------------------------------------- | --------------- |
| **Forest**           | Wolf, Bear, Treant, Wood Fairy           | Wood Fairy King |
| **Caves and Ruins**  | Goblin, Troll                            | Goblin Chieftain |

Monsters are **not tied to biomes**. The map biome (`MAP_DESCRIPTION.md`, section 4.4) is picture only, and any monster may stand on any map. The table above is the monster's origin, for flavor and loot.

---

## 12. On-chain storage

- The **Bestiary** is an account (or a set of accounts) with template records: role, biome, size, minimum map tier, stats, attack, predisposition, defenses, traits, skill pool, summon, loot table.
- A **spawn** on the map (`MAP_DESCRIPTION.md`, section 7.2): template id, position, level, chosen skills (pool bitmask), elite flag, chosen aspects (for Wood Fairies and the Wood Fairy King).
- The monster's **live state** in combat is stored in `GameSession`.
- A new monster is added as a Bestiary record, without a program update, unless it introduces a new kind of mechanic.

---

## 13. Future

| What                                 | Where                                                            |
| ------------------------------------ | ---------------------------------------------------------------- |
| Undead, ghosts                       | a separate update with its own map and a new character class     |
| Other biomes                         | new monsters and resources                                       |
| Resistances to future families       | together with the mental and sacred families                     |
