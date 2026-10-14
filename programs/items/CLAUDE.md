# Zone: programs/items

Owner lane: items. Read the root CLAUDE.md first, then this folder's README.md.

## What lives here
Nothing in the game yet. Later: item NFTs (`ItemState ["item", asset]`), affixes, crafting, artisan
listings. The starter items and the equipment are `deadroll_character` `Character.gear`, not here.

## Rules for this zone
- Docs: `docs/ITEMS_AND_CRAFT.md`, `docs/ART_DESCRIPTION.md` 6.3 (slots).
- No item rule or number in this program: they are `deadroll_rules::items` and `BalanceConfig.items`.
- Errors are append only (the codes of the removed `Loadout` instructions stay reserved).
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md (lead PRs only).

## Commands
```
anchor build --tools-version v1.54 -p deadroll_items
```

## Done means
- builds, clippy clean
- every instruction has happy path + rejection tests
- events emitted for every state change
