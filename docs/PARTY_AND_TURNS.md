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
    refunding the remaining prepayment, closing session keys.
    The DM issues the rewards to all players and takes the escrow in **one transaction signed by the main wallet** (the DM's session key is refused). The DM gives the map a deadline for this (from 10 minutes to 10 hours, set at map creation). After the deadline each player claims their own rewards and experience with their main wallet and gets their contribution back.

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
- The DM receives the escrow in the same transaction that issues the rewards (section 2, step 10), within the deadline set on the map.
- If the DM abandons the party (reveal timeouts, no audit, or no issue before the deadline), the contributions are **returned to the players** proportionally, and the DM loses reputation.
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
- Experience is awarded for each defeated pack (`CHARACTER.md`, section 5). Currently it is given for **each killed monster**: the full amount goes at once into the temporary cache of every living character, it is not divided. Awarding it per pack comes later.

### 4.1 Planned: arena combat (big update)

**Planned, not implemented.** The text above (section 4) describes how the game works now and stays valid until this update.

- There are two modes: on the map (outside combat) and in combat.
- When combat starts, a **mini-arena** opens (about 10×10 cells). Players and monsters are placed on it and move the same way as outside combat.
- `move_path` is the same instruction on the map and in the arena: the player marks several cells in a row and walks along them.
- On the map only **one monster per pack** is visible. The whole pack appears in the arena when combat starts.

---

## 5. Character turn

### 5.1 Action points

At the start of its turn a character receives **action points (AP)** according to its speed (`CHARACTER.md`, section 3.1).

| Action                                 | Cost                              | Limit                                                       |
| -------------------------------------- | --------------------------------- | ----------------------------------------------------------- |
| Step onto a cell                       | 1 AP (difficult-terrain cell 2)   | to any of the 8 neighbours; a diagonal costs the same as a straight step; no corner cutting (section 5.1.1) |
| Active skill                           | its own AP cost                   | **no more than one per turn**                               |
| Basic attack                           | its own AP cost                   | **either a basic attack or an active skill** per turn, not both |
| Interaction (door, chest, search)      | its own AP cost                   | a chest only from an adjacent cell (section 9.3.1)          |
| Escape                                 | the whole remaining turn          | combat only, needs unanimity (section 7.3)                  |
| End turn                               | 0                                 |                                                             |

Unspent AP are lost at the end of the turn.

#### 5.1.1 Diagonal steps

A character may step to any of the 8 neighbouring cells, and a diagonal step costs the same as a straight one. **Cutting a corner is forbidden**: a diagonal step is not allowed if either of the two cells beside it is impassable according to the `Tileset`. The same rule applies to the path checks of the map builder (`MAP_DESCRIPTION.md`, section 10.1).

Haste changes the AP of a turn, not the order of turns.

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

A character at 0 HP leaves the fight at once: they do not take turns, cannot be targeted and do not act. The tombstone is made out by the next `start_turn` or by the rewards; until then nothing else happens in the party.

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
- If a character dies in a new party before the rewards of an earlier party have been issued, the rewards of the earlier party are lost too.

---

## 8. Temporary cache

- Each character in the party has their own **temporary cache** in `Participation`.
- What accumulates there: experience for each **killed monster**, and rewards from chests, hidden stashes and monsters.
- The contents become the character's property only on **victory** or **peaceful exit**, after the DM's final audit.

### 8.1 After the party ends

- The character is **free right after the party ends**: the owner can release it from the finished party, join a new one, change gear or lay out the cache. The tombstone of a dead character stays locked.
- The rewards of the finished party wait in a separate cache record of the character, **one record per party, up to 3 waiting parties** per character. A fourth join is rejected until one of them is issued or its DM deadline has passed. This replaces the earlier rule that the cache of an old party burns when the character enters a new one.
- Issuing only makes the cache the player's property. **Nothing is put into the bag automatically.**

### 8.2 Laying out the cache ("Done")

The player lays the cache out on the site and signs "Done" with the main wallet in **one transaction**. The site offers:

- take from the cache, also part of a stack, and top up an existing stack up to 10 units;
- replace: what is taken out of the bag goes into the cache in its place;
- rearrange slots, split a stack, throw an item out of the bag (with a confirmation).

Rules checked by the program (the rewards are rolled only by the program, with VRF, and written into the cache by the program itself):

- for each resource id: bag after + cache after = bag before + cache before - thrown away;
- one kind of resource per slot, up to 10 units in a slot, up to 40 resource slots;
- consumables only in the consumable slots (up to 5 units in a stack), everything else only in the resource slots.

"Done" **closes the cache**: whatever was not taken burns, and the rent of the cache record returns to the owner. There is no timeout for the remainder, but an issued cache that was not closed with "Done" burns when the character joins the next party.

The slot **lock** in the bag exists only on the site (the browser), can be switched on and off at any moment, and is separate from the lock at Ready.

The site shows the summary screen right after victory, and an unclosed cache on the character page.

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

#### 9.3.1 Visible chests

- A visible chest is on the map from the start; its contents are rolled when it is opened. A closed chest occupies its cell, an open one can be walked through.
- A chest is opened from an adjacent cell (distance 1 in any direction, including diagonals) for AP, **once per party**. The reward is rolled for every living character (section 9.3).
- A chest cannot be opened if a living monster is within the **chest radius** of the chest or of the character who opens it. The DM sets the radius on the map (1 to 10 cells), and it is checked from the chest and from every living player.
- On hover the site shows the range of levels of the chest's table.

### 9.4 Boss guarantee

The boss table has at least 2 levels.

If the **base level** of the table dropped on the boss reward for the party, every participant **additionally** gets one reward **one level higher**. It can be a resource, recipe or blank, not necessarily for a weapon.

### 9.5 Experience

- Experience is awarded for each **killed monster**, in full to every living participant (it is not divided), accumulates in the cache, and is added to the character at the end. Awarding it per pack comes later (section 4).
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
