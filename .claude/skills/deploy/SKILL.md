---
name: deploy
description: Use for any deploy task: deploy, devnet upgrade, deploy:local, local validator, deploy scripts, smoke test on devnet, building programs for deploy.
---

# Deploying Deadroll

Scripts and flags: `tools/scripts/README.md`. This skill is the agent procedure.

## Local

- The validator runs in WSL and the user starts it. Agents never start or reset it. If a `--reset` is needed, ask the user.
- Start command (for the user):

```
cd ~ && solana-test-validator --bpf-program CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d core.so --bpf-program KeyspM2ssCJbqUhQ4k7sveSiY4WjnYsrXkC8oDbwde5 session_keys.so
```

- `cd tools/scripts && pnpm deploy:local` deploys all programs, config, registry, rasterizer and the demo map. The session is built with `mock-vrf`.

## Build rules

- Programs for deploy are built only by the deploy scripts (`cargo build-sbf` in the program folder, cache `target/sbf-deploy/`). Never take a `.so` from `anchor build`: on Windows it starts cargo in a `\?\D:\...` folder, rebuilds shared crates on every run and its bytes vary.
- The build cache is never cleaned by hand.
- `pnpm --filter @deadroll/sdk generate` only after a plain `anchor build`, never after a `mock-vrf` build.

## Devnet

Only as a separate task and only with the user's OK.

1. RPC: take the Helius URL from the Solana CLI config and pass it with `--url`. In every output, log and message mask it as `***`. Never print the key.
2. Trial run without sending: run `deploy-devnet.ts` without `--yes` and answer no at the cost prompt. Show the cost table (programs, buffers that are refunded, extensions, accounts, fees, total, wallet balance) and wait for the user's OK.
3. Real deploy with `--skip-build`, so the files of the trial run are the ones sent.
4. Run `tests/smoke-devnet.ts` as it is (header of the file has the command). If it fails, stop and show the error. Do not rewrite or extend tests or scripts.
5. If a script has a bug: stop, show it, propose the smallest fix, change nothing without the user's OK.
6. Report: actual spend per line, new addresses, smoke result, compute units of the heaviest VRF callback.

## main and devnet

`main` always matches devnet: deploy right before or right after the merge. If `Participation`, `GameSession` or `MonsterState` changed, sessions opened before the upgrade cannot be read after it. Say so.

