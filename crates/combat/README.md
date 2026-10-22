# deadroll-combat

All the hit math of docs/COMBAT_MATH.md and docs/DAMAGE_TYPES_AND_MIXES.md: dice, damage packets, modifiers, states, resonances, buffs and debuffs. Pure Rust, `no_std`, integers only, no allocation.

Used by the session program (every hit), `crates/rules`, `crates/sim` and the website through `wasm/` (C ABI, see `wasm/README.md`).

| Module | What |
|---|---|
| `hit` | one hit, in the fixed order of COMBAT_MATH.md 6; preview for the website |
| `packet` | damage packets and their shares |
| `modifiers` | bonuses multiply, counterweights divide |
| `states`, `resonance` | states, resonances, tint traits |
| `support` | buffs, debuffs, heal |
| `rng` | `RollSource`, `VrfRoll` (`hash(seed, roll_index)`) |
| `balance`, `tables` | `Balance` numbers and fixed tables |

## Build and test

```
cargo test -p deadroll-combat
cargo build -p deadroll-combat --target wasm32-unknown-unknown --release
bash crates/combat/wasm/build.sh --check      # the committed app WASM matches this code
```
