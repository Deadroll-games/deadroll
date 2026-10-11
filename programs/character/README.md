# programs/character

Player profiles, characters, starting skills, bag and the character Core NFT
(`docs/CHARACTER.md`, `docs/SKILL_BUILDER.md`). Program id `9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL`.

## Accounts

| Account | Seeds | Layout (after the 8-byte discriminator) |
|---|---|---|
| `PlayerProfile` | `["profile", owner]` | `version, owner, character_count: u8, bump` |
| `Character` | `["character", owner, index: u8]` | `core: CharacterData` (frozen, CONTRACTS.md 4), then `name: [u8; 32], unspent_points: u8, nft_frozen: bool` |
| `SkillSet` | `["skills", character]` | `version, character, count: u8, skills: [SkillRecord; 16], bump` |
| `Bag` | `["bag", character]` | `version, character, resources: [u32; 8], items: [Pubkey; 20], item_count: u8, bump` |

`SkillRecord` = `blocks: SkillBlocks` (12 bytes, `crates/skillcheck`), `shares_bp: [u32; 3]`,
`power_bp: u32`, `fusion: u8`, `cooldown: u8`, `name: [u8; 24]`. Slots `i < count` are used.
Session builds `deadroll_combat::Skill` from `blocks.aspects/aspect_count/source/form`,
`shares_bp` and `power_bp`. Names are zero-padded printable ASCII.

## Instructions

| Instruction | Who | Accounts (in order) |
|---|---|---|
| `create_profile()` | player | `owner (s, w), profile (w), system_program` |
| `create_character(args)` | player | `owner (s, w), profile (w), character (w), skill_set (w), bag (w), config, asset (s, w, optional), mpl_core_program (optional), system_program` |
| `lock_for_session(session: Pubkey)` | session PDA | `character (w), session_authority (s), config, owner (s)` |
| `grant_rewards(xp: u32, resources: [u32; 8])` | session PDA | `character (w), session_authority (s), config, bag (w)` |
| `mark_dead()` | session PDA | `character (w), session_authority (s), config` |
| `unlock()` | session PDA | `character (w), session_authority (s), config` |
| `freeze_tombstone()` | anyone | `character (w), asset (w), payer (s, w), mpl_core_program, system_program` |

- **Trust**: `session_authority` must be the `["authority"]` PDA of `GameConfig.programs[Session]`.
  `config` is the registry `["config"]` PDA, read in place (`src/trust.rs`), no registry types.
- `lock_for_session` also needs the character owner as a signer (the player signs `join_session`),
  so a session cannot lock someone else's character.
- `mark_dead` makes no CPI (ARCHITECTURE.md 4.4: death runs inside the VRF callback). The app
  sends the permissionless `freeze_tombstone` after `CharacterDied`. The tombstone stays
  `in_session` until `unlock`, and can never be locked again.
- **NFT**: owner = player, update authority = the character PDA, plugin
  `PermanentFreezeDelegate { frozen: false }` (survives transfers), uri
  `<GameConfig.uri_base>/char/<character>/v<state_version>.json`. Never deserialize a Core asset on-chain.
- Balance numbers (stat budget, HP, XP per level) are placeholders in `src/rules.rs`.

## Features

- `nft` (default): Core NFT. `--no-default-features` is `FALLBACK(nft)`: PDA only, `nft = None`.

## Commands

```
anchor build --tools-version v1.54 -p deadroll_character
# shared local validator (never start, stop or reset it; deploy only this program):
anchor deploy -p deadroll_character --provider.cluster localnet
anchor test --skip-local-validator --skip-deploy           # or: pnpm --filter @deadroll/tests run test:character
```

Tests: `tests/character.test.ts`, fresh keypairs, `GameConfig` read if it exists. Without it
(registry, L4) creation is skipped; the session-only happy paths need the session program (L7).
