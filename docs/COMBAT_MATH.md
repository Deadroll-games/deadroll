# COMBAT_MATH: combat calculation logic of Deadroll

Version 0.1, draft. This document describes the **logic** of the calculations: how values are structured, what is summed, what is multiplied, in what order an attack is calculated, and where the limits are. **There are no specific formulas or numbers in this document**, they will appear at balancing.

Related documents:

- `DAMAGE_TYPES_AND_MIXES.md`: aspects, states, resonances, reactions, tiers.
- `MAP_DESCRIPTION.md`: cells, movement, vision.

---

## 1. Principles

1. **Integers only.** No floating-point fractions: on different devices they can be calculated differently, while the on-chain result must match to the unit everywhere.
2. **No negative values.** No stat, bonus or result is ever negative. "Bad" effects do not subtract, they **divide** (section 3).
3. **Values cannot "drop to zero" or "go to infinity" from stacking.** No matter how many Slows, Weakenings or resistances are applied, the effect approaches the limit but does not cross it.
4. **Rounding once, at the very end.** Intermediate steps are calculated with spare precision, and rounding down is done only when the final number is obtained.
5. **One piece of randomness per check.** The die answers the question "did it work or not". The strength of an effect is calculated from the skill, the weapon and the modifiers into a range; the d100 only picks a point in that range (section 5).
6. **Every calculation is explainable.** Every attack leaves a full hit breakdown: which rolls, which bonuses, which defenses. This is both fairness and content for streams.
7. **One code everywhere.** The combat logic lives in one Rust crate, which is used by the on-chain program, the balance simulator and the website (a preview of "roughly how much will I deal").

---

## 2. How values are stored

| What                        | How it is stored                                      | Example                                  |
| --------------------------- | ----------------------------------------------------- | ---------------------------------------- |
| **Quantities**              | integer                                               | HP, damage, cells, duration in turns     |
| **Shares and percentages**  | integer in **basis points**: 10000 = 100%             | a 25% chance is stored as 2500           |
| **Multipliers**             | integer in basis points: 10000 = ×1                   | ×1.5 is stored as 15000                  |

Basis points give precision to hundredths of a percent without fractions. All intermediate calculations are done in numbers of large bit width, so that multiplications do not overflow.

---

## 3. Opposing pairs

Every "minus" in the game is expressed as a **separate positive value** that acts in the opposite direction.

| Positive side         | Counterweight           |
| --------------------- | ----------------------- |
| haste                 | slow                    |
| damage empowerment    | damage weakening        |
| protection            | vulnerability           |
| accuracy              | evasion                 |
| state chance boost    | state resistance        |
| healing               | healing reduction       |
| armor                 | penetration             |

**How they combine:** the positive side **multiplies**, the counterweight **divides**. The result is the ratio of "how many bonuses" to "how many counterweights".

Consequences:

- a 100% Slow makes a character half as fast, a 300% Slow makes it four times slower, but **never down to zero**;
- if Haste and Slow are equal, they cancel each other out completely;
- a huge number of applied Weakenings does not turn damage negative, damage just becomes very small.

A full stop (Freeze, Stun) is not "a Slow down to zero" but a **separate state** with an explicit "skips the action" rule (section 7.4).

---

## 4. Modifier groups

Modifiers are divided into groups. Within a group, bonuses are **summed**, and the groups themselves are **multiplied** with each other. This is the same idea as "increased" and "more" in Path of Exile.

| Group              | How it works                                                         | What goes here                                                                      |
| ------------------ | -------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| **Addition**       | adds to the base value                                               | "+N Fire damage", added damage from a tint trait                                    |
| **Increase**       | all sources are summed into one total, which is then applied once    | tiers of items and predispositions, leveling, most passive bonuses                  |
| **Multiplier**     | each source multiplies separately                                    | resonance, crit, reactions, individual rare effects                                 |
| **Counterweight**  | all sources are summed into one total, and the total divides         | Slows, Weakenings, resistances, healing reduction                                   |

Why it works this way:

- **Increase is summed**, so ten small bonuses do not blow up damage. Each new bonus feels weaker and weaker relative to the accumulated total.
- **Multipliers are rare and noticeable.** A resonance or a crit must be felt, so they multiply. But there are few of them, and each one is clearly visible in the hit breakdown.
- **Counterweight is summed and divides**, so stacking debuffs never goes into the negative.

A rule for balancing: **every new source of power we first try to put into "Increase"**. Into "Multiplier" goes only what should feel like an event (resonance, crit, reaction).

---

## 4.1 Damage packets

The damage of an attack is a set of **packets**, one per aspect: the base of the skill or weapon (for example, Lightning 5-8) and the added damage from predispositions and items (for example, Piercing 1-2).

- Each modifier applies only to packets with matching tags.
- Added packets **inherit** the power source and form of the skill (magical, strike...), but keep their own aspect and family. So "% to magical damage" affects all packets of a magical skill, while "% to Lightning damage" affects only the Lightning packet.
- Packets are **not summed before target defense**: the target has different defense against different aspects.
- As a hint to the player, ranges are summed minimum to minimum and maximum to maximum: Lightning 14-26 + Piercing 3-10 = 17-36.

---

## 5. Rolls

All rolls are made on-chain and recorded in the hit breakdown.

| Check                   | Die                                            | What it decides                                                                              |
| ----------------------- | ---------------------------------------------- | -------------------------------------------------------------------------------------------- |
| **Hit**                 | d20 + attack modifier against target defense   | hit or miss; a natural 20 is always a hit and a crit, a natural 1 is always a miss           |
| **Damage**              | d100                                           | a point in the range of each packet: 0 = minimum, 100 = maximum; one roll for the whole attack |
| **Resonance tint**      | d100 against the chance, one per each tint     | whether the tint triggered or not                                                            |
| **State application**   | d100 against the chance                        | whether the state was applied or not                                                         |

All chances (crit, tint, state) are rolled on a d100, from 0 to 100, against the chance in basis points.
| **Special checks**      | d20 against difficulty                         | trap detection, searching, resistance to an effect                                           |

Rules:

- **One piece of randomness per action.** The action requests a random value from MagicBlock Solana VRF; all rolls of the action (d20, tints, damage, states) are derived from it deterministically: seed + roll number → hash → number. The action has two steps: a request and a callback, but for the player it is one click.
- **Rolls that are not needed are not made.** A miss ends the attack; if the target is immune to the base, the tints are not rolled; if the target is immune to a state, its application is not rolled.
- **Strength is not a yes-or-no roll.** The strength of an effect is calculated from the skill, the weapon and the modifiers into a range (minimum to maximum). The d100 picks a point in that range: 0 is the minimum, 100 is the maximum. Chances (crit, tint, state) are the only yes-or-no rolls.
- **Chances always stay within limits.** Any chance is bounded by a minimum and a maximum (section 10), so there are no guaranteed or impossible events, except for explicit rules such as immunity.
- The hit modifier is D&D-style: a bonus is added to the d20 roll, and target defense is the threshold. Summing is acceptable here because all the values are positive.
- The website shows the rolls as a **cascade of animations**: the hit d20, the tint dice, the damage d100, the state dice.

---

## 6. Attack calculation order

Every attack is calculated strictly step by step. The order is fixed and the same for the program, the simulator and the website.

### 6.1 Preparation

1. **Declaration:** who attacks, with which skill, and who the target is.
2. **Rules check:** range, line of sight, the attacker's turn, the skill's tags are valid.
3. **Check of the target's immunity to the base.** If the target is immune, there is no damage of this aspect, no native state, and resonances with this base are not rolled.

### 6.2 Hit

4. **Hit roll.** A miss ends the attack (except skills with the explicit rule "deals part of the damage on a miss").
5. **Crit determination:** a natural 20 or the crit chance from stats.

### 6.3 Resonance

6. **Tint rolls.** Each candidate tint rolls its own die.
7. **Resonance selection:** a pair, a triple or nothing (rules in `DAMAGE_TYPES_AND_MIXES.md`, section 6).
8. **Resonance condition check** on the target. Not met → the resonance is cancelled.

### 6.4 Attacker's damage

9. **Packets:** the base of the skill or weapon + added packets from predispositions, items and tint traits (section 4.1).
10. **Increase** by tags, separately for each packet.
11. **Multipliers** of the attacker (resonance, crit, reactions on the attacker, for example Searing Rage) by tags, for each packet.
12. **Counterweights** of the attacker (Weakenings applied to the attacker) for each packet.
13. **Damage roll d100:** one point in the range, applied to all packets.

### 6.5 Target defense

Defense is calculated **separately for each packet**: physical packets are cut by armor, elemental packets are cut by resistance to their own aspect.

14. **Target armor** (for physical packets), reduced by the attacker's **penetration**. Armor lowers damage with diminishing returns: the more armor, the smaller the gain in defense from each new unit, and armor never gives 100% defense.
15. **Target resistance to the packet's aspect** as a counterweight.
16. **Target Vulnerabilities** as multipliers: Shock, Soaked, Brittleness, reactions.

### 6.6 Result

17. **Sum of packets** after defense and **rounding down**, once.
18. **Minimum damage:** if the attack hit and the target is not immune, damage is not less than the minimum (section 10).
19. **HP deduction.** Zero HP means the death of the character or monster.

### 6.7 Consequences

20. **Application of native states** of each packet: the chance (from the skill, tier, increase) against the target's resistance, a separate d100 for each packet.
21. **Application of resonance states**, if the resonances grant them.
22. **State collisions** (`DAMAGE_TYPES_AND_MIXES.md`, section 8): for each packet with each state that was on the target **before** the attack. Collision damage is calculated as a separate instance: only target defense, without repeated multipliers of the attacker.
23. **Reactions** on the target, if the attack applies effects to a character with a predisposition.
24. **Recording the hit breakdown** in the transaction log: all rolls, all modifier groups, resonances, collisions, the result.

---

## 7. States

### 7.1 State strength

The strength of over-time states (Burning, Bleeding) **depends on the damage of the attack** that applied them: a strong attack gives strong Burning. A state has no separate strength of its own, so there is no need to balance two independent numbers.

### 7.2 One state of each kind on a target

- A target can have **only one instance** of each state (one Burning, one Bleeding).
- A new application of the same state: **the strongest one remains**, and the duration is refreshed to the longer one.
- This prevents stacking Burnings endlessly and keeps the calculations simple.

### 7.3 Buildup

Some states build up: **Chill → Freeze**. Each Cold hit adds buildup, and when the threshold is reached the target is frozen and the buildup is reset.

### 7.4 Soft and hard control

| Type          | Example                | How it is calculated                                          |
| ------------- | ---------------------- | ------------------------------------------------------------- |
| **Soft**      | Chill, Slow            | through a counterweight to Speed (section 3)                  |
| **Hard**      | Freeze, Stun           | an explicit rule: the target skips an action or movement. For now Freeze and Stun skip the whole turn |

After hard control, the target receives **temporary tenacity** to the same control, so that it cannot be held frozen for the whole party.

### 7.5 Time

- Duration is counted in the **target's turns**, as an integer.
- Over-time damage is dealt at the **start of the target's turn**.
- Removing a state (healing, cleansing) removes the whole instance.

---

## 8. Movement and initiative

- **Speed** gives Action Points per turn, as an integer. Haste and Slow work through a ratio (section 3), the result is rounded down, but **not below the minimum** (section 10), unless there is hard control.
- **Cell cost** comes from `MAP_DESCRIPTION.md`: a step costs 1 AP, a difficult-terrain cell costs 2.
- **No more than one active skill per turn**, regardless of the amount of AP.
- **Initiative** works through the initiative timeline (`PARTY_AND_TURNS.md`, section 6): after a turn, a delay that depends inversely on Initiative is added to the participant's time until turn. A participant with high Initiative sometimes acts twice while a slow one acts once.
- The delay is calculated with integers; a tie is resolved by Dexterity, then by the order of joining the party.
- **Stun** burns the participant's turn: a delay is added to its time as after a turn.
- Outside combat the turn order is fixed, in a circle; the timeline is switched on only in combat.

---

## 9. Resonances and reactions in the calculation

| Mechanic                            | Where it goes                                                                         | Where the strength comes from            |
| ----------------------------------- | ------------------------------------------------------------------------------------- | ---------------------------------------- |
| Resonance                           | the attacker's **Multiplier** group (step 12) and/or new states (step 21)             | tiers of the triggered carriers          |
| Tint trait "add damage"             | the **Addition** group (step 10)                                                      | carrier tier                             |
| Reaction "affinity / conflict"      | a multiplier or counterweight on the effect                                           | tier of the receiver's predisposition    |
| Reaction "transformation"           | creates a state with its own rules                                                    | tier of the receiver's predisposition    |

Affinity and conflict from the two tables (relation and inclination) **naturally cancel each other out**: one multiplies, the other divides. No special cancellation rules are needed.

---

## 10. Minimums and caps

| Value                          | Limit                        | Why                                                     |
| ------------------------------ | ---------------------------- | ------------------------------------------------------- |
| Damage on a hit                | not less than the minimum    | any hit is felt                                         |
| Damage under immunity          | zero                         | immunity is absolute                                    |
| Speed without hard control     | not less than the minimum    | a character can always move at least a little           |
| Any chance                     | between minimum and maximum  | no guaranteed or impossible events                      |
| Resistance                     | not above the cap            | without immunity the target is always at least slightly vulnerable |
| Defense from armor             | never 100%                   | diminishing returns                                     |
| State duration                 | not above the cap            | no eternal control                                      |
| HP                             | from zero to maximum         | healing does not exceed the maximum                     |

**Immunity** is a separate flag, not "100% resistance". This way resistance can be limited by a cap without breaking immunities.

---

## 11. Precision and overflow

- **Multiply first, then divide.** Division in the middle loses precision.
- **Large bit width** for intermediate values, so that a chain of multipliers does not overflow.
- **Saturation instead of overflow:** if a value hits the maximum of its type, it stays at the maximum instead of "wrapping around".
- **Rounding is always down** and always in one place (step 17).

---

## 12. Hit breakdown

Every attack writes a structured breakdown to the transaction log:

- rolls: hit, crit, tints, states;
- which resonances and reactions triggered and why (tiers, conditions);
- the sum of each modifier group;
- target defense: armor, penetration, resistance, vulnerabilities;
- the final damage and the applied states.

The website shows it as an expandable row in the combat log (like the damage breakdown in PoE), and the stream gets a clear picture of "why this attack dealt this much".

---

## 13. Where the code lives

| Where                   | What it does                                                                                              |
| ----------------------- | --------------------------------------------------------------------------------------------------------- |
| Crate `deadroll-combat` | all the logic of this document, with no dependencies on Anchor, integers only                             |
| On-chain program        | calls the crate for every attack                                                                          |
| Balance simulator       | runs the crate millions of times (Monte Carlo) for balance: average damage, combat length, dangerous builds |
| Website                 | calls the crate through WASM for the preview and the hit breakdown                                        |
