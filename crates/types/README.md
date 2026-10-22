# deadroll-types

Shared types of all programs, the sdk and the website: account formats, ids of aspects, slots and tags, seeds, serialization (docs/ARCHITECTURE.md, 5; agents/CONTRACTS.md, sections 2-4).

| Module | What |
|---|---|
| `accounts` | account cores (`CharacterData`, `GameSessionData`, `ParticipationData`, ...) |
| `enums` | aspects, states, classes, races, tags, statuses |
| `skill` | skill blocks stored in accounts (`SkillBlocks`, `SkillExt`); the rules on them are in `crates/skillcheck` |
| `consts`, `seeds` | sizes and PDA seeds |
| `render` | `RenderRequest`, `LayerRef` |

Without features: `no_std` and plain borsh. `anchor`: the same types derive `AnchorSerialize` / `AnchorDeserialize` / `InitSpace`, so programs embed them in accounts; `idl-build` puts them into the IDLs. Both give the same bytes.

## Build and test

```
cargo test -p deadroll-types
cargo test -p deadroll-types --features anchor
```
