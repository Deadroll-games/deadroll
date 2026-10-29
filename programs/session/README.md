# deadroll_session

Program id `3gzfih2WaBgXcYZ57jWokmZM3FDE6QHX5y4R9EvKZoNJ`.

Party, `Participation`, turns, combat, the VRF callback and the party price escrow (docs/ARCHITECTURE.md, 3.1 and 3.3). Rules come from `crates/rules` and `crates/combat`; this program holds accounts, signers, CPI and events.

## Accounts

| Account | Seeds | What |
|---|---|---|
| `GameSession` | `["session", map]` | party state, turn order, the map copy |
| `MonsterState` | `["monsters", session]` | monsters of the party and the registry resonance tables (aspects, traits, resonances, triples); 10 144 bytes, a CPI allocates at most 10 240 (a test pins it) |
| `Escrow` | `["escrow", session]` | the party price (docs/GAME_COMMISSIONS.md) |
| `Participation` | `["participation", session, character]` | one hero in the party |
| `RewardCache` | `["cache", character]` | patch 11: the cache after the party, until "Done" in the bag screen |

All of them are created at `open_session` / `join_session` (the reward cache once per character, then rebound); game moves never create accounts. Formats: agents/CONTRACTS.md.

## Instructions

| Instruction | Who | What |
|---|---|---|
| `open_session` | DM | opens a party on a finalized map, sets the price, copies registry data |
| `join_session` | player | joins with a character, pays into the escrow |
| `set_ready` | player | picks 4 active + 1 reactive + 1 passive, locks the character and its gear (CPI character) |
| `start_session` | DM | rolls initiative, moves the map to InSession (CPI map) |
| `start_turn`, `end_turn` | current unit | turn start (ticks, skips, cooldowns), turn end |
| `move_path(unit, path)` | player: owner or its session key; monster: DM or its session key | the cells in order (8 neighbors, diagonal = one step, never past an impassable corner), action points by tile type, no rolls; a reactive skill can stop the walk and roll (patch 13) |
| `act`, `monster_act` | owner / DM | one action; requests a VRF value |
| `consume_randomness` | VRF oracle | resolves the action from the VRF value |
| `flee` | player | flee vote |
| `open_chest` | owner / key | patch 11: opens a visible chest next to the player (AP); requests a VRF value |
| `settle_party` | DM main wallet | patch 11: every cache becomes its player's, XP and unlock (CPI character), the escrow for the DM, the map Finished |
| `claim_after_timeout` | player main wallet | patch 11: after the DM's deadline, the same for one character and its contribution back |
| `leave_party` | owner main wallet | patch 11: frees a living character from its ended party; its rewards keep waiting |
| `close_cache` | owner main wallet | patch 11: "Done": the bag layout (CPI character `apply_bag`), the cache closes |

The temporary cache fills during the party in `Participation.cache` (`src/loot.rs`: kills pay out at the end of each VRF callback, chests in their own callback); `src/settlement.rs` ends the party for each player.

Events: `src/events.rs`. The `mock-vrf` feature is for localnet only (`pnpm deploy:local`).

## Build and test

```
anchor build --tools-version v1.54 -p deadroll_session
cargo test -p deadroll-session
pnpm --filter @deadroll/tests run test:session     # tests/session.test.ts, local validator after pnpm deploy:local
```
