# DAMAGE_TYPES_AND_MIXES: aspects, states, resonances and reactions of Deadroll

Version 0.2, draft. The foundation of the future wiki. **There are no numbers in this document**: chances, durations and strength are set at balancing (the calculation logic is in `COMBAT_MATH.md`). All names are marked as drafts and may change.

The document describes the **model**: what parts damage and effects are made of, how they combine, and by what rules the result is calculated. The model is built so that new aspects, effects and combinations are added as **data**, not as new code.

---

## 0. How to read

| Layer                                  | Where it acts                                                          | Section |
| -------------------------------------- | ---------------------------------------------------------------------- | ------- |
| **Damage tags** (axes)                 | on a skill                                                             | 2       |
| **Aspects**, families and their physics | on skills, characters, items                                          | 3       |
| **Carriers** of aspects and tiers      | character, items                                                       | 4       |
| **States**                             | on the target                                                          | 5       |
| **Resonances**                         | inside one character when they use their skill                         | 6       |
| **Reactions**                          | when someone else's effect lands on a character                        | 7       |
| **State collisions**                   | on the target, when a damage packet meets an already applied state     | 8       |
| **Resistances and immunities**         | on the target                                                          | 9       |
| **Modularity and extension**           |                                                                        | 10      |

---

## 1. Glossary

| Term                | Meaning                                                                                                                                              |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Aspect**          | the elemental or physical nature of anything: Fire, Cold, Blood, Piercing... Not only of damage: Haste can also be "Blood" or "Lightning"             |
| **Family**          | a group of aspects: elemental, physical, mental, sacred                                                                                              |
| **Carrier**         | where a character's aspect comes from: a skill, a race predisposition, a class predisposition, an item                                              |
| **Tier**            | the strength of a carrier, from 14 (weak) to 1 (strongest), like affixes in PoE                                                                      |
| **Effect kind**     | what is done: damage, Haste, Slow, Empowerment, Weakening, Protection, Vulnerability; later curses, blessings, prayers, defilements                  |
| **State**           | what an aspect does to the target on a hit: Burning, Freeze...                                                                                       |
| **Resonance**       | what is born from the aspects of one character when they use their skill                                                                             |
| **Base**            | the aspect of the skill itself, the main one in a resonance                                                                                          |
| **Tint**            | any other aspect of the character (race, class, item) that adds its trait to a resonance                                                             |
| **Tint trait**      | what an aspect contributes when it acts as a tint                                                                                                    |
| **Reaction**        | what happens when an effect (from an ally or an enemy) lands on a character with a predisposition                                                   |
| **Transformation**  | a reaction where the effect turns into a new state (Soaked, Brittleness, Embers)                                                                     |
| **Collision**       | what happens when, on a target, one state meets another (a future layer)                                                                             |

---

## 2. Damage tags (axes)

Each skill and each hit is described by a set of tags along independent axes. Tags are needed for rules ("increases projectile damage", "immunity to magical").

| Axis             | Question                  | Starting values                                                                       | How many per skill            |
| ---------------- | ------------------------- | ------------------------------------------------------------------------------------- | ----------------------------- |
| **Family**       | what the damage is made of | elemental, physical; later mental, sacred                                            | determined by the base aspect |
| **Power source** | what powers it            | magical, martial                                                                      | exactly one                   |
| **Form**         | how it reaches the target | strike (hits the target on contact), projectile (a flying missile); later area, aura | one                           |
| **Range**        | from where                | melee, ranged                                                                         | one                           |
| **Timing**       | when it is dealt          | instant, over time                                                                    | one                           |

For now the timing tag is only a tag: it does not change any mechanic.

### 2.1 Compatibility rules

| Rule                    | Explanation                                      |
| ----------------------- | ------------------------------------------------ |
| magical ✗ martial       | a skill has one power source                     |
| strike ✗ projectile     | a blow on contact or a flying missile            |
| strike ✓ magical        | a fireball or a bolt of lightning hits the target |
| projectile ✓ magical    | a magic arrow, an ice spear                      |
| projectile ✓ martial    | an arrow, a thrown axe                           |

Examples:

| Skill                      | Source  | Form       | Range                                           |
| -------------------------- | ------- | ---------- | ----------------------------------------------- |
| sword strike               | martial | strike     | melee                                           |
| thrown axe                 | martial | projectile | ranged                                          |
| fireball                   | magical | strike     | ranged                                          |
| ice spear                  | magical | projectile | ranged                                          |
| touch spell                | magical | strike     | melee                                           |
| arrow from an enchanted bow | martial | projectile | ranged; the aspect comes from the carrier item |

Compatibility rules are stored as data (pairs of forbidden tags) and are checked by the skill builder and by the program.

### 2.2 Damage over time

"Over time" is a tag of the timing axis, not a separate aspect. Burning, Bleeding and, in the future, Poisoning have it. Effects like "increases damage over time" act on all states with this tag at once.

---

## 3. Families, aspects and their physics

### 3.1 Families

| Family         | Starting aspects          | Future aspects         |
| -------------- | ------------------------- | ---------------------- |
| **Elemental**  | Fire, Cold, Lightning     | Wind, Earth, Water     |
| **Physical**   | Blood, Blunt, Piercing    | Slashing, Poison       |
| **Mental**     |                           | Fear, Madness, Charm   |
| **Sacred**     |                           | Darkness, Light        |

### 3.2 Starting aspects

| Aspect        | Family    | Native state          | Tint trait                                                          | Epithet for triples |
| ------------- | --------- | --------------------- | ------------------------------------------------------------------- | ------------------- |
| **Fire**      | elemental | Burning               | **Fury**: growing damage, adds Fire to the hit                      | of Ash              |
| **Cold**      | elemental | Chill → Freeze        | **Composure**: accuracy, control, slowing                           | of Frost            |
| **Lightning** | elemental | Shock                 | **Swiftness**: speed, jumping to an adjacent target                 | of the Storm        |
| **Blood**     | physical  | Bleeding              | **Wasting**: damage over time, weakening the target                 | of Blood            |
| **Blunt**     | physical  | Stun                  | **Crushing**: hit strength, stun, knockback                         | of Ruin             |
| **Piercing**  | physical  | Penetration (of armor) | **Keenness**: crit, penetration                                    | of the Point        |

Blood can be both martial (a cut, bleeding) and magical (blood magic). It is one aspect with different power sources.

### 3.3 Aspect physics

The game relies on real physical properties, however simplified. This table is the **source** for all relations, transformations and collisions: if a new combination can be explained by physics, it is explained by it.

| Aspect        | Physics                                                                | What follows                                                                                           |
| ------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| **Fire**      | heats, melts, evaporates, expands                                      | against Cold; boils blood; makes you sweat (water); Blunt scatters it into **Embers**                 |
| **Cold**      | makes things brittle, thickens liquids, slows movement                 | Piercing against Cold gives **Brittleness**; thickens blood; ice is shattered by Blunt                 |
| **Lightning** | flows through water, metal, blood; creates a static field              | stronger on the wet (**Soaked**) and on the bleeding; the static field hinders movement                |
| **Blood**     | a liquid: conducts current, freezes, boils; carries life and movement  | faster blood flow gives speed and heat (**Searing Rage**); vulnerable to Cold, Fire and current        |
| **Blunt**     | shatters the brittle, scatters, shakes                                 | shatters ice; scatters fire; stuns                                                                     |
| **Piercing**  | finds cracks, penetrates inside                                        | exploits Brittleness; drives cold or poison into a wound; penetrates armor                             |

A new aspect is added together with its own row in this table.

---

## 4. Aspect carriers and tiers

| Carrier                                           | Tier         | How it changes                                                                                       |
| ------------------------------------------------- | ------------ | ---------------------------------------------------------------------------------------------------- |
| **Skill**                                         | **no tier**  | the skill sets only the base; otherwise players would recreate characters for a lucky tier           |
| **Race predisposition**                           | has          | grows **deterministically**: investing in a skill branch, special items                              |
| **Class predisposition**                          | has          | grows **deterministically**: investing in a skill branch, special items                              |
| **Item** (affix from crafting, enchantment)       | has          | **random** at item creation; the item is lost and burned on death                                    |

A tier from 14 to 1 (1 is the strongest) sets:

- the die **chance** when the carrier acts as a tint;
- the **strength** of the tint trait in a resonance;
- the **strength** of affinity or conflict in reactions.

**The die only decides "triggered or not".** The strength of the effect always comes from the tier, not from the roll. A single point of randomness per resonance keeps the calculations tunable, even when effects stack on each other.

The aspect of the weapon does not become a tint of the character's own basic attack: the weapon is already the base there. The hit parts of tint traits (Fury, Keenness) work every time their tint triggers.

Tier ranges for each carrier are set at balancing.

---

## 5. States

| State          | Aspect    | Timing    | Essence                                              |
| -------------- | --------- | --------- | ---------------------------------------------------- |
| **Burning**    | Fire      | over time | Fire damage every turn                               |
| **Chill**      | Cold      |           | slows; on buildup turns into Freeze                  |
| **Freeze**     | Cold      |           | the target skips an action or movement               |
| **Shock**      | Lightning |           | the target takes more damage                         |
| **Bleeding**   | Blood     | over time | physical damage every turn, increases with movement  |
| **Stun**       | Blunt     |           | the target loses initiative or an action             |
| **Penetration** | Piercing |           | lowers the target's armor                            |

One rule for all: a hit with an aspect gives a chance to apply its native state, if the target is not immune to it.

**Pure pair** (the tint matches the base, for example a Cold mage with a Cold predisposition): there is no separate resonance; instead the **chance and strength of the native state grow**.

---

## 6. Resonances

### 6.1 Rules

1. A character uses a skill. **Each aspect of the skill is a separate base**: a skill with three aspects has three bases.
2. All other aspects of the character (race, class, items) are **tint candidates**.
3. For **each base separately**:
   - **target check:** if the target is immune to this base, no dice are rolled for it;
   - each tint rolls **its own d100**, the chance depends on the carrier's tier;
   - none hits: there is no resonance for this base (a normal outcome);
   - one hits: a **pair resonance** (base + tint);
   - two or more hit: a **triple resonance** from the two best rolls (base + 2 tints).
4. A tie between rolls is resolved by carrier priority: item > class > race.
5. Each resonance has a **target condition** (for example, "the target can be frozen"). If the condition is not met, the resonance does not trigger.
6. The strength of a resonance comes from the tiers of the triggered carriers; the die does not affect strength.
7. **There is no limit on the number of aspects in one use.** A skill with three aspects can produce three resonances. The number of aspects is limited by the skill builder (`SKILL_BUILDER.md`, section 4), and skill power is divided among them.

### 6.2 Skill categories

The same pair gives different resonances depending on the skill category:

| Category                                | Where the resonance acts                      |
| --------------------------------------- | --------------------------------------------- |
| **Attacking**                           | on the hit and the target                     |
| **Self-targeted** (aura, buff)          | on the character themselves                   |
| **Reactive** (defense, counterattack)   | on the character, triggers from events        |

In version 0.2 only the **attacking** resonances are filled in. A counterattack hits through the table of attacking resonances. Self-targeted buffs, auras and reactive buffs have no resonances for now. Columns for the other categories are added as the wiki grows.

### 6.3 How the mechanics are calculated

**Resonance mechanics = native state of the base + tint trait.** The name and lore are written by hand. So any new pair gets its mechanics immediately, and the wiki only has to give it a name.

Order matters: "Cold with a Piercing tint" and "Piercing with a Cold tint" are different resonances.

Calculation rules for the tables below:

- A linked state is applied together with the native state of the base, without a roll of its own. A resonance rolls its own die only where its essence says "a chance of".
- A jump passes the state to the next target, not the damage.
- An adjacent target is a cell at Chebyshev distance 1; if there are several, the one with the lower index is taken.
- "Next in line" is the cell behind the target in the direction from the attacker to the target.
- Pale Flame: the accuracy bonus is a buff on the attacker for 2 turns.
- Blasting Flame sets adjacent cells on fire for `burning_cell_rounds` rounds.

### 6.4 Attacking pair resonances

Row: base (the skill's aspect). On the right: tint, name (draft), essence, target condition.

#### Base: Fire

| Tint      | Name             | Essence                                                              | Condition           |
| --------- | ---------------- | -------------------------------------------------------------------- | ------------------- |
| Cold      | Pale Flame       | Burning lasts longer, Fire attacks are more accurate                 | target can burn     |
| Lightning | Stormfire        | Burning jumps to an adjacent target                                  | target can burn     |
| Blood     | Boiling Blood    | Burning also causes Bleeding                                         | target can burn     |
| Blunt     | Blasting Flame   | the hit knocks back and ignites adjacent cells                       | target can burn     |
| Piercing  | Stinging Flame   | a crit is guaranteed to ignite, Burning penetrates part of the armor | target can burn     |

#### Base: Cold

| Tint      | Name                       | Essence                                                                     | Condition                  |
| --------- | -------------------------- | --------------------------------------------------------------------------- | -------------------------- |
| Fire      | Burning Frost              | when it thaws, Freeze deals damage                                          | target can be frozen       |
| Lightning | Ringing Ice                | Freeze jumps to an adjacent target                                          | target can be frozen       |
| Blood     | Chilled Blood              | Chill also weakens the target's strength                                    | target can be chilled      |
| Blunt     | Cleaving Ice               | a hit on a frozen target deals extra damage and removes Freeze              | target is frozen           |
| Piercing  | **Piercing Frost**         | a crit on a frozen target is stronger, a crit can freeze                    | target can be frozen       |

#### Base: Lightning

| Tint     | Name                  | Essence                                              | Condition                  |
| -------- | --------------------- | ---------------------------------------------------- | -------------------------- |
| Fire     | Incinerating Discharge | Shock ignites the target                            | target can be shocked      |
| Cold     | Frost Spark           | Shock also chills                                    | target can be shocked      |
| Blood    | Blood Arc             | the discharge jumps further along bleeding targets   | target can be shocked      |
| Blunt    | Thunderclap           | Shock with a chance of Stun                          | target can be shocked      |
| Piercing | Lightning Needle      | the discharge passes through the target to the next one in line | target can be shocked |

#### Base: Blood

| Tint      | Name                     | Essence                                                  | Condition              |
| --------- | ------------------------ | -------------------------------------------------------- | ---------------------- |
| Fire      | Seething                 | Bleeding deals part of its damage as Fire damage         | target can bleed       |
| Cold      | Chilling Wounds          | Bleeding slows                                           | target can bleed       |
| Lightning | Frenzied Pulse           | Bleeding ticks more often but is shorter                 | target can bleed       |
| Blunt     | Internal Bleeding        | Bleeding is stronger and is not removed by simple healing | target can bleed      |
| Piercing  | Deep Wound               | a crit lengthens and strengthens Bleeding                | target can bleed       |

#### Base: Blunt

| Tint      | Name                 | Essence                                    | Condition                |
| --------- | -------------------- | ------------------------------------------ | ------------------------ |
| Fire      | Red-hot Hammer       | a stunned target catches fire              | target can be stunned    |
| Cold      | Frozen Ram           | Stun lasts longer, the target is chilled   | target can be stunned    |
| Lightning | Concussive Discharge | Stun jumps to an adjacent target           | target can be stunned    |
| Blood     | Pulverize            | Stun causes Bleeding                       | target can be stunned    |
| Piercing  | Breach               | Stun also penetrates armor                 | target can be stunned    |

#### Base: Piercing

| Tint      | Name              | Essence                                     | Condition              |
| --------- | ----------------- | ------------------------------------------- | ---------------------- |
| Fire      | Red-hot Point     | Penetration ignites                         | target has armor       |
| Cold      | **Icy Point**     | Penetration chills                          | target has armor       |
| Lightning | Piercing Spark    | Penetration with a chance of Shock          | target has armor       |
| Blood     | Bloodletting      | a crit causes Bleeding                      | target can bleed       |
| Blunt     | Armor Breaker     | Penetration is stronger against heavy armor | target has armor       |

### 6.5 Triple resonances

Triple mechanics: **the pair resonance with the best tint + the tint trait of the second tint**.

The order of the two tints between themselves does not matter (base Cold + Piercing + Lightning = base Cold + Lightning + Piercing). For 6 aspects that is 6 bases × 10 pairs of tints = 60 triples.

The name is generated: **the name of the pair with the best tint + the epithet of the second tint** (epithets in 3.2).

Example: base Cold, best tint Piercing, second Lightning → **Piercing Frost of the Storm**: a crit on a frozen target is stronger + the Lightning tint trait (the effect jumps to an adjacent target).

Any triple can be **overridden manually** (its own name, its own mechanics) if generation gives a boring result. Overrides are marked in the wiki.

---

## 7. Reactions

### 7.1 What this is

An effect (an ally's buff or an enemy's debuff) lands on a character and **is refracted through their predisposition**. The same Support cast on different allies plays out differently.

### 7.2 Effect kinds at the start

| Kind          | Beneficial / harmful |
| ------------- | -------------------- |
| Haste         | beneficial           |
| Slow          | harmful              |
| Empowerment   | beneficial           |
| Weakening     | harmful              |
| Protection    | beneficial           |
| Vulnerability | harmful              |

Later: curses, blessings, prayers, defilements.

### 7.3 Two tables

**Relation table** (effect aspect → recipient's predisposition). Values:

| Value                     | For a beneficial effect        | For a harmful effect           |
| ------------------------- | ------------------------------ | ------------------------------ |
| **neutral**               | no change                      | no change                      |
| **affinity**              | stronger                       | weaker                         |
| **conflict**              | weaker                         | stronger                       |
| **transformation into X** | the effect becomes state X     | the effect becomes state X     |

**Inclination table** (effect kind → recipient's predisposition): some predispositions on their own like or dislike certain effect kinds, regardless of their aspect.

### 7.4 Calculation order

1. If the inclination table or the relation table has a **transformation** for this pair, it triggers (inclination takes precedence over relation).
2. Otherwise affinity and conflict from both tables are added together: two Empowerments give a strong Empowerment, an Empowerment and a Weakening cancel each other.
3. Strength depends on the tier of the recipient's predisposition.

### 7.5 Relation table (draft)

Row: effect aspect. Column: recipient's predisposition.

| Effect ↓ / Recipient → | Fire                            | Cold                           | Lightning | Blood    | Blunt    | Piercing |
| ---------------------- | ------------------------------- | ------------------------------ | --------- | -------- | -------- | -------- |
| **Fire**               | affinity                        | transformation: **Soaked**     | neutral   | conflict | neutral  | neutral  |
| **Cold**               | conflict                        | affinity                       | neutral   | conflict | neutral  | neutral  |
| **Lightning**          | neutral                         | conflict                       | affinity  | conflict | neutral  | neutral  |
| **Blood**              | transformation: **Searing Rage** | neutral                       | neutral   | affinity | neutral  | neutral  |
| **Blunt**              | transformation: **Embers**      | conflict                       | neutral   | neutral  | affinity | neutral  |
| **Piercing**           | neutral                         | transformation: **Brittleness** | neutral  | conflict | neutral  | affinity |

Physical explanation of each non-neutral cell:

| Pair                  | Value        | Why (section 3.3)                                    |
| --------------------- | ------------ | ---------------------------------------------------- |
| Fire → Cold           | Soaked       | heat makes you sweat, water conducts current         |
| Fire → Blood          | conflict     | fire boils blood                                     |
| Cold → Fire           | conflict     | cold puts out fire                                   |
| Cold → Blood          | conflict     | cold thickens blood                                  |
| Lightning → Cold      | conflict     | the static field hinders movement                    |
| Lightning → Blood     | conflict     | blood conducts current                               |
| Blood → Fire          | Searing Rage | accelerated blood heats up to fire                   |
| Blunt → Fire          | Embers       | the blow scatters fire into coals                    |
| Blunt → Cold          | conflict     | ice shatters                                         |
| Piercing → Cold       | Brittleness  | cold makes things brittle, the point finds a crack   |
| Piercing → Blood      | conflict     | punctures bleed                                      |
| any aspect → the same | affinity     | one's own element is familiar                        |

### 7.6 Inclination table (draft)

Row: effect kind. Empty cells = no inclination.

| Kind ↓ / Recipient → | Fire | Cold                                    | Lightning                         | Blood | Blunt    | Piercing |
| -------------------- | ---- | --------------------------------------- | --------------------------------- | ----- | -------- | -------- |
| **Haste**            |      | transformation: **Cold Mind**           | affinity: **Lightning Reflexes**  |       | conflict |          |
| **Slow**             |      |                                         |                                   |       |          |          |
| **Empowerment**      |      |                                         |                                   |       |          |          |
| **Weakening**        |      |                                         |                                   |       |          |          |
| **Protection**       |      |                                         |                                   |       |          |          |
| **Vulnerability**    |      |                                         |                                   |       |          |          |

### 7.7 Transformed states

| State                | Source                                                                    | Essence                                                                                                                                                  |
| -------------------- | ------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Soaked**           | a Fire effect on a recipient with Cold                                    | vulnerability to Lightning                                                                                                                               |
| **Searing Rage**     | a Blood effect (for example, Haste by blood magic) on a recipient with Fire | reaction speed, Fire added to attacks                                                                                                                  |
| **Cold Mind**        | Haste on a recipient with Cold                                            | instead of part of the speed, luck or wisdom grows                                                                                                       |
| **Embers**           | a Blunt effect on a recipient with Fire                                   | the recipient's inner fire scatters: their Fire bonuses are weaker, but adjacent cells smolder and set on fire those who step on them                    |
| **Brittleness**      | a Piercing effect on a recipient with Cold                                | the recipient is vulnerable to Blunt damage and crits                                                                                                    |

### 7.8 Examples

| Situation                                        | What happens                          | Why                                              |
| ------------------------------------------------ | ------------------------------------- | ------------------------------------------------ |
| Enemy: Cold Slow → player with Cold              | the Slow is weaker                    | Cold → Cold: affinity, the effect is harmful     |
| Enemy: Lightning Slow → player with Cold        | the Slow is stronger                  | Lightning → Cold: conflict                       |
| Enemy: Fire Slow → player with Cold              | the player is **Soaked**, vulnerable to Lightning | Fire → Cold: transformation        |
| Enemy: Piercing debuff → player with Cold        | the player gets **Brittleness**       | Piercing → Cold: transformation                  |
| Enemy: stunning blow → player with Fire          | **Embers** around the player          | Blunt → Fire: transformation                     |
| Support: Haste → ally with Lightning             | **Lightning Reflexes**, higher speed  | Haste → Lightning: inclination, affinity         |
| Support: Haste → ally with Cold                  | **Cold Mind**                         | Haste → Cold: inclination, transformation        |
| Blood mage: Blood Haste → ally with Fire         | **Searing Rage**                      | Blood → Fire: transformation                     |

---

## 8. State collisions

When a damage packet hits a target that already has a state, they can **collide** and produce a third effect.

- The check runs **for each damage packet of a hit separately** and for each state on the target. A skill with three aspects can cause three collisions.
- **Order matters:** "on target A, B arrives" and "on target B, A arrives" can give different results.
- Stored as a table "state on target × packet aspect → result"; an empty cell = nothing happens.
- The physics from section 3.3 applies here too.
- **Collision damage is a separate instance of damage:** it takes the target's defense into account but **does not receive** the attacker's multipliers a second time. This way layers do not multiply each other in a loop.

### 8.1 Starting table (draft)

| On target   | Arrives   | Result                | Essence                                                                                                  |
| ----------- | --------- | --------------------- | -------------------------------------------------------------------------------------------------------- |
| Chill       | Lightning | **Stun**              | current constrains chilled muscles                                                                       |
| Freeze      | Lightning | **Ice Discharge**     | ice conducts current: the Lightning packet deals more damage                                             |
| Freeze      | Cold      | **Extension**         | Freeze lasts longer                                                                                      |
| Freeze      | Fire      | **Thaw**              | Freeze is removed; damage is proportional to the Fire damage of this hit and the number of Freeze turns removed |
| Freeze      | Blunt     | **Shatter**           | Freeze is removed, large damage                                                                          |
| Burning     | Cold      | **Steam**             | Burning is removed, steam lowers the target's accuracy                                                   |
| Bleeding    | Fire      | **Cauterize**         | Bleeding stops, the remaining Bleeding damage is dealt at once as Fire damage                            |

### 8.2 Future (with aspects that do not exist yet)

| First                | Then                 | Result         | Essence                                        |
| -------------------- | -------------------- | -------------- | ---------------------------------------------- |
| Shock (Lightning)    | Poisoning (Poison)   | **Decay**      | flesh rots, the target's armor is lowered      |
| Poisoning (Poison)   | Shock (Lightning)    | **Radiation**  | the target loses control, may hit allies       |

---

## 9. Resistances and immunities

Each monster in the bestiary (and characters through items) has, for each aspect and for some tags (for example, "magical"), a defense level set:

| Level      | Damage  | Native state        | Resonances with this base            |
| ---------- | ------- | ------------------- | ------------------------------------ |
| normal     | full    | applied             | work                                 |
| resistance | reduced | lower chance and strength | work if the condition is met   |
| immunity   | none    | not applied         | **no dice are rolled**               |

Separately, a resonance may fail because of its own condition: for example, "Cleaving Ice" requires the target to be frozen **already**.

---

## 10. Modularity and extension

### 10.1 What is stored on-chain

| Data                                                          | Storage                                                                                        |
| ------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Aspect registry: id, family, native state, tags               | entries in the registry account                                                                |
| Tag compatibility rules                                       | a list of forbidden pairs                                                                      |
| Pair resonance mechanics                                      | generated by a rule from the base's state and the tint trait; overrides as separate entries    |
| Relation table, inclination table, collision table            | matrices of value ids in accounts                                                              |
| Monster resistances                                           | in bestiary entries                                                                            |

**The tables are data in accounts, not code.** The game authority changes them with a transaction; the program is not redeployed.

**Names, descriptions and lore are not stored on-chain**: only ids are on the chain. The texts live in this document and in the wiki generated from it.

### 10.2 Empty cell = neutral

Any unfilled cell in any table means "neutral" or "nothing happens". Therefore:

- a new aspect or a new effect kind **works immediately** with all the existing ones, just without special synergies;
- synergies are added gradually, as they are invented;
- there is no need to fill in all combinations in advance.

### 10.3 How to add a new aspect

1. A row in 3.1, 3.2 (family, native state, tint trait, epithet) and in **3.3 (physics)**.
2. Describe the native state in section 5.
3. A row and a column in the pair resonances (6.4): the mechanics are already defined by the rule, names need to be given.
4. A row and a column in the relation table (7.5) and a column in the inclination table (7.6), based on the physics.
5. If needed, rows in the collision table (8).
6. Set the monster resistances.
7. Record the new aspect and the table rows in the on-chain registry.

### 10.4 How to add a new kind of mechanic

For example, curses, the first aura, state collisions.

1. **Once**, the behavior code of this mechanic in the program.
2. Entries in the tables for synergies with the existing aspects (as they are invented, empty cells are neutral).
3. A section in this document.

---

## 11. Future extensions

| What                                              | Where it will appear                                                                                                           |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| Wind, Earth, Water                                | new aspects of the elemental family                                                                                            |
| Poison (Poisoning)                                | an aspect of the physical family, tag "over time"; together with it the collisions of section 8.2                              |
| Mental aspects                                    | a new family: Fear, Madness, Charm                                                                                             |
| **Sacred**: Darkness, Light                       | a new family                                                                                                                   |
| Curses, blessings, prayers, defilements           | new effect kinds; for example, the resonance "base Darkness + Piercing tint" gives a chance of a **Weakening curse**           |
| Resonances for self-targeted and reactive skills  | new columns in section 6                                                                                                       |
| Area, aura as a delivery form                     | new values of the form axis                                                                                                    |
