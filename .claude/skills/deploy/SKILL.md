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

1. RPC: take the Helius URL from the Solana CLI config and pass it with `--url`, and run the script as `pnpm --silent deploy:devnet ...` (checked: plain `pnpm` echoes the full command with the `--url` value, `--silent` does not). The scripts mask the URL themselves (`tools/scripts/lib/mask-url.ts`: host kept, path and query values `***`). In every message you write, mask it as `***` too. Never print the key. `solana config get` prints the computed `wss://` URL with the key too: never run it unfiltered, read `json_rpc_url` from the config file instead and mask `wss://` as well as `https://`.
2. Before the trial run:
   - Palette: if colors were appended to the `.pal` file, the version in its `palette <n>` header must be new. A finalized palette never changes, so the same version with other bytes fails mid-deploy, after the programs were upgraded. Compare the local `palette.bin` with the devnet blob of that version.
   - Demo map: check which `["map", wallet, index]` is free or holds the current layout. A map in an older layout cannot be decoded and the demo map step fails (`Codec [u8] cannot decode empty byte arrays`). Indexes 0-2 of the deploy wallet are old: pass `--map-index 3` (the next free one if 3 gets used by a session).
3. Trial run without sending: run `deploy-devnet.ts` without `--yes` and answer no at the cost prompt. Show the cost table (programs, buffers that are refunded, extensions, accounts, fees, total, wallet balance) and wait for the user's OK. The table has no line for the city accounts (`seed-city.ts`, about 0.02 SOL the first time).
4. Real deploy with `--skip-build`, so the files of the trial run are the ones sent.
5. Run `tests/smoke-devnet.ts` as it is (header of the file has the command). If it fails, stop and show the error. Do not rewrite or extend tests or scripts. Never run the city tests on devnet: they write test buildings into the shared city.
6. If a script has a bug: stop, show it, propose the smallest fix, change nothing without the user's OK.
7. Report: actual spend per line, new addresses, smoke result, compute units of the heaviest VRF callback.

## Devnet features

- The local validator turns every feature on, devnet does not. Off on devnet (U12): `sol_remaining_compute_units` (`5TuppMutoyzhUSfuYdhgzD47F92GL1g89KpCZQKqedxP`), `sol_blake3` (`HTW2pSyErTj4BV6KBM9NZ9VBUJVxt7sacNWcf76wtzb3`), `sol_big_mod_exp` (`EBq48m8irRKuE7ZnMTLvLg2UuGSqhe8s8oMqnmja1fJw`). A program calling one fails there with `unsupported BPF instruction`. The deploy scripts refuse such a build before sending (`tools/scripts/lib/syscalls.ts`); `solana feature status --display-all` lists the rest.
- A VRF callback that fails is dropped by the oracle without a transaction: the session stays in `pending_vrf` and the smoke reports "the oracle did not answer". Check before blaming the oracle: take the session's inner `consume_randomness` from an earlier callback of the same action, put a random value in its 32 bytes after the discriminator, and `simulateTransaction` it with `sigVerify: false`, the VRF identity (`vrf::VRF_IDENTITY`) as signer and the oracle's 300k CU limit. The logs show the real error.
- To reproduce devnet locally, the user restarts the validator with the feature off (`--reset` wipes the ledger, so ask first):

```
cd ~ && solana-test-validator --reset --deactivate-feature <feature id> --bpf-program CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d core.so --bpf-program KeyspM2ssCJbqUhQ4k7sveSiY4WjnYsrXkC8oDbwde5 session_keys.so
```

## main and devnet

`main` always matches devnet: deploy right before or right after the merge. If `Participation`, `GameSession` or `MonsterState` changed, sessions opened before the upgrade cannot be read after it. Say so.

