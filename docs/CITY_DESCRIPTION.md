# CITY_DESCRIPTION: the Deadroll city

Version 0.1. The city is a hub where all players appear: a Tavern with orders, workshops, later a market and guilds. This document describes the city and building accounts, what is on-chain and what is interface only, how new buildings are added, and how buildings with restricted access work.

Related documents:

- `MAP_DESCRIPTION.md`: tiles, prefabs, map rendering. The city is drawn by the same rules.
- `ART_DESCRIPTION.md`: geometry and the rasterizer.
- `GAME_COMMISSIONS.md`: sessions and fees.

> **Public repository.** This document sits next to open code. Do not give the names or lore of secret guilds here. Use neutral labels ("secret building No. 1") and keep the lore in the internal brief.

---

## 1. Principles

1. **One city for everyone.** One `City` account, and all new players appear in it.
2. **Looking is free.** Opening a building, viewing orders, the interior, a storefront is reading accounts via RPC: no signature, no fee, no turn.
3. **Walking around the city happens only in the interface.** There is no combat and there are no traps in the city, position decides nothing, so it is not stored on-chain and needs no transactions.
4. **A transaction only when something changes.** Post an order, take a quest, order a craft, buy.
5. **The city grows by data.** A new building of an existing type is added with a single instruction, without updating the site.
6. **Visibility in the interface, access in the program.** The site can hide a building, but only the program can forbid an action.

---

## 2. What is on-chain and what is interface

| Action                                   | Where                                        | Cost |
| ---------------------------------------- | -------------------------------------------- | ---- |
| A new player appearing in the city       | interface (after the player profile is created) | free |
| Walking around the city                  | interface                                    | free |
| Clicking a building, entering            | interface + reading `Building`               | free |
| Viewing orders, storefronts, recipes     | reading accounts                             | free |
| Post an order on the board               | transaction                                  | fee  |
| Take a quest                             | transaction                                  | fee  |
| Order a craft, buy, sell                 | transaction                                  | fee  |

Actions in the city are signed with the player's **main wallet**, not a session key: almost all of them move value (section 4 of `GAME_COMMISSIONS.md`).

Seeing other players walking around the city is not needed on-chain and is expensive. If it is ever needed, that is a task for a separate off-chain presence service, not the program.

---

## 3. Accounts

### 3.1 `City`

One PDA `["city"]`.

| Field             | Purpose                                                           |
| ----------------- | ----------------------------------------------------------------- |
| `width`, `height` | size of the city map                                              |
| `cells`           | tiles, 1 byte per cell (format as in `MAP_DESCRIPTION.md`)        |
| `objects`         | decor: trees, fountain, lanterns, carts                           |
| `spawn`           | the cell where a player appears                                   |
| `building_count`  | how many buildings have been created                              |
| `authority`       | who can add and change buildings                                  |

### 3.2 `Building`

A separate PDA for each building: `["building", id]`. Separate accounts rather than a list inside `City`, because that way a new building is added without resizing the city, and each building changes independently.

| Field                 | Example                                               |
| --------------------- | ----------------------------------------------------- |
| `id`                  | 3                                                     |
| `kind`                | Tavern / Blacksmith / Weaponsmith / Market / Guild... |
| `prefab_id`, `x`, `y` | how and where to draw it on the city map              |
| `door`                | the cell that, when clicked, opens the building       |
| `program`             | id of the program the building calls                  |
| `target`              | the building's main account (for example, the PDA of the order board) |
| `interior_id`         | interior (section 7)                                  |
| `status`              | `active` / `closed` / `secret` (section 6)            |
| `requirement`         | access condition (section 5)                          |

### 3.3 Building accounts

Each building type owns its accounts in its own program (or in a module of the Deadroll program):

| Type                  | Main account `target` | Child accounts                |
| --------------------- | --------------------- | ----------------------------- |
| Tavern                | `TavernBoard`         | a `Quest` PDA for each order  |
| Blacksmith, Weaponsmith | `Workshop`          | craft orders                  |
| Market (phase 2)      | `Market`              | lots                          |

---

## 4. Building types and interface

The client cannot build an interface by itself for an arbitrary unfamiliar contract: it does not know its instructions and data format. So **`kind` determines the interface module** on the site.

| `kind`       | What the player sees | Actions |
| ------------ | -------------------- | ------- |
| Tavern       | interior, order board, table for a new order | post an order (DM), take a quest and make a contribution to the party price (player) |
| MastersGuild | interior, list of player artisans with profession, level, perks and price; NPC artisan; tradesman who grants the first profession | choose an artisan and place an order; craft with the NPC artisan; get or change a profession; list own services (artisan) |
| Storage      | interior, contents of the storage and the bag | move items and resources between the bag and the storage |
| Market       | storefronts, lots   | list, buy (phase 2) |
| Guild        | depends on the guild | depends on the guild |

Extension rules:

- **A new building of an existing type** (a second Tavern, another Storage): data only, the site is not updated.
- **A new building type**: a new program or module + a new interface module.
- **Only the game's `authority` adds and changes buildings**, and only with programs from the allowlist. Otherwise anyone could attach someone else's contract to a "Tavern", and a player who clicked would sign a withdrawal of their SOL.

### MVP

| Building                | Purpose |
| ----------------------- | ------- |
| Tavern (Guild Board)    | DM orders, finding a party, collecting the party price |
| **Artisans' Guild**     | services of Blacksmiths and Weaponsmiths, NPC artisan, profession choice (`ITEMS_AND_CRAFT.md`, sections 7-8) |
| **Storage**             | storing items and resources, survives character death (`ITEMS_AND_CRAFT.md`, section 6) |

---

## 5. Access conditions

### 5.1 Condition types

| `requirement`       | Parameters               | Example                                                          |
| ------------------- | ------------------------ | ---------------------------------------------------------------- |
| `none`              |                          | Tavern                                                           |
| `holds_nft`         | Metaplex Core collection | the player holds an NFT from the "guild badge" collection        |
| `holds_badge`       | achievement id           | the player profile has the soulbound achievement "completed quest X" |
| `min_level`         | level                    | at least one of the player's characters is level ≥ 5             |
| `profession`        | profession               | only blacksmiths see the workshop's service entrance             |
| `all_of` / `any_of` | up to 3 conditions       | level ≥ 3 **and** achievement Y                                  |

Achievements (`badge`) are more convenient than NFTs for internal conditions: they are flags in the player profile PDA, they cannot be sold or transferred, and the check is cheaper. An NFT fits when access can be **transferred** (a key, a pass).

### 5.2 Two levels of checking

| Level         | What it does                                                          | Bypassable?                                 |
| ------------- | --------------------------------------------------------------------- | ------------------------------------------- |
| **Interface** | does not draw the building and does not open it if the condition is not met | yes, anyone can read the chain directly |
| **Program**   | every building instruction checks the signer's `requirement`          | no                                          |

The interface is responsible for **visibility**, the program for **access**. Even if someone found the building in the data and called its contract directly, the program will reject the action without the required NFT or achievement.

### 5.3 Checking in the program

```rust
fn check_requirement(req: &Requirement, player: &PlayerProfile, remaining: &[AccountInfo]) -> Result<()> {
    match req {
        Requirement::None => Ok(()),
        Requirement::HoldsBadge(id) => require!(player.has_badge(*id), GameError::NoAccess),
        Requirement::MinLevel(lvl) => { /* check the player's character account that was passed in */ }
        Requirement::HoldsNft(collection) => {
            // a Core asset is passed in remaining_accounts:
            // owner == signer, collection == collection
        }
        // ...
    }
}
```

---

## 6. Secret buildings

### 6.1 The problem

Everything on-chain is public. If a secret building is created in advance, anyone who reads `Building` accounts will learn about it before the players find it. Hiding it in the interface does not help here.

### 6.2 Solution: the building is created when it is opened

Before opening, the chain has neither a building account nor its description. There is only a **commitment**:

```
SecretSlot PDA ["secret", n]:
  commitment = SHA-256(secret answer ‖ salt)
  building_template  // encrypted or absent data is not stored;
                     // only n and commitment are here
```

Opening:

1. The players assemble the answer (for example, an address put together from fragments received in quests).
2. A player sends the instruction `discover(n, answer, salt)`.
3. The program checks `SHA-256(answer ‖ salt) == commitment`.
4. On a match: a `Building` is created with status `secret` (visible only with the access condition) or `active`, the discoverer receives an achievement or an NFT key, and the event is recorded in the lore.

Only the person hiding the secret (the head DM) knows the salt, and it is handed out together with the last fragment. Without the salt the answer cannot be brute-forced, even if it is short.

### 6.3 Fragments

Fragments must not be written on-chain openly as a reward: then any reader of the chain would collect them. Options:

- the head DM hands out a fragment in a live session (by voice, in Discord), and only the fact of the reward is recorded on-chain;
- the fragment is encrypted to the public key of the winner's wallet (ed25519 → x25519) and written on-chain in encrypted form; only the winner can read it.

The second option leaves a verifiable trace, but lets players trade fragments among themselves, which in itself becomes part of the game.

### 6.4 After opening

The building already exists on the chain, and its data is public. Only the **access** stays hidden: the building is seen and can be used only by those who meet the `requirement` (for example, hold the NFT key). Everyone else sees decor in its place, or nothing.

---

## 7. Interiors

An interior is a small map in the `MAP_DESCRIPTION.md` format:

- size 8×6 - 12×8 cells;
- floor and wall tiles, furniture prefabs (counter, tables, barrels, forge, anvil);
- **interactive points**: cells that, when clicked, open an interface module (order board, order table, anvil).

Interiors are stored in `Interior` accounts and are reused: all Taverns can refer to one interior. Inside a building the player also walks only in the interface.

---

## 8. Rendering

- The city and interiors are drawn with the same `render_map` of the rasterizer.
- Buildings are `PrefabBook` prefabs with the `building` category and a large base (for example, Tavern 4×3, forge 3×3) and headroom above for the roof.
- Draw order is the same as for maps: sorting by the bottom cell, so the player's character can walk behind a building.
- Buildings with status `closed` are drawn with a boarded-up door, `secret` ones without access are not drawn (decor from `City.objects` takes their place).

---

## 9. Account space budget

| What            | Size       | Deposit              |
| --------------- | ---------- | -------------------- |
| `City` 48×32    | ~1.6 KB    | ~0.012 SOL           |
| `Building`      | ~120 bytes | ~0.0017 SOL each     |
| `Interior` 12×8 | ~200 bytes | ~0.0023 SOL          |
| `SecretSlot`    | ~80 bytes  | ~0.0015 SOL          |

A city with about ten buildings and their interiors comes to roughly 0.05 SOL.

---

## 10. Implementation checklist

- [ ] `City` with tiles, decor, spawn point, `authority`
- [ ] `Building` as separate PDAs, instructions `add_building`, `update_building`, `close_building` for `authority` only
- [ ] Allowlist of programs for buildings
- [ ] `check_requirement` and calling it in all building instructions that have a condition
- [ ] Achievements (`badge`) in the player profile
- [ ] Interface modules: Tavern, Blacksmith, Weaponsmith
- [ ] Interiors with interactive points
- [ ] `SecretSlot` and the `discover` instruction
- [ ] Test: without an NFT or achievement, a secret building's instruction is rejected, even on a direct call
- [ ] Test: `discover` with a wrong answer or salt is rejected
- [ ] Test: a non-`authority` cannot add a building or change its `program`
