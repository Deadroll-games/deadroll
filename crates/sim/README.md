# deadroll-sim

Balance simulator: whole fights on the session program's rules, many of them in parallel.

- **Rules**: every rule call goes through `deadroll-rules` and `deadroll-combat`, the code the program runs. Only the orchestration lives here: the turn loop (`battle.rs`), a simple AI (`ai.rs`), seeded dice instead of VRF (one seed per action), statistics (`stats.rs`) and rayon batches (`run.rs`).
- **Numbers**: one `BalanceConfig` (`config_io` reads JSON over the defaults, so old files keep loading).
- **Content**: heroes, skills and encounters are JSON presets (`presets.rs`, `tools/balance/presets/`). Enums are written by variant name and listed from their `ALL` (`named.rs`).
- **Registry data**: aspects, tint traits, resonance pairs, the bestiary and the monster skills come from `assets/registry/*.json` (`deadroll_rules::registry`), the bytes the seed script uploads and the program copies at `open_session`. Monsters have no presets: an encounter names a bestiary monster with its level, count, `elite` and an optional `skills` mask (default: the first skills of each kind that fill the slots, `setup::default_skills`). `DEADROLL_REGISTRY` points at another folder.
- **Report**: `cargo run -p deadroll-sim --release --example report` prints the balance table attached to a PR that changes balance (docs/ARCHITECTURE.md 5.1).
- **Validation**: skills go through `skillcheck::validate_starting` and `check_loadout` like in the game (`setup.rs`); an invalid skill never fights, the reason is reported.

```rust
let (presets, _) = Presets::load(path);
let (party, issues) = resolve_party(&presets, &["human_warrior".into()], &cfg);
let (fight, _) = resolve_fight(&presets, "wolves");
let report = run_batch(&cfg, &party, &fight.unwrap(), Settings::default(), 10_000, 1, &Progress::default());
```

The fight ends when a boss dies (or every monster, if there is no boss), when every hero is dead, or at `max_rounds` (a draw). Same as the program: one attack or active skill per turn; monsters play their skills (active, the passive aura, the reactive one) through `deadroll_rules::monster_ai::plan`, the AI the website's auto mode runs too (heal a wounded kin ally, a buff an ally lacks, a debuff the target lacks, else the strongest damage).

`tests/parity.rs` runs the program's own code (`deadroll_session`, `SyscallRoll`) and the simulator (presets, JSON config, `VrfRoll`) on the same dice and asserts the same results.
