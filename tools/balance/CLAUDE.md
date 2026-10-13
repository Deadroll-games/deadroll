# Zone: tools/balance

Read the root CLAUDE.md first.

## What lives here
The egui (eframe) app over `deadroll-sim`, and the preset files in `presets/`.

## Rules for this zone
- No game rules and no hardcoded content in the app: rules come from `deadroll-rules` through `deadroll-sim`, content from `presets/`.
- Pickers list enum variants from `ALL` (`widgets::pick`); a new variant appears without code changes.
- Long work runs on a background thread with `deadroll_sim::Progress`; the UI thread never runs fights.
- eframe is pinned to the newest release for the workspace Rust (`rust-toolchain.toml`); check `rust-version` before bumping.

## Commands
```
cargo run -p balance --release
cargo clippy -p balance --all-targets -- -D warnings
```
