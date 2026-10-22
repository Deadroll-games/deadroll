# Zone: crates/types

Read the root CLAUDE.md first.

## What lives here
Account formats and ids shared by every program, the sdk and the website (`README.md` has the modules).

## Rules for this zone
- **Only add. Never rename or remove** a type, field, variant, constant or seed: devnet accounts and the sdk read these bytes. Every change is listed in the PR.
- New fields only at the end of a struct, new enum variants only at the end; `version: u8` first in every account core.
- Fixed-size data only, no `Vec`, no negative values.
- The only crate allowed to use Anchor (docs/ARCHITECTURE.md, 5), and only behind the `anchor` / `idl-build` features. No game logic beyond small helpers on the types.
- A change here usually needs `agents/CONTRACTS.md` updated (also add-only) and "needs devnet upgrade" in the PR.

## Commands
```
cargo test -p deadroll-types
cargo test -p deadroll-types --features anchor
cargo clippy -p deadroll-types --all-targets --features anchor -- -D warnings
```

## Done means
- tests green with and without `anchor` (`tests/roundtrip.rs`), clippy clean
