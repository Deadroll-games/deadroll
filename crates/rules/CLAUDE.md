# Zone: crates/rules

Read the root CLAUDE.md first.

## What lives here
The session program's rules without accounts: hit inputs, AP, initiative, reach, turn start, action resolution, and `BalanceConfig` with every balance number. The session and character programs call it with `BalanceConfig::DEFAULT`; `crates/sim` and `tools/balance` call it with a tuned config.

## Rules for this zone
- `no_std` without features, integers only, no allocation, fixed-size data. Keep large values (`HitInput`, `HitResult`) behind `&mut` from the caller: the program boxes them to stay under the 4 KB SBF stack frame.
- Every balance number goes into `BalanceConfig` (or the crate that owns it: `combat::Balance`, `combat::SupportBalance`, `skillcheck::SkillTuning`), never a literal in a rule.
- Tables indexed by an enum use `[T; Enum::COUNT]`; a new variant must not compile until `DEFAULT` has numbers for it. Matches on game enums are exhaustive, no `_` arm.
- A field added to `Balance` / `SupportBalance` / `SkillTuning` must be added to the serde mirrors in `config.rs` (it does not compile otherwise) and to the Config tab of `tools/balance`.
- Changing a default changes the programs: run `cargo test -p deadroll-session -p deadroll-sim` and build the programs.

## Commands
```
cargo test -p deadroll-rules --features serde
cargo test -p deadroll-sim --test parity
cargo clippy -p deadroll-rules --all-targets -- -D warnings
```
