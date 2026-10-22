# Zone: crates/combat

Read the root CLAUDE.md first. Skill: `combat-math`.

## What lives here
Hit math: dice, packets, modifiers, states, resonances (`README.md` has the modules). Called by `crates/rules`, the session program, `crates/sim` and the website WASM.

## Rules for this zone
- Docs: docs/COMBAT_MATH.md (hit order: section 6, no negatives: section 3), docs/DAMAGE_TYPES_AND_MIXES.md.
- `no_std`, integers only, no allocation, fixed-size arrays. Basis points: `10_000` = 100% = x1.
- Every number goes into `Balance`, never a literal in a formula.
- All dice through `RollSource`; the roll order of a hit is part of the format (the website replays hits).
- A change here changes `wasm/`: rebuild `app/src/chain/wasm/combat.wasm` (`bash crates/combat/wasm/build.sh`) and refresh the TS fixtures (`UPDATE_TS_FIXTURES=1 cargo test -p deadroll-combat-wasm --test ts_fixtures`) in the same PR.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md.

## Commands
```
cargo test -p deadroll-combat
cargo clippy -p deadroll-combat --all-targets -- -D warnings
cargo test -p deadroll-sim --test parity
```

## Done means
- tests green (unit, `tests/golden.rs`, `tests/edges.rs`, `tests/props.rs`), parity green, clippy clean
