# SKILL_BUILDER: the Deadroll skill builder

Version 0.1, draft. A document about skills: what they consist of, which limits and trade-offs apply when creating them, how fusions, slots, cooldowns and growth through lenses work. **There are no numbers in this document**, they come at balancing.

Related documents:

- `DAMAGE_TYPES_AND_MIXES.md`: aspects, tags, states, resonances, reactions.
- `COMBAT_MATH.md`: damage packets, modifier groups, the order of the hit calculation.
- `CHARACTER.md`: character, race, class, traits.

---

## 1. Principles

1. **The player creates skills on their own** from blocks: category, tags, aspects, effect, target. The player writes the name and description in their own words.
2. **Mechanics come only from blocks.** Text is identity and lore, it affects nothing.
3. **Every advantage costs something.** More aspects, more targets, a stronger effect: all of them share one skill power or require giving up something else.
4. **The program checks the rules.** The builder on the website and the program use one validation crate (`deadroll-skillcheck`), so the limits cannot be bypassed by going around the website.
5. **A skill is a soulbound item of the character.** It cannot be sold or transferred, and it dies together with the character.

---

## 2. What a skill consists of

| Block                 | What the player chooses                                             | Required                                 |
| --------------------- | ------------------------------------------------------------------- | ---------------------------------------- |
| **Category**          | active, reactive, passive                                           | yes                                      |
| **Purpose**           | damage, buff (on self or an ally), debuff (on an enemy)             | yes                                      |
| **Tags**              | power source (magical / martial), form (strike / projectile), range | yes                                      |
| **Aspects**           | from 1 to 3, by the rules of section 4                              | yes                                      |
| **Effect**            | one effect kind from section 6                                      | depends on purpose and number of aspects |
| **Target**            | single target, line; **area later**                                 | yes                                      |
| **Trigger condition** | only for reactive skills (section 3.2)                              | for reactive skills                      |
| **Story**             | name and description                                                | yes                                      |

Mechanics come **only** from blocks, text is identity and lore, as in the brief.

Tag compatibility rules from `DAMAGE_TYPES_AND_MIXES.md`, section 2.1: magical ✗ martial, strike ✗ projectile.

**Line target.** A line goes from the caster through the chosen target up to the range of the skill. A wall stops it. It hits the units of the skill's side (enemies for damage and debuffs, allies for buffs). Every unit gets its own roll, all on the same VRF value, in the order along the line. In melee a line reaches 1 cell.

**Range.** A staff and a wand attack at range (projectile); a sword and a fist attack in melee (strike). A monster's range comes from the form of its attack.

## 3. Categories and slots

| Category     | Slots per map | How it works                                                       |
| ------------ | ------------- | ------------------------------------------------------------------ |
| **Active**   | 4             | used on the character's own turn, spends an action, has a cooldown |
| **Reactive** | 1             | triggers by itself when its condition is met                       |
| **Passive**  | 1             | works constantly                                                   |

### 3.1 Basic attack

Every weapon has a **basic attack**: a sword strike, a bow shot, a wand shot with a bolt of energy, an unarmed punch.

- does not take a skill slot;
- **has no cooldown**;
- deals damage of the weapon's own aspect without conversion (section 5);
- **is not strengthened by passive skills**;
- in one turn the character uses **either a basic attack or an active skill**, not both.

So a mage can use a spell on any turn (if it is not on cooldown) or simply shoot from the wand.

### 3.2 Reactive skills

This is a passive ability with a trigger condition. Examples of conditions:

- the character was hit;
- the character was hit by a crit;
- the character's HP dropped below a threshold;
- a nearby ally dropped below an HP threshold;
- the character was frozen or stunned;
- an enemy entered an adjacent cell.

A reactive skill also has a cooldown, so that it does not trigger every turn. A reactive skill can be a **temporary aura** (section 3.5).

How a reaction works:

- A reaction is not an action: it spends no AP, and it works under Freeze and Stun too. It has its own cooldown.
- The target of damage and debuffs is the enemy that caused the trigger.
- A buff goes on the character themselves; for the condition "a nearby ally dropped below an HP threshold" it goes on that ally.
- A counterattack checks the range of the skill.
- A monster that enters a cell adjacent to the carrier of the condition "an enemy entered an adjacent cell" stops there.
- The HP threshold triggers when HP goes from the threshold level or higher to a level below it.

### 3.3 Passive skills

They work constantly while the skill is selected for the map: an addition to a stat, tag-based increase, resistance. A passive skill can be a **permanent aura** (section 3.5). A passive skill works at a fixed strength, without a roll. A passive Empowerment does not strengthen the basic attack.

### 3.4 Choosing skills for the map

- Before entering a map the player chooses **4 active, 1 reactive, 1 passive** from their skills: exactly min(slots, number of skills the character has in that category). The same applies at a spawn.
- The choice is recorded **in the same transaction** as joining the party (in `Participation`).
- Until the end of the map the set **does not change**. The character's other skills are unavailable on this map.

### 3.5 Auras

An aura affects the character themselves **and allies within a radius** around them.

| Kind          | Category | How it works                                                | Example                                                                                  |
| ------------- | -------- | ----------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| **Permanent** | passive  | works all the time while the skill is selected for the map  | nearby allies gain Cold resistance                                                       |
| **Temporary** | reactive | switches on by a condition for several turns, then cooldown | Human Warrior: HP below a threshold → Protection for self and nearest allies for X turns |

Rules:

- An aura can belong to **any class**, not only to Support. Support simply strengthens auras more (`CHARACTER.md`, section 8.1).
- **One aura = one effect.** An aura gives only one thing: one stat, one resistance, one increase. This way auras do not turn into over-buffing.
- Identical auras from different allies **do not stack**: the strongest one applies. Auras of one kind do not stack with each other.
- A passive debuff works only as an aura on enemies within the radius. A passive debuff without an aura is not allowed. An aura has no hard control (Stun, Freeze).
- An aura has an aspect, and it takes part in the **reactions** of allies, like any buff: Protection with Blood on an ally with Fire can produce Searing Rage.
- The aura radius is in cells, line of sight is not needed.

---

## 4. Aspects

### 4.1 How many aspects

| Skill purpose                  | Aspects                                     | Effect                                                |
| ------------------------------ | ------------------------------------------- | ----------------------------------------------------- |
| **Damage, pure**               | 1 or 2                                      | none; with 2 aspects a **fusion** appears (section 7) |
| **Damage with an effect**      | exactly 1                                   | 1 effect of choice                                    |
| **Damage with a third aspect** | 3                                           | only at the price of giving things up (section 4.3)   |
| **Buff**                       | 1 (element or modifier: Blood, Piercing...) | 1 buff kind                                           |
| **Debuff**                     | 1                                           | 1 debuff kind                                         |

For a buff and a debuff the aspect is the **aspect of the effect**: it takes part in reactions (`DAMAGE_TYPES_AND_MIXES.md`, section 7). "Haste with Blood" on an ally with Fire produces Searing Rage.

### 4.2 Order of aspects

The order of choice matters:

- the **first aspect** is the main one: it gets the largest share of damage and determines the base of the resonance;
- the **second** gets a smaller share and sets the fusion together with the first;
- the **third** gets the smallest share.

Example of a distribution (numbers are illustrative): Cold 4-6, Lightning 2-3, Fire 1-1. Decreasing shares keep players from simply taking as many aspects as possible for damage.

### 4.3 The third aspect

The third aspect can be taken only by **giving up** everything that adds complexity to the skill:

- **no fusion** ("burn with ice" and similar are lost);
- **no effect** (no slows, no freezes);
- **a single target only** (no line and no area).

In return the player gets a third damage packet. The only exception is given by the Mythic lens (section 9.3).

### 4.4 Every aspect works on its own

**There is no limit on the number of aspects in one use.** Each aspect of the skill:

- gives **its own damage packet** (by decreasing shares, section 4.2);
- acts as **its own base** of a resonance with the character's tints (`DAMAGE_TYPES_AND_MIXES.md`, section 6.1);
- causes **its own collisions** with states on the target (`DAMAGE_TYPES_AND_MIXES.md`, section 8).

Example: a "Lightning + Cold + Fire" skill against a frozen target. Lightning gives Ice Discharge and its own resonance, Cold extends the Freeze, Fire thaws it and deals damage for the turns removed.

This is an incentive to take several aspects and to build teams whose skills have synergy. The cost remains in the builder: power is divided between aspects, and a third aspect means giving up fusion, effect and multiple targets (section 4.3).

---

## 5. Where damage comes from

### 5.1 Weapon base

**The damage base always comes from the weapon** (or from the fist when unarmed). A weapon has a damage range and its own aspect: a Lightning wand 5-8, a Piercing sword, a Blunt hammer.

### 5.2 Conversion

The skill's aspects **convert** the weapon base into their own elements:

- the weapon base is distributed between the skill's aspects by decreasing shares (section 4.2);
- the weapon's own aspect does not enter the skill's damage, but **remains an aspect of the item** and can trigger as a resonance tint (`DAMAGE_TYPES_AND_MIXES.md`, section 4).

Example: a Cold mage with a Lightning wand uses the "Cold" skill. The wand's base 5-8 becomes a Cold packet, and the wand's Lightning rolls the tint die and can give the resonance "Cold with a Lightning tint" (Ringing Ice).

### 5.3 Skill power

Every skill has a **power**: a multiplier on the weapon base. All builder choices divide it:

| Choice        | Influence                                                                |
| ------------- | ------------------------------------------------------------------------ |
| Target: line  | lower damage, lower effect chance, longer cooldown                       |
| Target: area  | even lower damage, even lower effect chance, even longer cooldown        |
| Second aspect | power is divided between aspects, a fusion appears                       |
| Third aspect  | power is divided by three, giving up fusion, effect and multiple targets |
| Effect        | part of the power goes to the effect                                     |

All penalties are **multipliers and counterweights**, not subtractions (`COMBAT_MATH.md`, sections 3-4).

### 5.4 Order of calculation

1. Weapon base.
2. Conversion into the skill's aspects by shares.
3. Skill power with the penalties of the choices.
4. Added packets (predisposition, items, lenses).
5. Then per `COMBAT_MATH.md`, section 6: increase, multipliers, damage roll, target defense.

---

## 6. Effects

### 6.1 Starting set

| Effect                         | Kind         | Aspect                                       |
| ------------------------------ | ------------ | -------------------------------------------- |
| **Weakening**                  | debuff       | any                                          |
| **Slow**                       | debuff       | any                                          |
| **Vulnerability to an aspect** | debuff       | any; vulnerability to the skill's own aspect |
| **Stun**                       | hard control | only with Blunt                              |
| **Freeze**                     | hard control | only with Cold                               |

Hard control as a **chosen effect of a skill** is tied to the aspect by physics: you cannot freeze with Fire or stun with a Piercing strike.

This limit applies only to the builder. The same states can arise in other ways: for example, Lightning hitting a Chilled target stuns it through a **state collision** (`DAMAGE_TYPES_AND_MIXES.md`, section 8).

### 6.2 Buffs at the start

Haste, Empowerment, Protection, Resistance. The buff's aspect determines the recipient's reaction. Resistance gives resistance to the skill's aspects by their shares, plus the bearer's predispositions, each with the share of its base aspect.

**Heal** is open to monsters only: as an active or reactive skill on a single target (not a passive, not an aura). A kin-only skill heals and buffs only the monster's own kin (`BESTIARY.md`, section 2). Players cannot take Heal.

### 6.3 Effect and states

A skill's effect is **separate from the aspect's native state**. A Cold skill with the Freeze effect has both the effect chance and the usual Chill chance from Cold damage.

---

## 7. Fusions

### 7.1 What it is

A **fusion** is an effect that appears when **two aspects** are chosen in one skill. It is recorded in the skill and triggers by its own d100 chance on every use.

A fusion and a resonance are **different mechanics with different tables**:

|        | Fusion                       | Resonance                                           |
| ------ | ---------------------------- | --------------------------------------------------- |
| Source | two aspects inside the skill | the skill's aspect + the character's predisposition |
| When   | d100 chance on every use     | tint die d100                                       |
| Table  | section 7.3 of this document | `DAMAGE_TYPES_AND_MIXES.md`, section 6              |

One hit can give both a fusion and a resonance at the same time.

### 7.2 Fusion rule

**The first aspect's damage takes on the native state of the second.**

Cold + Fire: Cold deals damage over time, as if burning: so cold that it burns. The resonance "base Cold, tint Fire" does something different here (Burning Frost).

The rule gives mechanics for any pair at once, only the names are written by hand.

### 7.3 Fusion table (draft)

Row: first aspect. Column: second aspect. In the cell: the name of the fusion, whose damage takes on the native state of the second aspect.

| First ↓ / Second → | Fire (Burning)    | Cold (Chill)      | Lightning (Shock) | Blood (Bleeding)  | Blunt (Stun)       | Piercing (Penetration) |
| ------------------ | ----------------- | ----------------- | ----------------- | ----------------- | ------------------ | ---------------------- |
| **Fire**           |                   | Shackling Flame   | Sparking Flame    | Scorching Wound   | Fire Blast         | Melting Flame          |
| **Cold**           | **Frost Burn**    |                   | Icy Jolt          | Frost Wound       | Heavy Ice          | Ice Needles            |
| **Lightning**      | Blazing Discharge | Numbing Discharge |                   | Tearing Discharge | Stunning Discharge | Penetrating Discharge  |
| **Blood**          | Flaming Blood     | Icy Blood         | Sparking Blood    |                   | Heavy Blood        | Blood Thorns           |
| **Blunt**          | Red-hot Blow      | Frost Blow        | Thundering Blow   | Ragged Blow       |                    | Spiked Blow            |
| **Piercing**       | Fire Sting        | Frost Sting       | Spark Sting       | Blood Sting       | Ramming Sting      |                        |

The same aspects are not chosen in one skill.

---

## 8. Cooldowns

- **All active skills** have a cooldown in turns.
- A basic attack has no cooldown.
- Cooldown grows with line and area, with a third aspect, with hard control.
- Reactive skills also have a cooldown.
- The cooldown is stored in the party state and decreases at the start of the character's turn.

---

## 9. Skill growth: lenses

A late-game mechanic. In the MVP skills do not grow, power grows from the character and items.

### 9.1 Skillsmith

A live player with the **Skillsmith** profession applies lenses to other players' skills (as a Blacksmith crafts for others).

Lenses apply to **all categories**: active, reactive and passive skills. What exactly a lens does to a reactive or passive skill is determined when such lenses appear.

### 9.2 Lens tiers

| Lens tier         | What it can do                                                                                        | Limit per skill             |
| ----------------- | ----------------------------------------------------------------------------------------------------- | --------------------------- |
| **Plain** (white) | a few points of damage to one aspect                                                                  | 10                          |
| **Magic** (blue)  | more damage to one aspect or a few points to several                                                  | 7                           |
| **Rare**          | stronger than Magic (to be specified)                                                                 | 3                           |
| **Unique**        | a special effect (to be specified)                                                                    | 1                           |
| **Mythic**        | breaks the builder's rules: adds multi-target damage or one effect even to a skill with three aspects | 1, under special conditions |

### 9.3 Mythic lens

- Applied **only to a pure skill** that has no lens used on it.
- After it the skill **no longer accepts any lenses** of any lens tier.
- The effect is **irreversible**.
- Obtaining a Mythic lens is incredibly hard: many hours of play and complex crafting.

A choice for the player: either up to 21 regular lenses gradually, or one Mythic lens that changes the rules for this skill.

---

## 10. Creating skills

### 10.1 Starting skills

- The first 6 skills (4 active, 1 reactive, 1 passive) are created **by a plain transaction**, without resources.
- Starting skills are **limited by class**: at the beginning the character is recognizable by their path.

| Class       | Available among starting skills                                         |
| ----------- | ----------------------------------------------------------------------- |
| **Warrior** | only martial skills (strike and projectile)                             |
| **Mage**    | only magical skills (strike and projectile)                             |
| **Support** | only with the purpose buff or debuff, including auras; any power source |

Race **does not limit** starting skills: aspects are chosen freely. Race has its influence through predisposition (`CHARACTER.md`, section 7).

In the starter presets Bleeding Cleave is a ranged line skill, and Last Stand triggers at an HP threshold of 40%.

### 10.2 Uncut skills

- After the start, new skills are created from an **uncut skill** (a working name; possibly an empty scroll): a special item obtained in the game.
- Creating from an uncut skill requires **resources**. There is no gold in the game, only resources.
- During **cutting**, class and race limits do not apply: any character can create any skill, as in PoE. The other builder rules (section 13) always apply.

### 10.3 Checking

Creation is checked by the program: category, tags, number of aspects, effect, the conditions of section 4.3, and for starting skills also the limits of section 10.1.

## 11. Ownership and death

- A skill is a **soulbound item of the character**.
- It cannot be sold or transferred.
- The character's bag holds **no more than 50 skills** (a starting limit, may change).
- A skill can be **deleted** to free up space. Lenses invested in it are **destroyed** together with it.
- When the character dies, the mechanics of the skills are erased together with them; the skill names and descriptions stay on the tombstone as a memorial.
- In the late game a Skillsmith can modify skills with lenses (section 9).

---

## 12. On-chain storage

The skill account (PDA, bound to the character):

| Group     | Fields                                                                       |
| --------- | ---------------------------------------------------------------------------- |
| Owner     | character                                                                    |
| Structure | category, purpose, tags, aspects in order, effect, target, trigger condition |
| Power     | the power multiplier with penalties (computed at creation and stored)        |
| Fusion    | fusion id (computed from the pair of aspects)                                |
| Lenses    | counters of applied lenses by lens tier, Mythic flag                         |
| Story     | name, description **up to 200 printable ASCII characters**; the description is written once, by a separate instruction, and is not burned at death |

The game is in English: players write skill names and descriptions in English. The names in these documents are working names.

The cooldown and the choice for the map are stored not in the skill, but in the party state and `Participation`.

---

## 13. Builder checks

- [ ] Tags are compatible (magical ✗ martial, strike ✗ projectile)
- [ ] The number of aspects matches the purpose (section 4.1)
- [ ] Aspects do not repeat
- [ ] Third aspect: no effect, no fusion, one target
- [ ] Effect only with one damage aspect (except the Mythic lens)
- [ ] Hard control only with its own aspect (Freeze with Cold, Stun with Blunt)
- [ ] A buff and a debuff have exactly one aspect and one effect kind
- [ ] A reactive skill has a trigger condition
- [ ] Aura: only a passive or reactive skill, exactly one effect, a radius is set
- [ ] The name is not longer than the limit, the description is not longer than 200 printable ASCII characters
- [ ] **Starting skill:** power source and purpose are allowed by the class (section 10.1)
- [ ] **Skill from an uncut skill:** the character has an uncut skill and the needed resources, both are burned at creation
- [ ] Slots on entering the map: exactly 4 active, 1 reactive, 1 passive (or fewer, if there are fewer skills)
