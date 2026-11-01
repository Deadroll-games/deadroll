---
name: pr-check
description: Use before opening a PR and before every push to a PR branch: run locally everything CI runs (.github/workflows), with the same commands, then wait for CI on the PR and fix until it is green.
---

# PR check

CI is `.github/workflows/{crates,programs,app,integration}.yml`. If a workflow changes, update this list
from it: the workflow is the source of truth. A PR without green CI is not ready, and the agent's report
is written only after CI is green.

Keep output short: pipe through `| tail -n 40` or grep for `test result|FAILED|error`.

## 1. Before opening a PR and before every push

Run all of it, not only what the diff touches. In a stacked PR, run it on the top branch.

crates (`crates.yml`):

```
C="-p deadroll-types -p deadroll-combat -p deadroll-skillcheck -p deadroll-mapcheck -p deadroll-render -p deadroll-rules"
cargo fmt --all --check
cargo clippy $C -p deadroll-sim --all-targets -- -D warnings
cargo clippy -p deadroll-types --all-targets --features anchor -- -D warnings
cargo test $C -p deadroll-sim
cargo test -p deadroll-types --features anchor
cargo build $C --target wasm32-unknown-unknown --release
```

ts-fixtures (`crates.yml`):

```
cargo test -p deadroll-skillcheck --test ts_fixtures
cargo test -p deadroll-combat-wasm --test ts_fixtures
cargo test -p deadroll-mapcheck-wasm --test ts_fixtures
cargo test -p deadroll-rules --features serde --test registry
cargo test -p deadroll-rules --features serde --test arena_ts_fixtures
```

WASM the site ships (`crates.yml`, needs Docker Desktop; if it is not running, ask the user):

```
bash crates/combat/wasm/build.sh && bash crates/combat/wasm/build.sh --check
bash crates/mapcheck/wasm/build.sh && bash crates/mapcheck/wasm/build.sh --check
```

Commit `app/src/chain/wasm/*.wasm` when they changed. Any change in `crates/` the WASM uses
(types, combat, skillcheck, mapcheck, rules) can make them stale.

programs (`programs.yml`):

```
anchor build --tools-version v1.54 2>&1 | grep -v "frame space"
cargo test -p deadroll-registry -p deadroll-character -p deadroll-items -p deadroll-map -p deadroll-session -p deadroll-city -p deadroll-render-program
pnpm --filter @deadroll/sdk check
```

The build also reports SBF frames over 4096 bytes ("overflows the maximum allowed frame space").
Ignore `mpl_core`. A new entry in our programs is a bug unless the function is never called (as
`MonsterState`'s Borsh decoders and `LazyAccount` loaders, `programs/session/src/state/monster_state.rs`).
To see them all, touch `programs/*/src/lib.rs` first so every program recompiles.

app (`app.yml`, in `app/`):

```
pnpm typecheck
pnpm test
pnpm build:devnet
```

A local `.env.*` with a private RPC makes `build:devnet` refuse to bundle it. That is the guard
working: blank `VITE_RPC_URL` and `VITE_RPC_WS_URL` for the run, never print them.

integration (`integration.yml`): program and cross-program TS tests on a local validator. The user
starts the validator (skill `deploy`): ask once, after everything above is green, and name the
devnet-inactive features to deactivate. Then:

```
cd tools/scripts && node --experimental-strip-types --no-warnings deploy-local.ts
cd tests && pnpm test:programs && pnpm test:cross
```

## 2. After the push

```
gh pr checks <n> --watch
```

If a job fails: `gh run view --job <id> --log-failed | tail -n 40`, fix, run section 1 again, push,
watch again. Repeat until every check is green. Stop dev servers, watchers and node processes you
started before reporting.
