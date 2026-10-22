# deadroll_session

Program id `3gzfih2WaBgXcYZ57jWokmZM3FDE6QHX5y4R9EvKZoNJ`.

Party, `Participation`, turns, combat, the VRF callback and the party price escrow (docs/ARCHITECTURE.md, 3.1 and 3.3). Rules come from `crates/rules` and `crates/combat`; this program holds accounts, signers, CPI and events.

## Accounts

| Account | Seeds | What |
|---|---|---|
| `GameSession` | `["session", map]` | party state, turn order, the map copy |
| `MonsterState` | `["monsters", session]` | monsters of the party |
| `Escrow` | `["escrow", session]` | the party price (docs/GAME_COMMISSIONS.md) |
| `Participation` | `["participation", session, character]` | one hero in the party |
| `TempStash` | `["stash", session, character]` | loot held until the party ends |

All of them are created at `open_session` / `join_session`; game moves never create accounts. Formats: agents/CONTRACTS.md.

## Instructions

| Instruction | Who | What |
|---|---|---|
| `open_session` | DM | opens a party on a finalized map, sets the price, copies registry data |
| `join_session` | player | joins with a character, pays into the escrow |
| `set_ready` | player | picks 4 active + 1 reactive + 1 passive, locks the character and its gear (CPI character) |
| `start_session` | DM | rolls initiative, moves the map to InSession (CPI map) |
| `start_turn`, `end_turn` | current unit | turn start (ticks, skips, cooldowns), turn end |
| `move_unit`, `monster_move` | owner / DM | movement on the map |
| `act`, `monster_act` | owner / DM | one action; requests a VRF value |
| `consume_randomness` | VRF oracle | resolves the action from the VRF value |
| `flee` | player | flee vote |
| `claim_rewards`, `claim_escrow` | player / DM | XP and death finalization (CPI character), the escrow for the DM |

Events: `src/events.rs`. The `mock-vrf` feature is for localnet only (`pnpm deploy:local`).

## Build and test

```
anchor build --tools-version v1.54 -p deadroll_session
cargo test -p deadroll-session
pnpm --filter @deadroll/tests run test:session     # tests/session.test.ts, local validator after pnpm deploy:local
```
