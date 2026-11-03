# PARTY_AND_TURNS: party, turns and rewards in Deadroll

Version 0.1, draft. This document covers the party lifecycle, the map and arena modes, action points, the initiative timeline, party outcomes and reward distribution. **There are no numbers in this document**, they will appear at balancing.

Related documents:

- `CHARACTER.md`: speed, action points, initiative.
- `COMBAT_MATH.md`: hit calculation, durations, cooldowns.
- `MAP_DESCRIPTION.md`: map, secrets, DM obligations, final audit.
- `GAME_COMMISSIONS.md`: session keys, fee prepayment.
- `SKILL_BUILDER.md`: skill slots, cooldowns.

---

## 1. Principles

1. **The game is always turn-based.** On the map and in an arena, characters act strictly in turn. There is no "mess" where everyone moves at once.
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
4. The price is collected, the minimum number of players is reached: the party moves to the status "assembled" (**later**: there is no such status yet)
5. The DM and players agree on a time (for example, in Discord);
   the DM records the start time in the party (**later**: there is no start time yet)
6. Launch (section 2.1): players mark themselves ready in the lobby
7. Party: map ↔ arena (section 4)
8. End: victory, peaceful exit, escape, defeat
9. The DM's final audit: reveal of all declared secrets
10. Issuing rewards from temporary caches, paying the DM,
    refunding the remaining prepayment, closing session keys.
    The DM issues the rewards to all players and takes the escrow in **one transaction signed by the main wallet** (the DM's session key is refused). The DM gives the map a deadline for this (from 10 minutes to 10 hours, set at map creation). After the deadline each player claims their own rewards and experience with their main wallet and gets their contribution back.

### 2.1 Party launch

1. **Start time.** **Later**, not built yet: the DM records the agreed time in the party, and the site shows it to all participants.
2. **Lobby.** The lobby opens when the DM publishes the quest: `session.open_session` and `city.post_quest` go in one transaction signed by the DM's main wallet (`CITY_DESCRIPTION.md`, section 4). Taking the quest is `session.join_session`. The quest window on the site holds the lobby: join with a share of the price, Ready, Start, enter the dungeon. The DM's client opens the map's secrets with the DM's wallet (`MAP_DESCRIPTION.md`, section 17.5).
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
- The player sets their contribution. By default the site offers the rest of the price divided among the players still missing.
- Contributions are held in the **party escrow** (PDA), not by the DM.
- **The party starts** when the whole price is collected and the map's minimum number of players is reached.
- The party price is a separate amount, **not** the fee prepayment from `GAME_COMMISSIONS.md`.

### 3.2 When the DM is paid

- Only after the **final audit**: the DM has revealed all declared secrets and the program has verified them (`MAP_DESCRIPTION.md`).
- The DM receives the escrow in the same transaction that issues the rewards (section 2, step 10), within the deadline set on the map.
- If the DM abandons the party (reveal timeouts, no audit, or no issue before the deadline), the contributions are **returned to the players** proportionally, and the DM loses reputation.
- If the party is never filled, the contributions are returned.

#### 3.2.1 A missing DM

The secrets open only with the DM's wallet (`MAP_DESCRIPTION.md`, section 17.5), so a party never goes on without its DM. When the DM stops answering during the party:

1. **Mark.** The DM owes a reveal (`MAP_DESCRIPTION.md`, section 8.5: a cell a step or a look uncovered, or the composition of the pack the party engaged) for longer than the reveal timeout (`BalanceConfig`). Any living player can then mark the DM as missing. Not while a roll is pending: the DM cannot reveal until it resolves.
2. **Freeze.** Nothing is paid at that moment. The mark starts the countdown of the map's issue deadline (the same interval the DM has to issue the rewards after a party, section 2, step 10). While it runs the party is frozen: no moves, no turns, no reveals.
3. **The DM comes back** before the countdown ends and signs "I'm back". Every living player then has the return confirmation time (`BalanceConfig`) to press "Return to the party".
   - If all living players confirm, the party goes on where it stopped: the DM reveals what is owed, and the reveal timeout counts again from that moment.
   - If not all confirm in time, the party ends. The DM settles it with the main wallet before the map's issue deadline, counted from the end of the confirmation time: every living character gets their whole temporary cache (experience and rewards, as on a peaceful exit, without its penalty), every player gets their contribution back, the DM gets **no escrow**, only the map's deposits back. After that deadline each player claims their own, as below.
4. **The DM does not come back** before the countdown ends: the party ends. Each player claims with their main wallet: the living character's whole temporary cache, as on a peaceful exit without its penalty, and their contribution back. The DM gets no escrow; the map's deposits go back to the DM with the last claim.
5. Dead characters are finalized as usual on every path (tombstone, their cache burns).
6. The final audit is not needed on these paths: it is the condition of the DM's payment, and the DM is not paid.
7. **Later:** the DM's reputation (section 3.3) is not built yet; a party the DM left will cost reputation.

### 3.3 DM reputation

The DM gains reputation for every party completed honestly. Reputation is visible in the Tavern when choosing quests. In phase 1 a DM stake will be added to it.

This does not violate the brief's principle "the project has no income except the market fee": payment goes from players to the DM, and the project takes nothing from it.

### 3.4 Open point

What happens to the contribution of a player who left the party early or lost connection is not decided yet.

---

## 4. Modes

The game has two modes, **map** and **arena**. Map side: `MAP_DESCRIPTION.md`, section 17. Experience is awarded for each defeated pack (section 4.1.4; `CHARACTER.md`, section 5).

### 4.1 Map and arena

#### 4.1.1 Two modes

| Mode      | When                        | Turn order                                                                 | Monsters                          |
| --------- | --------------------------- | -------------------------------------------------------------------------- | --------------------------------- |
| **Map**   | outside combat              | in a circle: each living character acts once                               | stand still, the DM does not move them |
| **Arena** | a fight with one pack       | initiative timeline (section 6), a tie goes to the higher Dexterity        | the DM moves them                 |

`move_path` is the same instruction on the map and in the arena: the player marks several cells in a row and walks along them.

#### 4.1.2 Packs on the map

- Only **one monster of each pack** is visible on the map; the DM picks which one. The rest of the pack is hidden (`MAP_DESCRIPTION.md`, section 17.5).
- The site shows the pack's **threat level**. The DM declares it openly; the program checks it when the pack is revealed, and a mismatch rejects the reveal. The threat is the sum of the monster levels, an elite counted 2 times, a boss 3 times.
- A pack has a **facing** set by the DM.
- A map holds at most **12 different monster templates** in all its packs; the party copies them once when the session opens.

#### 4.1.3 Start of combat

Combat starts when:

- a character steps onto a cell **next to** a visible monster (within 1 cell, diagonals included); or
- an **ambush trap** triggers: the DM links a hidden pack with its arena to it, and the monsters move first.

Then:

- The **whole living party** goes into the pack's arena. Any signer may send the entering instruction.
- **Where the party stands:** at the edge of the arena the character came from. The approach direction, seen from the visible monster, picks the edge on each axis (a character to the west of the monster comes in from the west edge); on an axis where the character stood level with the monster the party comes in at the middle, and a diagonal approach gives a corner. After an ambush trap the party comes in from the edge the pack's facing names.
- **Who moves first:** the side of the approach decides.
  - The **back** is the 3 cells behind the visible monster (against its facing): the party moves first.
  - The **front** and the **sides** are the other 5 cells: the monsters move first. A side counts as the front.
  - After an ambush trap the monsters move first.
  - A pack that is **on guard** after a retreat moves first from any side (section 7.5).
- The arena is drawn by the DM for each pack in the builder; the DM picks its size within the limits of `BalanceConfig` (6 to 12 cells a side). An arena may hold traps and secrets.
- **One fight is one pack.** The map waits while the arena is played; other packs do not join.
- Everything carries over between the map and the arena, both ways: HP, states, buffs, cooldowns.

**Large monsters.** A monster of size 2×2 (`BESTIARY.md`, section 2) stands on its whole square in its arena; on the map the visible monster stays one cell.

- Distance, reach, adjacency, auras and reactive triggers are measured from the **nearest cell** of the square to the other unit.
- A line hits a large monster **once**, however many of its cells the line crosses.
- Knockback moves the **whole square**.
- Movement: a step costs the most expensive cell the square enters, and a diagonal step needs both side squares open (no corner cutting, section 5.1.1).

#### 4.1.4 End of combat

- **Victory** (the pack is dead): the arena is won. The kills not yet paid get their body rewards, then the pack's experience is paid, and the pack disappears from the map; the characters return to the cells they stood on before the fight. Any signer may send the leaving instruction.
- **Experience** for a won arena is the **sum of the experience of the pack's monsters**. It goes to every living character in full (it is not divided) into their temporary cache.
- A **boss pack** ends the party with Victory (section 7.1) and also pays its pack experience.
- **Retreat** and **Escape**: section 7.5.

#### 4.1.5 Later

- A boss calling allies during combat: a reactive skill with a limit of uses, for some bosses, in a separate skills update (`BESTIARY.md`, section 6).

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
| Retreat, Escape                        | proposing or confirming ends the turn | arena only, proposed by one, confirmed by the others (section 7.5) |
| End turn                               | 0                                 |                                                             |

Unspent AP are lost at the end of the turn.

#### 5.1.1 Diagonal steps

A character may step to any of the 8 neighbouring cells, and a diagonal step costs the same as a straight one. **Cutting a corner is forbidden**: a diagonal step is not allowed if either of the two cells beside it is impassable according to the `Tileset`. The same rule applies to the path checks of the map builder (`MAP_DESCRIPTION.md`, section 10.1).

Haste changes the AP of a turn, not the order of turns.

### 5.2 On the map

The same AP apply, but without damage skills: steps, interactions, searching, buffs.

### 5.3 Turn timeout

- If a player does not act for the turn timeout (`BalanceConfig`), **any participant** (a player of the party or the DM) can call a skip of their turn.
- The DM's monsters follow the same rule: if the DM does not move a monster, its turn is skipped.
- The timeout counts from the start of the turn or the unit's last action. Waiting for a VRF value or for the DM's reveals does not count: the timeout starts again when the wait ends.
- A skip is not the player's action: a Retreat or Escape proposal on the table stays (section 7.5).
- No skip while a roll is pending, while the DM owes a reveal, while the party is frozen by a missing DM (section 3.2.1), or between the map and an arena.
- Without this, one player who stepped away would hold up the whole party.

#### 5.3.1 A silent VRF oracle

- Every action with dice waits for one VRF value (`GAME_COMMISSIONS.md`). If the oracle does not answer for the VRF retry wait (`BalanceConfig`, in slots), **any participant** can request a new value for the same action and pays its fee.
- Each request has a number; the answer to an older request is refused, so an action resolves once, from one value.

---

## 6. Initiative timeline (combat)

The timeline runs in the arena (section 4.1). The side that moves first is set by the approach (section 4.1.3).

### 6.1 How it works

- Every arena participant (characters and monsters) has a **time until turn**.
- The one with the **lowest** time acts.
- After its turn, a **delay** is added to its time, inversely dependent on its initiative: the higher the initiative, the smaller the delay. The delay is `1200 / (6 + initiative)`, at least 1 (`BalanceConfig.arena`: `time_scale`, `initiative_base`).
- At the start of the arena the side that moves first has time 0; every unit of the other side starts with its own delay, so the first side acts before any unit of the other.
- This is why a character with high initiative sometimes acts **twice** while a slow one acts once.

### 6.2 Rules

- All values are integers (`COMBAT_MATH.md`).
- Tie: the higher Dexterity wins (a monster has no Dexterity, its initiative counts instead), then the order of joining the party.
- Effects on initiative change the delay starting from the **next** turn. The delay is taken at the **start** of the unit's turn, under its effects and auras at that moment; an effect that lands later in the same turn changes only the delay after its next turn.
- **Slow** divides the bearer's initiative as a counterweight (`COMBAT_MATH.md`, section 3), so its turns come less often. The share of the Slow that counts is `slow_initiative_bp` in `BalanceConfig.arena`. Haste does not change the order of turns (section 5.1). Chill acts through speed only.
- Stun and Freeze: the participant loses their turn, and the delay is added to their time as after a played turn.
- A turn skipped by the timeout (section 5.3) adds the bare delay, without the unit's effects, because its turn never started. If the turn's start already ran, its delay was taken then, under the unit's effects, and the skip adds nothing more.
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
| **Escape**           | in an arena, all living participants chose "Escape" (section 7.5)  | **nothing**     | **nothing**, temporary cache abandoned   | none                                           |
| **Retreat**          | from the arena back to the map (section 7.5)                       | none for this fight | the cache stays                      | the pack heals, is on guard and stronger       |
| **DM left**          | the DM was marked missing and did not come back, or came back without every living player's confirmation (section 3.2.1) | all accumulated | the entire temporary cache, contributions back | none for the players; the DM gets no escrow |
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

- Escape is a decision of the **whole group**, taken in an arena the way of section 7.5: one living participant proposes it, the others confirm on their turns.
- When **all living** participants have voted and the monsters have moved once more, the party flees: everyone leaves the map and **abandons their temporary caches**, with no experience and no rewards.
- If any living participant takes any other action on their turn, the proposal is **cancelled**; spent turns are not returned.
- The dead do not take part in the decision. Example: in a party of four the tank has died, the other three choose to escape and leave.
- There is no roll for escape: unanimity is the check.

### 7.4 Death

- A dead character becomes a tombstone (`CHARACTER.md`, section 11), and their temporary cache burns.
- The party continues without them.
- If a character dies in a new party before the rewards of an earlier party have been issued, the rewards of the earlier party are lost too.

### 7.5 Retreat and Escape from the arena

Both are possible only in an arena, not on the map.

| Action      | Where to                             |
| ----------- | ------------------------------------ |
| **Retreat** | from the arena back to the map       |
| **Escape**  | out of the dungeon (as in section 7.3: no experience, no rewards) |

How it is decided:

1. One player proposes it with a button on their turn. Proposing counts as their vote and ends their turn.
2. The other living players confirm on their turns. A character who confirms skips the rest of their turn.
3. Any other action on a player's turn (a move, an attack, a skill, a chest, ending the turn) cancels the proposal. A vote for the other action replaces the proposal.
4. When every living player has voted, each living monster of the arena moves once more; the players take no turns meanwhile. If the pack dies during these moves, the arena is won instead.
5. The survivors leave.

After a **retreat**:

- the characters return to the cells they stood on before the fight;
- no experience for this fight, not even for the monsters killed; the rewards already in the cache stay;
- the monsters killed in this fight stay dead; the rest of the pack heals fully (HP, states, buffs and cooldowns are reset when the party comes back);
- the pack is **on guard**: coming from its back no longer works, the monsters move first, for the rest of the party;
- the pack is **stronger** in its next fight: a buff from `BalanceConfig` (Empowerment and Protection for a number of the monsters' own turns);
- the number of retreats per party is limited (`BalanceConfig`).

---

## 8. Temporary cache

- Each character in the party has their own **temporary cache** in `Participation`.
- What accumulates there: experience for each **won arena** (section 4.1.4), and rewards from chests, hidden stashes and monsters.
- The contents become the character's property only on **victory** or **peaceful exit**, after the DM's final audit, or when the DM left the party (section 3.2.1), without the audit.

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
- The budget and the number of visible chests grow with the map's size class: Medium ×3, Big ×5, Dungeon ×8 of the Small numbers (`MAP_DESCRIPTION.md`, section 17.6; multipliers in `BalanceConfig`).

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

- Experience is awarded for each **won arena**: the sum of the experience of the pack's monsters, in full to every living character (it is not divided), accumulates in the cache, and is added to the character at the end (section 4.1.4). Nothing after a retreat, not even for the monsters killed in that fight (section 7.5).
- A character whose level is noticeably higher than the map tier gets less experience or none, and no rewards are rolled for them.
- The counterweight to the peaceful exit penalty applies here.

---

## 10. What is stored

### 10.1 `GameSession`

| Group        | Fields                                                                              |
| ------------ | ----------------------------------------------------------------------------------- |
| References   | map, DM, contribution escrow                                                        |
| Mode         | map / arena, the current arena and pack, the cells the party left on the map        |
| Queue        | map order; in an arena, each participant's time until turn                          |
| Exits        | the open Retreat or Escape proposal and who voted, retreats used, packs on guard and buffed, killed spawns |
| Participants | characters and their positions, monsters (template, HP, position, skills)           |
| Map          | `explored`, `revealed`, revealed secrets, opened doors, DM obligations              |
| Time         | turn number, timestamp of the current turn's start, the turn timeout's clock (the start or the unit's last action), when the DM's reveals became owed |
| Missing DM   | when the DM was marked missing, when the DM came back, who confirmed (section 3.2.1) |
| VRF          | the number and slot of the latest request (section 5.3.1)                           |
| Status       | in progress / victory / finished / abandoned by the DM                              |

### 10.2 `Participation`

| Group           | Fields                                                    |
| --------------- | --------------------------------------------------------- |
| References      | player, character, party, session key                     |
| Skills          | chosen 4 + 1 + 1                                          |
| Combat          | current HP, states, cooldowns, AP of the current turn     |
| Temporary cache | accumulated experience, rewards                           |
| Contribution    | amount, refund status                                     |
| Outcome         | in party / exited / escaped / dead                        |
