# Zone: programs/city

Read the root CLAUDE.md first.

## What lives here
City, buildings, the Tavern board and its quests (docs/ARCHITECTURE.md, 3.1; agents/CONTRACTS.md, section 25). Interiors are site only. The city data is `assets/city/city.json` (seeded by `tools/scripts/seed-city.ts`).

## Rules for this zone
- Docs: docs/CITY_DESCRIPTION.md, docs/ITEMS_AND_CRAFT.md (storage, masters), docs/GAME_COMMISSIONS.md.
- A first instruction follows the `anchor-instruction` skill: one instruction = one file, `version: u8` first in every account, append-only fields and errors.
- Other programs that need city accounts or CPI depend on the `lib/` package (`deadroll-city`); keep `lib/Cargo.toml` in sync with `Cargo.toml`.
- Accounts of other programs are read, not called (`src/reads.rs`): owner from `GameConfig.programs`, discriminator, then the `crates/types` core.
- Game rules and numbers go to `crates/rules`, never here.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md.

## Commands
```
anchor build --tools-version v1.54 -p deadroll_city
cargo clippy -p deadroll-city --all-targets -- -D warnings
```

## Done means
- builds, all tests green, clippy clean
- every instruction has happy path + rejection tests in `tests/`
- events emitted for every state change
