# PARTY_AND_TURNS: party, turns and rewards in Deadroll

Version 0.1, draft. This document covers the party lifecycle, the exploration and combat modes, action points, the initiative timeline, party outcomes and reward distribution. **There are no numbers in this document**, they will appear at balancing.

Related documents:

- `CHARACTER.md`: speed, action points, initiative.
- `COMBAT_MATH.md`: hit calculation, durations, cooldowns.
- `MAP_DESCRIPTION.md`: map, secrets, DM obligations, final audit.
- `GAME_COMMISSIONS.md`: session keys, fee prepayment.
- `SKILL_BUILDER.md`: skill slots, cooldowns.

---

## 1. Principles

1. **The game is always turn-based.** In both exploration and combat, characters act strictly in turn. There is no "mess" where everyone moves at once.
2. **The fast move farther, the high-initiative take turns more often.** Speed gives action points per turn, initiative determines how often a turn comes.
3. **One active skill per turn.** No matter how many action points there are, there are no double spells. Speed gives mobility, not extra damage.
4. **Loot is not issued until the end.** Experience and rewards accumulate in the temporary cache and become yours only on victory or peaceful exit.
5. **Rewards are set by the game, not by the DM.** The DM picks reward tables from those allowed for the tier, and the program checks the budget.
6. **Nothing drops finished.** Rewards are resources, recipes and blanks. Finished items are made by player artisans.

---

## 2. Party lifecycle

1. The DM creates a map for the party and finalizes it (MAP_DESCRIPTION.md)
2. The DM publishes a quest in the Tavern: map, tier, party price,
   optionally a private party code
3. Players take the quest and make contributions to the party price (section 3)
4. The price is collected, the minimum number of players is reached: the party moves to the status "assembled"
5. The DM and players agree on a time (for example, in Discord);
   the DM records the start time in the party
6. Launch (section 2.1): the DM opens the lobby, players mark themselves ready
7. Party: exploration ↔ combat
8. End: victory, peaceful exit, escape, defeat
9. The DM's final audit: reveal of all declared secrets
10. Issuing rewards from temporary caches, paying the DM,
    refunding the remaining prepayment, closing session keys

### 2.1 Party launch

1. **Start time.** The DM records the agreed time in the party. The site shows it in the "My Parties" section to all participants.
2. **Lobby.** At the appointed time the DM opens the lobby (a DM transaction). The DM's client must have the map's secrets file, otherwise it cannot reveal cells.
3. **Ready.** Each player presses "Ready" in the "My Parties" section. One main wallet transaction:
   - creates a session key and the fee prepayment (`GAME_COMMISSIONS.md`);
   - records the skill selection on the map: 4 active, 1 reactive, 1 passive (`SKILL_BUILDER.md`, section 3.4);
   - freezes the character's bag (`ITEMS_AND_CRAFT.md`, section 6.3).
4. **Start.** When everyone is ready, the DM starts the party. The map opens for all participants.

There is no need to enter the map's hash or address by hand: the player's participation is already recorded on-chain at payment, and the site finds their parties by itself.

### 2.2 Private parties

The DM can publish a quest with a **private party code**. Such a quest is not visible in the public Tavern, and a contribution can be made only with the code. This is for groups of friends who play with their own roster.

---

## 3. Party price and DM payment

### 3.1 Party price

- The DM sets the **party price** personally: how much they want for running the game (for example, 1 SOL).
- The minimum is 0.01 SOL, **there is no maximum**: parties that are too expensive simply will not gather players.
- Players make contributions in any shares: four players at 0.25 SOL each, or one player paying 1 SOL for everyone.
- Contributions are held in the **party escrow** (PDA), not by the DM.
- **The party starts** when the whole price is collected and the map's minimum number of players is reached.
- The party price is a separate amount, **not** the fee prepayment from `GAME_COMMISSIONS.md`.

### 3.2 When the DM is paid

- Only after the **final audit**: the DM has revealed all declared secrets and the program has verified them (`MAP_DESCRIPTION.md`).
- If the DM abandons the party (reveal timeouts, no audit), the contributions are **returned to the players** proportionally, and the DM loses reputation.
- If the party is never filled, the contributions are returned.

### 3.3 DM reputation

The DM gains reputation for every party completed honestly. Reputation is visible in the Tavern when choosing quests. In phase 1 a DM stake will be added to it.

This does not violate the brief's principle "the project has no income except the market fee": payment goes from players to the DM, and the project takes nothing from it.

### 3.4 Open point

What happens to the contribution of a player who left the party early or lost connection is not decided yet.

---

## 4. Modes

| Mode            | When                                | Turn order                                                                                      |
| --------------- | ----------------------------------- | ----------------------------------------------------------------------------------------------- |
| **Exploration** | no monster pack is in combat        | in a circle: each living character acts once, the order is fixed on entry by initiative         |
| **Combat**      | at least one pack has entered combat | initiative timeline (section 6)                                                                 |

- **Entering combat:** as soon as any monster of a pack comes into the vision of any character, or an ambush trap triggers, the **whole pack** enters combat (`BESTIARY.md`, section 2.1). All characters and the pack's monsters get a time until turn set by initiative.
- **Other packs** join a combat already in progress the same way: when any of their monsters comes into the party's vision. An incautious party can draw two packs at once.
- **Leaving combat:** all monsters in combat are dead. The mode returns to exploration, and cooldowns keep counting in turns.
- Experience is awarded for each defeated pack (`CHARACTER.md`, section 5).

---

## 5. Character turn

### 5.1 Action points

At the start of its turn a character receives **action points (AP)** according to its speed (`CHARACTER.md`, section 3.1).

| Action                                 | Cost                              | Limit                                                       |
| -------------------------------------- | --------------------------------- | ----------------------------------------------------------- |
| Step onto a cell                       | 1 AP (difficult-terrain cell 2)   |                                                             |
| Active skill                           | its own AP cost                   | **no more than one per turn**                               |
| Basic attack                           | its own AP cost                   | **either a basic attack or an active skill** per turn, not both |
| Interaction (door, chest, search)      | its own AP cost                   |                                                             |
| Escape                                 | the whole remaining turn          | combat only, needs unanimity (section 7.3)                  |
| End turn                               | 0                                 |                                                             |

Unspent AP are lost at the end of the turn.

### 5.2 In exploration

The same AP apply, but without damage skills: steps, interactions, searching, buffs.

### 5.3 Turn timeout

- If a player does not act for N seconds, **any participant** can call a skip of their turn.
- The DM's monsters follow the same rule: if the DM does not move a monster, its turn is skipped.
- Without this, one player who stepped away would hold up the whole party.

---

## 6. Initiative timeline (combat)

### 6.1 How it works

- Every combat participant (characters and monsters) has a **time until turn**.
- The one with the **lowest** time acts.
- After its turn, a **delay** is added to its time, inversely dependent on its initiative: the higher the initiative, the smaller the delay.
- This is why a character with high initiative sometimes acts **twice** while a slow one acts once.

### 6.2 Rules

- All values are integers (`COMBAT_MATH.md`).
- Tie: higher Dexterity wins, then the order of joining the party.
- Effects on initiative (Cold Mind, Slow) change the delay starting from the **next** turn.
- Stun: the participant loses their turn, and the delay is added to their time as after a turn.
- Skill cooldowns and state durations are counted in the participant's **own turns**.

### 6.3 Speed and initiative together

|                | Gives                                              |
| -------------- | -------------------------------------------------- |
| **Speed**      | how many AP per turn: how far to go and what to fit in |
| **Initiative** | how often a turn comes                             |

Mage: low speed, high initiative (Intelligence). Acts often, but does not go far. Elf Warrior: high speed, normal initiative. Acts less often, but runs half the map in one turn.

---

## 7. Party outcomes

### 7.1 Table

| Outcome              | Condition                                                          | Experience      | Rewards                                  | Penalty                                        |
| -------------------- | ------------------------------------------------------------------ | --------------- | ---------------------------------------- | ---------------------------------------------- |
| **Victory**          | the boss is killed                                                 | all accumulated | the entire temporary cache + boss reward | none                                           |
| **Peaceful exit**    | outside combat, living characters on the exit cell, "Leave" button | all accumulated | the entire temporary cache               | penalty on the next dungeons (section 7.2)     |
| **Escape**           | in combat, all living participants chose "Escape"                  | **nothing**     | **nothing**, temporary cache abandoned   | none                                           |
| **Death**            | HP = 0                                                             |                 |                                          | tombstone, items burn                          |

The party ends when there are no living characters left on the map (all died, escaped or exited) or the boss is killed.

### 7.2 Peaceful exit

- Possible only **outside combat**.
- Characters receive the accumulated experience and rewards.
- **Penalty** for this character's **next 2 dungeons**:
  - experience gained is reduced by 90%;
  - the chance of rewards above the table's base level is reduced by 80% (section 9.3).
- If the character **kills the boss** in the next dungeon, the remaining penalty is shortened to one dungeon.
- The penalty is **cumulative**: a new peaceful exit during a penalty adds another 2 dungeons to the remaining ones (was 1, becomes 3).
- This prevents abusing "grab the easy loot and leave before the boss".

### 7.3 Escape

- Escape is a decision of the **whole group**. Each living participant, on their turn, chooses the "Escape" action: this sets their **intent** to flee and spends their whole turn.
- When **all living** participants have the intent, the party flees: everyone leaves the map and **abandons their temporary caches**, with no experience and no rewards.
- If any living participant takes any other action on their turn, all intents are **reset** and the escape counts as failed; spent turns are not returned.
- The dead do not take part in the decision. Example: in a party of four the tank has died, the other three choose to escape and leave.
- There is no roll for escape: unanimity is the check.

### 7.4 Death

- A dead character becomes a tombstone (`CHARACTER.md`, section 11), and their temporary cache burns.
- The party continues without them.

---

## 8. Temporary cache

- Each character in the party has their own **temporary cache** in `Participation`.
- What accumulates there: experience for each **won combat**, and rewards from chests, hidden stashes and monsters.
- The contents become the character's property only on **victory** or **peaceful exit**, after the DM's final audit.

---

## 9. Rewards

### 9.1 What drops at all

**Finished items do not drop.** Everything is made by player artisans. Rewards are:

- **resources** (wood, iron, leather, stones...);
- **recipes**;
- **uncut skills** and blanks for lenses;
- enchantment **reagents**.

Rewards drop **regardless of class**: a Mage can get resources and an axe recipe and become a melee mage through crafting.

The **NPC artisan** in the Artisans' Guild is needed so that the game works while there are few player artisans: they craft **only low-level items** and **only weapons and equipment** (not jewelry). Everything else is made by player artisans (`ITEMS_AND_CRAFT.md`, section 8.4).

### 9.2 Map reward budget

- Rewards are set by the game: each map tier has a set of **allowed reward tables** and a **budget**.
- Each table has a **reward level range**: from the base level to the maximum level that can drop from it. This is the same level system as for items (`ITEMS_AND_CRAFT.md`, section 4).
- When creating a map, the DM assigns tables to chests, hidden stashes and the boss. Each table costs budget points, and tables with higher levels cost more.
- A table with levels above the tier ceiling is **not in** the allowed list: the program will not accept the map. A top-level reward on a first-tier map is impossible.
- The budget grows with party size, so that the reward **per person** stays stable.

### 9.3 Reward roll

Each reward event (chest, hidden stash, boss) is rolled as follows:

1. The **reward level** is rolled **once for the whole party** within the table's range: everyone gets a reward of the same level. Usually the base level drops, higher ones less often.
2. **The reward itself** is rolled **separately for each** living participant from the table of that level: each gets their own, pure chance.
3. The result goes into each one's temporary cache.

The peaceful exit penalty (section 7.2) lowers the chance of levels above the base level for that account.

### 9.4 Boss guarantee

If the **base level** of the table dropped on the boss reward for the party, every participant **additionally** gets one reward **one level higher**. It can be a resource, recipe or blank, not necessarily for a weapon.

### 9.5 Experience

- Experience is awarded for each **won combat**, accumulates in the cache, and is added to the character at the end.
- A character whose level is noticeably higher than the map tier gets less experience or none, and no rewards are rolled for them.
- The counterweight to the peaceful exit penalty applies here.

---

## 10. What is stored

### 10.1 `GameSession`

| Group        | Fields                                                                              |
| ------------ | ----------------------------------------------------------------------------------- |
| References   | map, DM, contribution escrow                                                        |
| Mode         | exploration / combat                                                                |
| Queue        | exploration order; in combat, each participant's time until turn                    |
| Participants | characters and their positions, monsters (template, HP, position, skills)           |
| Map          | `explored`, `revealed`, revealed secrets, opened doors, DM obligations              |
| Time         | turn number, timestamp of the current turn's start (for the timeout)                |
| Status       | in progress / victory / finished                                                    |

### 10.2 `Participation`

| Group           | Fields                                                    |
| --------------- | --------------------------------------------------------- |
| References      | player, character, party, session key                     |
| Skills          | chosen 4 + 1 + 1                                          |
| Combat          | current HP, states, cooldowns, AP of the current turn     |
| Temporary cache | accumulated experience, rewards                           |
| Contribution    | amount, refund status                                     |
| Outcome         | in party / exited / escaped / dead                        |
