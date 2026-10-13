# Zone: crates/sim

Read the root CLAUDE.md first.

## What lives here
Fight orchestration for balancing: turn loop, AI, seeded dice, presets, statistics, rayon batches. No game rule lives here: rules are `deadroll-rules` / `deadroll-combat` calls.

## Rules for this zone
- Never copy a rule from the programs. If the sim needs one that only exists in a program, move it into `crates/rules` and call it from both.
- No hardcoded content: heroes, skills, monsters, encounters come from JSON presets. Enum lists come from `ALL` (`named.rs`).
- Matches on game enums that need handling (effects, modifier kinds) are exhaustive without `_`.
- Fight logic is integer and deterministic per seed; floats only in `stats.rs`.
- Anchor and Solana are dev-dependencies only (the parity test).

## Commands
```
cargo test -p deadroll-sim
cargo clippy -p deadroll-sim --all-targets -- -D warnings
```
