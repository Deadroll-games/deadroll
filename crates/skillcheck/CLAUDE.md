# Zone: crates/skillcheck

Owner lane: L5. Read the root CLAUDE.md first.

## What lives here
The skill builder rules (`docs/SKILL_BUILDER.md`, section 13), shared by the character program
and the website: `SkillBlocks`, `validate`, `validate_starting` (class limits, section 10.1),
`check_loadout` (4 active, 1 reactive, 1 passive), `check_map_selection` (the 4 + 1 + 1 taken onto a
map, checked by the character program at `set_ready`), `derive` (shares, power, fusion, cooldown) and
the class starter `presets`.

## Rules for this zone
- Pure Rust, `no_std` without features, integers only, no allocation.
- Enum discriminants and the `SkillBlocks` field order are on-chain format: append only.
- Every number in `derive.rs` and `rules.rs` is a BALANCE placeholder; changing one changes stored
  skills of new characters only.
- No Anchor in this crate (docs/ARCHITECTURE.md, section 5). `SkillBlocks`, `SkillExt` and the block
  enums live in `deadroll_types::skill` (re-exported here), because programs store them in accounts.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md.

## Commands
```
cargo test -p deadroll-skillcheck
cargo clippy -p deadroll-skillcheck --tests -- -D warnings
```

## Done means
- tests green (happy paths in `tests/presets.rs`, one test per `SkillError` in `tests/rejections.rs`,
  map selections in `tests/map_selection.rs`)
- clippy clean
