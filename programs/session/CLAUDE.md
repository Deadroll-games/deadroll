# Zone: programs/session

Read the root CLAUDE.md first.

## What lives here
Party, `Participation`, turns, combat, the VRF callback and the party price escrow (docs/ARCHITECTURE.md, 3.1, 3.3). All of combat stays in this program; CPI into `character` and `map` happens only at party boundaries.

## Rules for this zone
- Docs: docs/PARTY_AND_TURNS.md, docs/COMBAT_MATH.md, docs/GAME_COMMISSIONS.md.
- `lib/Cargo.toml` is this program as a dependency of other crates (same `src/`, rlib; `crates/sim` uses it): a dependency or feature added to `Cargo.toml` goes there too.
- CPI out only through `deadroll-character` and `deadroll-map` from `[workspace.dependencies]` (their `lib/` packages), wrappers in `src/cpi_out/`.
- `mock-vrf` is for localnet only (`pnpm deploy:local`); `deploy-devnet.ts` refuses a mock build. Never take a deploy `.so` from `anchor build`, the deploy scripts build their own.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md.

## Commands
```
anchor build --tools-version v1.54 -p deadroll_session
cargo test -p deadroll-session
pnpm --filter @deadroll/tests run test:session
```

## Done means
- builds, all tests green, clippy clean
- every instruction has happy path + rejection tests
- events emitted for every state change
