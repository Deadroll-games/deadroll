# Zone: programs/character

Owner lane: L5. Read the root CLAUDE.md first, then this folder's README.md.

## What lives here
Player profiles, characters (PDA + Core NFT) with their gear (soulbound starter items and what is
worn, `Character.gear`), starting skills (`SkillSet`), bag (`Bag`), `equip` / `unequip`, and the
session-only CPI targets: lock (with the map skill check), rewards, death, unlock. Skill rules come
from `crates/skillcheck`, item rules from `crates/rules` (`items`, `BalanceConfig.items`).

## Rules for this zone
- Docs: `docs/CHARACTER.md`, `docs/SKILL_BUILDER.md` sections 3.4 and 10-13,
  `docs/ITEMS_AND_CRAFT.md` 2, 6 and 10, `docs/STORAGE_AND_DELIVERY.md` 6.2.
- Accounts owned here: `PlayerProfile`, `Character`, `SkillSet`, `Bag` (seeds in CONTRACTS.md 2).
  `Character.core` is the frozen `CharacterData`; only append fields after it (`gear` is last now).
- Gear changes only outside a session: `in_session` freezes the bag and the gear. Keep the gear logic
  pure in `src/state/gear.rs`; `crates/sim/tests/parity.rs` runs it.
- CPI out: mpl-core only (`src/cpi_out/nft.rs`, feature `nft`). No CPI into game programs, and none
  from `mark_dead` (ARCHITECTURE.md 4.4). Never deserialize a Core `Asset` or `Collection` on-chain.
- CPI in: session (`lock_for_session`, `grant_rewards`, `mark_dead`, `unlock`), checked by
  `trust::check_session_caller`. Keep their account order: `[character, session_authority, config, ...]`.
- Errors are append only.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md.

## Commands
```
anchor build --tools-version v1.54 -p deadroll_character
cd tools/scripts && pnpm deploy:local    # shared validator, never start or reset it
cargo test -p deadroll-character        # unit tests (rules, gear, uri)
pnpm --filter @deadroll/tests run test:character
pnpm --filter @deadroll/tests run test:gear
```

## Done means
- builds, all tests green, clippy clean
- every instruction has happy path + rejection tests in `tests/character.test.ts` / `tests/gear.test.ts`
  (in-session, selection and death in `tests/session.test.ts`)
- events emitted for every state change
