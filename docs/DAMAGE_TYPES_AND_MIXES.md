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

### 5.1 Debuffs from resonances

Resonances (section 6) also put timed debuffs on the target. Each one divides, never subtracts (`COMBAT_MATH.md`, section 3).

| Debuff       | Source         | Essence                                                       |
| ------------ | -------------- | ------------------------------------------------------------- |
| **Exposed**  | pairs, triples | every attacker's crit chance against the bearer grows         |
| **Root**     | pairs, triples | the bearer cannot move, but can still act                     |
| **Blinded**  | triples        | the bearer's d20 is divided                                   |
| **OffGuard** | triples        | the bearer's defense is divided                               |
| **Sundered** | triples        | the bearer's armor against Piercing is divided                |
| **Dulled**   | triples        | the bearer's crit chance is divided                           |
| **Dazed**    | triples        | the bearer uses no reactive skill (only monsters for now)     |
| **Silenced** | triples        | the bearer uses no skill; a basic attack and a move are still allowed |
| **NoHeal**   | triples        | heals on the bearer do nothing                                |

Slow and Weakening are the ordinary skill debuffs (`SKILL_BUILDER.md`, section 6.1); resonances use them too.

---

## 6. Resonances

### 6.1 Rules

1. A character uses a skill. **Each aspect of the skill is a separate base**: a skill with three aspects has three bases.
2. All other aspects of the character (race predisposition, items) are **tint candidates**. A class has no aspect, and the aspects of one skill never tint each other.
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

Row: base (the skill's aspect). On the right: tint, name, essence, target condition. The table follows `assets/registry/resonances.json` (mechanics) and `assets/wiki/resonances.json` (texts); the wiki shows the same rows with flavor text.

#### Base: Fire

| Tint | Name | Essence | Condition |
| --- | --- | --- | --- |
| Cold | Pale Flame | Burning lasts longer, and the attacker gets a Fire accuracy buff. | target can burn |
| Lightning | Stormfire | Burning jumps to the nearest other enemy. | target can burn |
| Blood | Boiling Blood | Bleeding comes together with Burning. | target can burn |
| Blunt | Blasting Flame | Knocks the target back one cell and sets the cells around it on fire. | target can burn |
| Piercing | Stinging Flame | A crit always burns; Penetration comes together with Burning. | target can burn |

#### Base: Cold

| Tint | Name | Essence | Condition |
| --- | --- | --- | --- |
| Fire | Burning Frost | Thaws a frozen target and deals damage for the Freeze turns removed. | target can be frozen |
| Lightning | Ringing Ice | Freeze jumps to the nearest other enemy. | target can be frozen |
| Blood | Chilled Blood | Weakens the target together with Chill. | target can be chilled |
| Blunt | Cleaving Ice | Against a frozen target: a damage multiplier, and the Freeze shatters. | target is frozen |
| Piercing | Piercing Frost | The hit is always a crit, and the target freezes. | target can be frozen |

#### Base: Lightning

| Tint | Name | Essence | Condition |
| --- | --- | --- | --- |
| Fire | Incinerating Discharge | If the target was already shocked before the hit, it catches fire. | target can be shocked |
| Cold | Ice Conduit | If the target is frozen, every enemy next to it takes a share of the hit's damage. | target can be shocked |
| Blood | Blood Arc | Shock jumps along a chain of bleeding enemies, up to three. | target can be shocked |
| Blunt | Thunderclap | A chance to stun together with Shock. | target can be shocked |
| Piercing | Lightning Needle | Shock jumps to the nearest other enemy; the target and that enemy are Exposed: every attacker crits them more often. | target can be shocked |

#### Base: Blood

| Tint | Name | Essence | Condition |
| --- | --- | --- | --- |
| Fire | Seething | A weaker Burning comes together with Bleeding. | target can bleed |
| Cold | Chilling Wounds | Slows the target together with Bleeding. | target can bleed |
| Lightning | Frenzied Pulse | Bleeding ticks faster. | target can bleed |
| Blunt | Blackout | Only on a target that already bleeds: it is stunned, and its Bleeding is renewed. | target is bleeding |
| Piercing | Deep Wound | Bleeding lasts longer and is stronger. | target can bleed |

#### Base: Blunt

| Tint | Name | Essence | Condition |
| --- | --- | --- | --- |
| Fire | Red-hot Hammer | Burning comes together with Stun. | target can be stunned |
| Cold | Frozen Ram | Stun lasts longer, and the target stays slowed after it. | target can be stunned |
| Lightning | Thunderstruck | Stun lasts longer. | target can be stunned |
| Blood | Dread Pulse | When Stun lands, the enemies around the target are stunned and weakened too. | target can be stunned |
| Piercing | Breach | Extra armor penetration and more damage on this hit. | target can be stunned |

#### Base: Piercing

| Tint | Name | Essence | Condition |
| --- | --- | --- | --- |
| Fire | Red-hot Point | A weak Burning without a roll (the Fire tint also adds Fire damage). | target has armor |
| Cold | Brittle Mark | The target is Exposed: every attacker's crit chance against it grows for a few turns. | target has armor |
| Lightning | Piercing Spark | Extra Lightning damage and a chance of Shock of its own. | target has armor |
| Blood | Bloodletting | A Bleeding with much more damage, without a roll. | target can bleed |
| Blunt | Crippling Point | The target is Rooted: it cannot move for a few turns, but can still act. | target has armor |

### 6.5 Triple resonances

A triple is **the skill's aspect (base) + the best tint + the second tint**. The order matters: base Cold, best tint Piercing, second Lightning is a different triple from base Cold, best tint Lightning, second Piercing. For 6 aspects that is 6 bases × 5 best tints × 4 second tints = **120 triples**.

How a triple works:

- The **pair** of the base and the best tint always gives its own effect (section 6.4).
- The triple **adds its own effect on top**: up to 3 operations from the registry table `triples` (one record of 16 bytes per triple). The record replaces the trait template of the second tint; the hit part of the second tint's trait still applies.
- "+ Shock", "+ Stun" and "+ Penetration" in a triple are **guaranteed**, without a roll. In a pair, Shock keeps its normal chance.
- Tints come only from the race predisposition and items; the aspects of one skill never tint each other.
- All numbers are in `BalanceConfig` or in the operation values.

Name: **the name of the pair with the best tint + the epithet of the second tint** (of Ash, of Frost, of the Storm, of Blood, of Ruin, of the Point).

Some triples are approximations, close to the text but not literal:

- thaw damage to neighbours lands when the Freeze lands, not when it thaws;
- "a guaranteed crit at thaw" is a strong Exposed;
- delayed states wait while the target has any Freeze or Stun, not only the named one;
- a few effects meant for neighbours also land on the target.

The mapping is in `assets/registry/triples.json`, the texts in `assets/wiki/triples.json` (mechanics) and `assets/wiki/triple_lore.json` (flavor). The wiki shows the triples under each pair.

#### Base: Fire

| Best tint | Second tint | Name | Added on top of the pair |
| --- | --- | --- | --- |
| Cold | Lightning | Pale Flame of the Storm | A short lightning arc: Burning jumps to one more adjacent enemy. |
| Cold | Blood | Pale Flame of Blood | A burn from within: the Burning damage grows with every tick. |
| Cold | Blunt | Pale Flame of Ruin | The blow breaks the guard and knocks the target back one cell; it is off guard (its defense is divided) for a while. |
| Cold | Piercing | Pale Flame of the Point | The target is Sundered: its armor against Piercing is divided (about half). |
| Lightning | Cold | Stormfire of Frost | Frost: the target and the enemy the burn jumped to are Slowed and Blinded (their d20 is divided). |
| Lightning | Blood | Stormfire of Blood | The jump becomes a blood lightning: Bleeding lands and jumps too, and the neighbours take a share of the damage. |
| Lightning | Blunt | Stormfire of Ruin | A chance to stun with the Burning, the Stun jumps to a neighbour, and the target is knocked back one cell. |
| Lightning | Piercing | Stormfire of the Point | The target (and the neighbour) are Exposed to crits, and a crit always ignites. |
| Blood | Cold | Boiling Blood of Frost | Bleeding ticks slower; the target is Slowed and Rooted. |
| Blood | Lightning | Boiling Blood of the Storm | Shock lands and jumps to every bleeding enemy within 2 cells. |
| Blood | Blunt | Boiling Blood of Ruin | Stun lands; Bleeding is refreshed and stronger. |
| Blood | Piercing | Boiling Blood of the Point | Bleeding deals its damage twice per turn. |
| Blunt | Cold | Blasting Flame of Frost | After the knockback the target slides: Slowed and off guard. |
| Blunt | Lightning | Blasting Flame of the Storm | Shock lands and jumps to every burning enemy within 2 cells. |
| Blunt | Blood | Blasting Flame of Blood | The knocked-back target leaves a bloody trail: Weakened and bleeding. |
| Blunt | Piercing | Blasting Flame of the Point | The target flies 2 cells and gets Penetration. |
| Piercing | Cold | Stinging Flame of Frost | An ice plug in the wound: the target is Slowed and Vulnerable, so the next hit hurts more. |
| Piercing | Lightning | Stinging Flame of the Storm | Shock lands; the Burning of a crit also reaches a neighbour. |
| Piercing | Blood | Stinging Flame of Blood | Strong Bleeding and Weakening. |
| Piercing | Blunt | Stinging Flame of Ruin | The target is Dazed: it uses no reactive skill. |

#### Base: Cold

| Best tint | Second tint | Name | Added on top of the pair |
| --- | --- | --- | --- |
| Fire | Lightning | Burning Frost of the Storm | The damage of the thawing Freeze also hits all neighbours (dealt at once). |
| Fire | Blood | Burning Frost of Blood | Weakening now; Bleeding starts only after the thaw. |
| Fire | Blunt | Burning Frost of Ruin | Stun, and extra damage from the broken ice against a frozen target. |
| Fire | Piercing | Burning Frost of the Point | The target is so Exposed that the next hit crits almost surely. |
| Lightning | Fire | Ringing Ice of Ash | Thermal shock: the neighbours take damage, and a weak Burning jumps to one of them. |
| Lightning | Blood | Ringing Ice of Blood | The freeze jumps further (to the enemy behind), and everyone it reaches is Weakened. |
| Lightning | Blunt | Ringing Ice of Ruin | Stun; if the target was frozen, the shards hurt every enemy next to it. |
| Lightning | Piercing | Ringing Ice of the Point | The target and the enemy the freeze jumped to are Exposed to crits. |
| Blood | Fire | Chilled Blood of Ash | The Weakening is taken off, but a weak Burning appears. |
| Blood | Lightning | Chilled Blood of the Storm | Shock chains to every enemy within 2 cells, and all of them are Weakened. |
| Blood | Blunt | Chilled Blood of Ruin | Stun lasts longer, and a Slow remains after it. |
| Blood | Piercing | Chilled Blood of the Point | Bleeding starts only when the target thaws, and hits harder. |
| Blunt | Fire | Cleaving Ice of Ash | Thermal shock (more damage) and a weak Burning on the target. |
| Blunt | Lightning | Cleaving Ice of the Storm | Shock, and the shards hurt the neighbours. |
| Blunt | Blood | Cleaving Ice of Blood | Bleeding and Weakening. |
| Blunt | Piercing | Cleaving Ice of the Point | Penetration, and the hit is a certain crit against a frozen target. |
| Piercing | Fire | Piercing Frost of Ash | When the target thaws it takes damage and a weak Burning. |
| Piercing | Lightning | Piercing Frost of the Storm | The Freeze jumps to a neighbour, and Shock lands. |
| Piercing | Blood | Piercing Frost of Blood | Strong Bleeding after the thaw, and a Weakening that lasts past it. |
| Piercing | Blunt | Piercing Frost of Ruin | The ice cracks and the target is stunned. |

#### Base: Lightning

| Best tint | Second tint | Name | Added on top of the pair |
| --- | --- | --- | --- |
| Fire | Cold | Incinerating Discharge of Frost | A Slow, and the Burning ticks less often but longer. |
| Fire | Blood | Incinerating Discharge of Blood | Shock is stronger, and the target is Weakened. |
| Fire | Blunt | Incinerating Discharge of Ruin | Stun, and the Burning spreads to a neighbour. |
| Fire | Piercing | Incinerating Discharge of the Point | The target is Exposed to crits, and a crit ignites it. |
| Cold | Fire | Ice Conduit of Ash | A weak Burning lands and jumps to a neighbour (the pair already damages the neighbours). |
| Cold | Blood | Ice Conduit of Blood | Everyone hit by the discharge is Weakened. |
| Cold | Blunt | Ice Conduit of Ruin | Stun, and the Stun spreads to the enemies around. |
| Cold | Piercing | Ice Conduit of the Point | Penetration, and the damage to the neighbours goes only along the straight line. |
| Blood | Fire | Blood Arc of Ash | A weak Burning on every bleeding enemy within 3 cells. |
| Blood | Cold | Blood Arc of Frost | Everyone in the chain is Slowed. |
| Blood | Blunt | Blood Arc of Ruin | Stun on every bleeding enemy within 3 cells. |
| Blood | Piercing | Blood Arc of the Point | Penetration on every bleeding enemy within 3 cells. |
| Blunt | Fire | Thunderclap of Ash | Burning, and the Stun lasts longer. |
| Blunt | Cold | Thunderclap of Frost | A Slow that lasts past the end of the Stun. |
| Blunt | Blood | Thunderclap of Blood | Weakening, and the Shock jumps to a neighbour. |
| Blunt | Piercing | Thunderclap of the Point | Penetration, and a crit always stuns. |
| Piercing | Fire | Lightning Needle of Ash | A weak Burning on the target and on the neighbour. |
| Piercing | Cold | Lightning Needle of Frost | Both are Slowed, and their crit chance drops (Dulled). |
| Piercing | Blood | Lightning Needle of Blood | Weakening and Bleeding on the target and on the neighbour. |
| Piercing | Blunt | Lightning Needle of Ruin | Both are Dazed, and the target is knocked back one cell. |

#### Base: Blood

| Best tint | Second tint | Name | Added on top of the pair |
| --- | --- | --- | --- |
| Fire | Cold | Seething of Frost | A Slow, and the weak Burning ticks longer. |
| Fire | Lightning | Seething of the Storm | Shock lands, and the Burning jumps to a neighbour. |
| Fire | Blunt | Seething of Ruin | Stun, and the Bleeding is refreshed. |
| Fire | Piercing | Seething of the Point | Bleeding deals its damage twice per turn. |
| Cold | Fire | Chilling Wounds of Ash | The Slow is taken off, and a weak Burning appears. |
| Cold | Lightning | Chilling Wounds of the Storm | Shock chains to every enemy within 2 cells, and all of them are Weakened. |
| Cold | Blunt | Chilling Wounds of Ruin | Stun, and a strong Slow remains after it. |
| Cold | Piercing | Chilling Wounds of the Point | Bleeding is stronger, and the target is Slowed. |
| Lightning | Fire | Frenzied Pulse of Ash | The Bleeding tick deals Fire damage: the armor no longer cuts it, Fire resistance does. |
| Lightning | Cold | Frenzied Pulse of Frost | Bleeding ticks less often, and the target is Slowed. |
| Lightning | Blunt | Frenzied Pulse of Ruin | Stun, and two Bleeding ticks at once. |
| Lightning | Piercing | Frenzied Pulse of the Point | The Bleeding ticks even more often, and Penetration lands. |
| Blunt | Fire | Blackout of Ash | Burning lands (with the Stun of the pair). |
| Blunt | Cold | Blackout of Frost | A Slow, and the Stun lasts longer. |
| Blunt | Lightning | Blackout of the Storm | Shock, and the Stun jumps to a neighbour. |
| Blunt | Piercing | Blackout of the Point | Penetration (the Stun of the pair is already certain). |
| Piercing | Fire | Deep Wound of Ash | Stronger Bleeding, but the target cannot be healed. |
| Piercing | Cold | Deep Wound of Frost | Bleeding lasts longer, and the target is Slowed. |
| Piercing | Lightning | Deep Wound of the Storm | Shock, and the Bleeding ticks twice at once. |
| Piercing | Blunt | Deep Wound of Ruin | The target is Dazed, and the Bleeding is refreshed. |

#### Base: Blunt

| Best tint | Second tint | Name | Added on top of the pair |
| --- | --- | --- | --- |
| Fire | Cold | Red-hot Hammer of Frost | A Slow, and a stronger Burning. |
| Fire | Lightning | Red-hot Hammer of the Storm | Shock, and the Burning jumps to a neighbour. |
| Fire | Blood | Red-hot Hammer of Blood | Weakening, and Bleeding along with the Burning. |
| Fire | Piercing | Red-hot Hammer of the Point | Penetration (the Burning already lands with the Stun). |
| Cold | Fire | Frozen Ram of Ash | A weak Burning; the Stun is shorter, but the blow hits harder. |
| Cold | Lightning | Frozen Ram of the Storm | Shock, and the shards hurt the neighbours. |
| Cold | Blood | Frozen Ram of Blood | Weakening; Bleeding starts after the Stun ends. |
| Cold | Piercing | Frozen Ram of the Point | A certain crit, and a Slow. |
| Lightning | Fire | Thunderstruck of Ash | Burning starts after the Stun ends. |
| Lightning | Cold | Thunderstruck of Frost | A Slow, and the Stun cannot be shaken off early: it lasts longer. |
| Lightning | Blood | Thunderstruck of Blood | Weakening, and a Shock that lasts longer. |
| Lightning | Piercing | Thunderstruck of the Point | Penetration; on a crit the Stun is certain and lasts longer. |
| Blood | Fire | Dread Pulse of Ash | A weak Burning on everyone around. |
| Blood | Cold | Dread Pulse of Frost | A Slow on the target and on everyone the wave reaches. |
| Blood | Lightning | Dread Pulse of the Storm | Shock on everyone around. |
| Blood | Piercing | Dread Pulse of the Point | Penetration on everyone around (the wave of the pair weakens them). |
| Piercing | Fire | Breach of Ash | Penetration and a weak Burning at the point of impact. |
| Piercing | Cold | Breach of Frost | A Penetration lands, and a Slow follows it (the pair already adds damage). |
| Piercing | Lightning | Breach of the Storm | Shock, and the damage passes through nearly all the armor. |
| Piercing | Blood | Breach of Blood | Weakening, and a strong Bleeding. |

#### Base: Piercing

| Best tint | Second tint | Name | Added on top of the pair |
| --- | --- | --- | --- |
| Fire | Cold | Red-hot Point of Frost | A Slow, and the next hit on this wound hurts more (Vulnerability). |
| Fire | Lightning | Red-hot Point of the Storm | Shock, and extra Lightning damage on a crit. |
| Fire | Blood | Red-hot Point of Blood | Strong Bleeding and Weakening. |
| Fire | Blunt | Red-hot Point of Ruin | The target is Dazed, and knocked back one cell. |
| Cold | Fire | Brittle Mark of Ash | The target is even more Exposed, and a crit adds a weak Burning. |
| Cold | Lightning | Brittle Mark of the Storm | Shock, and a crit is almost certain. |
| Cold | Blood | Brittle Mark of Blood | Bleeding and Weakening; the target also defends worse (off guard). |
| Cold | Blunt | Brittle Mark of Ruin | More damage, and the target is Dazed. |
| Lightning | Fire | Piercing Spark of Ash | A weak Burning, and extra damage when the target was already shocked. |
| Lightning | Cold | Piercing Spark of Frost | A Slow, and a second chance of Shock. |
| Lightning | Blood | Piercing Spark of Blood | Weakening, and a Bleeding that ticks as Lightning damage. |
| Lightning | Blunt | Piercing Spark of Ruin | The target is Dazed, and Shock can land even on a miss. |
| Blood | Fire | Bloodletting of Ash | Stronger Bleeding, but the target cannot be healed. |
| Blood | Cold | Bloodletting of Frost | Bleeding lasts longer, and the target is Slowed. |
| Blood | Lightning | Bloodletting of the Storm | Shock, and the Bleeding ticks twice at once. |
| Blood | Blunt | Bloodletting of Ruin | Stun, and the Bleeding is refreshed and stronger. |
| Blunt | Fire | Crippling Point of Ash | The target is rooted and catches a weak Burning. |
| Blunt | Cold | Crippling Point of Frost | The Root lasts longer, and a Slow follows it. |
| Blunt | Lightning | Crippling Point of the Storm | Shock, and the target cannot use skills (Silenced). |
| Blunt | Blood | Crippling Point of Blood | Weakening and Bleeding while the target is rooted. |

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
| resistance | reduced | chance lowered by the rule below; strength unchanged | work if the condition is met   |
| immunity   | none    | not applied         | **no dice are rolled**               |

**Chance of a state.** The chance is divided by the sum of two counterweights: the target's resistance to that state, and a share (`aspect_state_resist_share_bp`, now 25%) of the target's resistance to the aspect whose native state it is. Example: a wolf with Cold resistance 50% gets a 12.5% counterweight against Chill. This works for the native state of a packet, for the states of resonances and for Stun (Blunt). Freeze has no aspect of its own, so only resistance to Freeze lowers it. The strength of a state does not depend on resistance, except damage over time, which falls together with the damage.

**Tag defenses.** A target can also resist or be immune by the tags of a hit: the source of power (magical, martial) and the form (strike, projectile). Resistance to a tag adds to the resistance to the packet's aspect as one counterweight. Immunity to a tag of the hit means immunity to all its packets.

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
