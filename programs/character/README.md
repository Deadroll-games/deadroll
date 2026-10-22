# programs/character

Player profiles, characters with their gear, starting skills, bag and the character Core NFT
(`docs/CHARACTER.md`, `docs/SKILL_BUILDER.md`, `docs/ITEMS_AND_CRAFT.md` 2, 6, 10). Program id
`9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL`.

## Accounts

| Account | Seeds | Layout (after the 8-byte discriminator) |
|---|---|---|
| `PlayerProfile` | `["profile", owner]` | `version, owner, character_count: u8, bump` |
| `Character` | `["character", owner, index: u8]` | `core: CharacterData` (frozen, CONTRACTS.md 4; `version` 3), then `name: [u8; 32], unspent_points: u8, nft_frozen: bool, gear: Gear, class_tier: u8` |
| `SkillSet` | `["skills", character]` | `version, character, count: u8, skills: [SkillRecord; 16], bump` |
| `Bag` | `["bag", character]` | `version, character, resources: [u32; 8], items: [Pubkey; 20], item_count: u8, bump` |
| `SkillExtras` (zero-copy) | `["skill_extras", character]` | `version, bump, character, extras: [SkillExtra; 16]`; `SkillExtra` = `range, timing, hp_below_pct, ap_cost: u8, description: [u8; 200]` by `SkillSet` index |

`SkillRecord` = `blocks: SkillBlocks` (12 bytes, `crates/skillcheck`), `shares_bp: [u32; 3]`,
`power_bp: u32`, `fusion: u8`, `cooldown: u8`, `name: [u8; 24]`. Slots `i < count` are used.
Session builds `deadroll_combat::Skill` from `blocks.aspects/aspect_count/source/form`,
`shares_bp` and `power_bp`. Names are zero-padded printable ASCII.

`Gear` = `items: [ItemRecord; 4], item_count: u8, equipped: [EquipRef; 11]` (`crates/types`):
the class starter items as soulbound records (they never leave the character and take no bag
slot) and what is worn by `EquipSlot`. `EquipSource::Asset` is kept for item NFTs. The account
logic is pure (`src/state/gear.rs`) and runs in the program-vs-simulator parity test.

## Instructions

| Instruction | Who | Accounts (in order) |
|---|---|---|
| `create_profile()` | player | `owner (s, w), profile (w), system_program` |
| `create_character(args)` | player | `owner (s, w), profile (w), character (w), skill_set (w), bag (w), config, asset (s, w, optional), mpl_core_program (optional), system_program, skill_extras (w)` |
| `set_skill_description(skill_index: u8, description: [u8; 200])` | owner main wallet | `owner (s), character, skill_set, skill_extras (w)` |
| `equip(item_index: u8, slot: EquipSlot)` | owner or session key | `signer (s), character (w), session_token (optional)` |
| `unequip(slot: EquipSlot)` | owner or session key | `signer (s), character (w), session_token (optional)` |
| `spend_stat_points(points: [u16; 4])` | owner main wallet | `owner (s), character (w)` |
| `lock_for_session(session: Pubkey, selection: [u8; 6])` | session PDA | `character (w), session_authority (s), config, owner (s), skill_set` |
| `grant_rewards(xp: u32, resources: [u32; 8])` | session PDA | `character (w), session_authority (s), config, bag (w)` |
| `mark_dead()` | session PDA | `character (w), session_authority (s), config, skill_set (w), bag (w)` |
| `unlock()` | session PDA | `character (w), session_authority (s), config` |
| `freeze_tombstone()` | anyone | `character (w), asset (w), payer (s, w), mpl_core_program, system_program` |

- **Starter items**: `create_character` gives the class starter kit (`BalanceConfig.items.starter_kits`,
  `deadroll_rules::items::starter_loadout`), each item worn in its own slot.
- **Equip / unequip**: refused while `in_session` is set (from `set_ready(true)` until the party
  ends: the bag and the gear are frozen) and for a dead character. Equipping into a taken slot
  replaces what was there; an item is worn in one slot at a time. A signer other than the owner
  needs a live MagicBlock `SessionTokenV2` the owner issued for the session program, checked by
  the `session-keys` crate's `#[session_auth_or]` (`src/auth.rs`: `game_session_token!`).
  Rules and numbers (slots, item level gap, kits) are `deadroll_rules::items` / `BalanceConfig.items`.
- **Trust**: `session_authority` must be the `["authority"]` PDA of `GameConfig.programs[Session]`.
  `config` is the registry `["config"]` PDA, read in place (`src/trust.rs`), no registry types.
- `lock_for_session` (the session's `set_ready(true)`) also needs the character owner as a signer,
  so a session cannot lock someone else's character. It checks the skills taken onto the map:
  `selection` is `SkillSet` indexes, slots 0-3 active, 4 reactive, 5 passive, `0xFF` empty;
  `deadroll_skillcheck::check_map_selection` (every category full: min(slots, owned), own
  skills only, no repeats). Emits `CharacterLocked` and `SkillsSelected`.
- `mark_dead` makes no CPI (ARCHITECTURE.md 4.4: death runs inside the VRF callback). It burns
  the skills (mechanics erased, names kept as the tombstone's memorial), the bag (resources and
  items) and the soulbound items (`gear` cleared). The app sends the permissionless
  `freeze_tombstone` after `CharacterDied`. The tombstone stays `in_session` until `unlock`, and
  can never be locked again.
- **NFT**: owner = player, update authority = the character PDA, plugin
  `PermanentFreezeDelegate { frozen: false }` (survives transfers), uri
  `<GameConfig.uri_base>/char/<character>/v<state_version>.json`. Never deserialize a Core asset on-chain.
- **Stat points** (docs/CHARACTER.md 5): `grant_rewards` adds `stat_points_per_level` per level
  gained; `spend_stat_points` places them all at once (`points` by `Stat`, sum in
  `1..=unspent_points`), only from the owner's main wallet, for a living character outside a
  session, and recomputes `hp_max`. Event `StatPointsSpent`.
- **Trait tiers** (docs/CHARACTER.md 6.3): the race trait tier is `core.predisposition_tier`, the
  class trait tier `class_tier`; both start at the worst tier and grow only deterministically.
- **Skill extras** (patch 5, docs/SKILL_BUILDER.md): `create_character` checks each skill's
  `SkillExt` (range, timing, HP threshold) with its blocks (`deadroll_skillcheck::validate_starting`)
  and stores it with the derived AP cost in `SkillExtras`. The description (up to 200 printable
  ASCII bytes) is written once by `set_skill_description`, outside a session, for a living
  character; it does not fit the creation transaction. `SkillExtras` is not burned at death.
- Balance numbers are placeholders: creation, HP, level XP, stat points and item numbers come from
  `deadroll_rules::BalanceConfig::DEFAULT` (`crates/rules`, tuned in `tools/balance`).

## Features

- `nft` (default): Core NFT. `--no-default-features` is `FALLBACK(nft)`: PDA only, `nft = None`.

## Commands

```
anchor build --tools-version v1.54 -p deadroll_character
# shared local validator (never start, stop or reset it): cd tools/scripts && pnpm deploy:local
cargo test -p deadroll-character                           # unit tests (rules, gear, uri)
pnpm --filter @deadroll/tests run test:character           # needs ANCHOR_PROVIDER_URL / ANCHOR_WALLET
pnpm --filter @deadroll/tests run test:gear
```

Tests: `tests/character.test.ts` and `tests/gear.test.ts`, fresh keypairs. The in-session lock,
the map selection and the death burn run through the session program in `tests/session.test.ts`.
