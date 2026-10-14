# programs/items

Item loadouts: the class starter items (soulbound) and what a character wears, by slot
(`docs/ITEMS_AND_CRAFT.md`, sections 6 and 10). Program id `48FtbrDegNrAUHFza9HdJzZAvUo3uUNqs1kNnZxFsqJS`.
Item NFTs (`ItemState`), affixes and crafting come later.

## Accounts

| Account | Seeds | Layout (after the 8-byte discriminator) |
|---|---|---|
| `Loadout` | `["loadout", character]` | `core: LoadoutData` (`crates/types`): `version, character, owner, items: [ItemRecord; 4], item_count: u8, equipped: [EquipRef; 11], bump` |

- `ItemRecord` = `base: ItemBase, level: u8`; records `i < item_count` are used. Soulbound: they
  never leave the loadout and do not take bag slots.
- `EquipRef` = `source: EquipSource (Empty | Soulbound | Asset), index: u8, asset: Pubkey`, one per
  `EquipSlot` (main hand, off hand, helmet, body, pants, boots, gloves, necklace, cloak, ring 1,
  ring 2). `Asset` is reserved for item NFTs: the layout does not change when they arrive.
- The session program reads the core as `LoadoutData` at `join_session` and keeps the worn bases in
  `Participation.equipment` for the whole party.

## Instructions

| Instruction | Who | Accounts (in order) |
|---|---|---|
| `init_loadout()` | character owner | `owner (s, w), character, loadout (w), system_program` |
| `equip(item_index: u8, slot: EquipSlot)` | owner or session key | `signer (s), character, loadout (w), session_token (optional)` |
| `unequip(slot: EquipSlot)` | owner or session key | `signer (s), character, loadout (w), session_token (optional)` |

- `init_loadout` gives the class starter kit (`BalanceConfig.items.starter_kits`), each item worn in
  its own slot. The website sends it in the same transaction as `character::create_character`; a
  character created before items can call it later.
- `equip` / `unequip` are refused while `CharacterData.in_session` is set (from `join_session`
  until the rewards are claimed) and for a dead character. Equipping into a taken slot replaces
  what was there; an item is worn in one slot at a time.
- **Session key**: a signer other than the owner needs a live MagicBlock `SessionTokenV2` the owner
  issued to it for the session program: the one token the game starts, no second wallet popup.
  Equipping moves no value (`docs/GAME_COMMISSIONS.md`, section 5).
- Rules and numbers (slots, item level gap, kits, base stats) are `deadroll_rules::items` and
  `BalanceConfig.items` (`crates/rules`, tuned in `tools/balance`, Items tab). The logic on the
  account is pure (`src/state/loadout.rs`) and runs in the program-vs-simulator parity test.
- The character is read in place (`src/external.rs`): owner `deadroll_character`, the `Character`
  discriminator, the frozen `CharacterData` core. No dependency on the character crate.

## Errors (append only)

`Unauthorized, NotOwner, CharacterLocked, CharacterDead, InvalidCharacter, NoSuchItem, WrongSlot,
LevelTooLow, SlotEmpty, SessionTokenMissing, InvalidSessionToken, SessionTokenExpired`

## Events

`LoadoutCreated { character, owner, class, item_count }`,
`ItemEquipped { character, slot, item_index, base, replaced }`,
`ItemUnequipped { character, slot, base }`.

## Commands

```
cargo test -p deadroll-items                         # loadout logic
anchor build --tools-version v1.54 -p deadroll_items
pnpm --filter @deadroll/scripts run deploy:local      # whole stack on the local validator
pnpm --filter @deadroll/tests test:items              # needs ANCHOR_PROVIDER_URL / ANCHOR_WALLET
```
