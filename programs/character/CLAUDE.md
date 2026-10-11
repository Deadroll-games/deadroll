# Zone: programs/character

Owner lane: L5. Read the root CLAUDE.md first, then this folder's README.md.

## What lives here
Player profiles, characters (PDA + Core NFT), starting skills (`SkillSet`), bag (`Bag`), and the
session-only CPI targets: lock, rewards, death, unlock. Skill rules come from `crates/skillcheck`.

## Rules for this zone
- Docs: `docs/CHARACTER.md`, `docs/SKILL_BUILDER.md` sections 10-13, `docs/STORAGE_AND_DELIVERY.md` 6.2.
- Accounts owned here: `PlayerProfile`, `Character`, `SkillSet`, `Bag` (seeds in CONTRACTS.md 2).
  `Character.core` is the frozen `CharacterData`; only append fields after it.
- CPI out: mpl-core only (`src/cpi_out/nft.rs`, feature `nft`). No CPI into game programs, and none
  from `mark_dead` (ARCHITECTURE.md 4.4). Never deserialize a Core `Asset` or `Collection` on-chain.
- CPI in: session (`lock_for_session`, `grant_rewards`, `mark_dead`, `unlock`), checked by
  `trust::check_session_caller`. Keep their account order: `[character, session_authority, config, ...]`.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md.

## Commands
```
anchor build --tools-version v1.54 -p deadroll_character
anchor deploy -p deadroll_character --provider.cluster localnet   # shared validator, this program only
anchor test --skip-local-validator --skip-deploy
cargo test -p deadroll-character        # unit tests (rules, uri)
```

## Done means
- builds, all tests green, clippy clean
- every instruction has happy path + rejection tests in `tests/character.test.ts`
- events emitted for every state change
