# deadroll_city

Program id `2Cr5vB5awqDVx6XRcxtFFYQEG6Zy3aSAcKcqzh1xbVjS`.

The city, its buildings and the Tavern board with its quests (docs/ARCHITECTURE.md, 3.1; design in docs/CITY_DESCRIPTION.md; layouts in agents/CONTRACTS.md, section 25). Interiors are drawn by the site only.

| Instruction | Signer | What it does |
| --- | --- | --- |
| `init_city` | `GameConfig.authority` | the one `City` (`["city"]`) |
| `write_city`, `set_city_objects` | `City.authority` | tiles, decor, spawn |
| `add_building`, `update_building` | `City.authority` | a `Building` (`["building", id]`) calling one of `GameConfig.programs`, or none when closed |
| `close_building` | `City.authority` | status `Closed`: a boarded-up door |
| `init_tavern_board` | `City.authority` | the `TavernBoard` of a Tavern |
| `post_quest` | the DM's main wallet | a `Quest` (`["quest", session]`) for a party in the lobby, sent after `session.open_session` in the same transaction |
| `close_quest` | the quest's DM | takes it off the board once the party left the lobby or nobody joined |

The program makes no CPI: `post_quest` and `close_quest` read the map and the party (owners from `GameConfig.programs`). Taking a quest is `session.join_session`.

The city itself is data: `assets/city/city.json`, seeded by `tools/scripts/seed-city.ts` (part of `deploy:local` and `deploy:devnet`).

## Build and test

```
anchor build --tools-version v1.54 -p deadroll_city
cargo test -p deadroll-city
pnpm --filter @deadroll/tests run test:city     # local validator after deploy:local
```
