# Zone: programs/items

Owner lane: items. Read the root CLAUDE.md first, then this folder's README.md.

## What lives here
Character loadouts: soulbound starter items and equipment by slot (`Loadout`). Later: item NFTs
(`ItemState ["item", asset]`), affixes, crafting, artisan listings.

## Rules for this zone
- Docs: `docs/ITEMS_AND_CRAFT.md`, `docs/ART_DESCRIPTION.md` 6.3 (slots), `docs/GAME_COMMISSIONS.md` 5 (session keys).
- `Loadout.core` is `deadroll_types::LoadoutData`; the session program reads it. Only append fields after `core`.
- No item rule or number in this program: they are `deadroll_rules::items` and `BalanceConfig.items`.
  Keep the account logic pure in `src/state/loadout.rs`; `crates/sim/tests/parity.rs` runs it.
- Equipment never changes while the character is in a session (`in_session`): the session copies it once at join.
- Errors are append only. Account owners and discriminators of other programs are constants in
  `src/external.rs`, checked by the parity test.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md (lead PRs only).

## Commands
```
cargo test -p deadroll-items
anchor build --tools-version v1.54 -p deadroll_items
pnpm --filter @deadroll/tests test:items
```

## Done means
- builds, unit and integration tests green, clippy clean
- every instruction has happy path + rejection tests in `tests/items.test.ts` (in-session and dead in `tests/session.test.ts`)
- events emitted for every state change
