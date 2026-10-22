# Zone: programs/city

Read the root CLAUDE.md first.

## What lives here
City, buildings, interiors, order board (docs/ARCHITECTURE.md, 3.1). Empty skeleton for now: no accounts, no instructions.

## Rules for this zone
- Docs: docs/CITY_DESCRIPTION.md, docs/ITEMS_AND_CRAFT.md (storage, masters), docs/GAME_COMMISSIONS.md.
- A first instruction follows the `anchor-instruction` skill: one instruction = one file, `version: u8` first in every account, append-only fields and errors.
- Other programs that need city accounts or CPI depend on a `lib/` package: create `programs/city/lib/` first (root CLAUDE.md, section 3).
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
