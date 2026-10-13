# TASKS: board for parallel agents

## Where we are (main at #26)

Done: L0 to L9 (the demo plays end to end on the local validator), turn rules (#23, app #24) and support and control skills (L7 phase 2, #25, app #26). Devnet was deployed in #19 and still runs the `session` and `character` builds from before #23 and #25.

## Next

Now, in this order where it matters:
- [ ] **Devnet upgrade:** redeploy `session` and `character` from `main` with `deploy:devnet` (real VRF, no `mock-vrf`), then rerun the devnet VRF smoke test. Parties opened before the upgrade cannot be read after it, so open new ones.
- [ ] **SDK regeneration (L8.1):** Codama clients for all IDLs into `sdk/`. The app still uses the IDL snapshots in `app/src/chain/idl/`.
- [ ] **Hosting:** deploy the app to Vercel (public devnet RPC by default; Custom RPC for players).
- [ ] **CI for `app/`:** only `.github/workflows/crates.yml` exists, and it triggers on crate paths only. Add a trigger and checks (typecheck, test, build) for changes in `app/`.

Then:
- [ ] Stat points on level up.
- [ ] Map visuals.
- [ ] Monster sprites in the map editor.
- [ ] A boss that takes 2 cells.
- [ ] Items and craft (`programs/items`, `docs/ITEMS_AND_CRAFT.md`).
- [ ] Auras and passive skills in combat (the skill builder already accepts reactive and passive skills and auras; combat uses only Active skills).

## Decision: dependencies and toolchain (L0.4, 2026-10-11, lead)

Spike result: Anchor 1.2.1 + all three external crates compile to SBF and produce a correct IDL. **No fallbacks.**

| Crate | Pin (root `Cargo.toml`, `[workspace.dependencies]`) |
|---|---|
| anchor-lang | `=1.2.1` (default `solana-v3`) |
| ephemeral-vrf-sdk | `=0.17.3`, feature `anchor` |
| session-keys | `=3.1.1`, features `no-entrypoint`, `idl-build` (forward `session-keys/idl-build` from the program's `idl-build`) |
| mpl-core | `=0.12.1`, default features (never its `anchor` feature) |

Toolchain: Solana CLI 4.1.2, Anchor CLI 1.2.1, Rust 1.92.0, platform tools v1.54. Outgoing CPI wrappers live in `src/cpi_out/`.

Rules for the lanes:
1. **Character lane:** never deserialize a Core `Asset` (or `Collection`) on-chain: mpl-core's deserializers overflow the 4 KB stack frame. Death freeze = plugin update CPI only.
2. **VRF on localnet:** `programs/session` gets a cargo feature `mock-vrf`. With it, the `consume_randomness` signer check expects a fixed test keypair instead of the VRF identity; the rest of the callback is the **same** code path. Do not use `SlotHashesRoll` for tests. The devnet build must never enable `mock-vrf`: the deploy script (L8.3) fails if it is on. L8.3 adds one devnet smoke test with the real VRF oracle.
3. **All lanes build with** `anchor build --tools-version v1.54` (cargo-build-sbf 4.1.0 panics on Windows with Anchor's default v1.57; one version everywhere keeps builds identical).
4. **TS tests** (`tests/<program>.test.ts`, shared validator; on Windows `anchor test` cannot run its script, so call node directly). After `anchor build -p <program>` (IDL + `target/types`) and `anchor deploy -p <program> --provider.cluster localnet`:
   `cd tests && ANCHOR_PROVIDER_URL=http://127.0.0.1:8899 ANCHOR_WALLET=<funded keypair json> node --import tsx --test --test-concurrency=1 <program>.test.ts`
   Import Anchor as `import anchor from "@anchor-lang/core"` (named ESM imports such as `BN` fail on its CJS build).

## Decision: one shared local validator (H, 2026-10-11, written by L2)

One shared local validator runs in WSL at `http://127.0.0.1:8899`, started by H. Session Keys and Metaplex Core are cloned from devnet into it; VRF uses the `mock-vrf` feature. (Native Windows `solana-test-validator` cannot create its ledger, and LiteSVM's agave crates need rustc > 1.92, so neither is used.)

1. **Never start, stop or reset a validator yourself.**
2. **Deploy only your own program:** `anchor deploy -p <your_program> --provider.cluster localnet`. Never deploy other lanes' programs.
3. **Run tests with** `anchor test --skip-local-validator --skip-deploy` (plus `-p <program>` / your test file).
4. **Tests use fresh random keypairs for everything** and random ids for PDAs keyed by an id (versions, indices), so runs and lanes never collide. They must **tolerate singletons that already exist** (e.g. `GameConfig`): if it exists, read it instead of failing.
5. If you need another lane's program that is not deployed yet, test what you can, skip the rest with a clear reason in the test output, and note it in the PR.

Legend: **C** = Claude agent, **G** = Grok (via `delegate-to-grok`, reviewed by the lane's Claude), **H** = human (Ivan).
Each lane works in its own worktree: `git worktree add ../deadroll-<lane> -b lane/<lane>`.
Write your name next to a task when you take it. Tick when merged to `main`.

## Checkpoints (Warsaw time, demo deadline, historical)

| When | What must be true |
|---|---|
| Sun 14:00 | L0 merged: workspace builds, IDs fixed, `crates/types` frozen, dependency spike answered |
| Sun 22:00 | all crates green; registry, render, character, map programs green locally |
| Mon 06:00 | session P0 green locally; integration test "create -> map -> party -> fight -> death -> victory" passes |
| Mon 12:00 | everything on devnet, app plays the demo script |
| Mon 15:00 | code freeze, record video (H) |

If a checkpoint slips by more than 2 hours, the lead moves the lowest P0 item to its fallback (`CLAUDE.md`, section 0).

---

## L0. Lead: contracts and scaffold (sequential, blocks everyone)

- [x] **L0.1 (C)** Claude (lead L0). Workspace: `Anchor.toml` with 7 programs, root `Cargo.toml` workspace, `rust-toolchain.toml`, pnpm workspace (`sdk`, `app`, `tests`, `tools/scripts`), `.gitignore` (keys, `target/`, `node_modules/`, `.env`, `test-ledger/`, `.anchor/`). Empty programs build.
- [x] **L0.2 (C)** Claude (lead L0). Fill program IDs in `agents/CONTRACTS.md` and `Anchor.toml`. Keypairs stay local; tell H to back them up.
- [x] **L0.3 (C)** Claude (lead L0). `crates/types` exactly as `CONTRACTS.md` section 3-4, with Borsh + `InitSpace`-compatible sizes and round-trip tests.
- [x] **L0.4 (C) Dependency spike, 60 min max** Claude (lead L0): a throwaway program that compiles with Anchor 1.0 + `ephemeral-vrf-sdk` + `session-keys` + `mpl-core`. Report per crate: OK / needs pin / fallback. Lead decides and writes the decision at the top of this file.
- [ ] **L0.5 (G)** Zone `README.md` + `CLAUDE.md` stubs in every program and crate folder from the template in `agents/zone-claude-template.md`. Partly done: some zones have them, others do not yet.
- [x] **L0.6 (C)** Claude (lead L0). CI skeleton: `crates.yml` (fmt, clippy, test). Others later.

## L1. Combat crate (`crates/combat`)

- [x] **L1.1 (C)** Claude (lane L1). `rng.rs`: `RollSource`, `SeededRoll`, `VrfRoll(seed: [u8;32])` with `hash(seed ‖ index)`, `d20`, `d100`, `chance`. (#4)
- [x] **L1.2 (C)** Claude (lane L1). Modifiers, packets, conversion by shares, armor, resistances, counterweights. (#4)
- [x] **L1.3 (C)** Claude (lane L1). States P0 (burning, chill -> freeze, bleeding), one instance per kind. (#4)
- [x] **L1.4 (C)** Claude (lane L1). Resonance pairs (d100 per tint per skill aspect, no cap). (#4)
- [x] **L1.5 (C)** Claude (lane L1). `resolve_hit -> HitResult` + golden test. (#4)
- [x] **L1.6 (G)** Grok, reviewed by Claude (lane L1). Property tests and edge-case tests (only `crates/combat/tests/**`). (#4)
- [x] **L1.7 (C)** Claude (lane L1). WASM bindings `crates/combat/wasm` (C ABI or wasm-bindgen, lead's choice) for the app. (#4)
- [ ] P1 **L1.8** Alloys, collisions, reactions. P2 triples.

## L2. Render crate + render program

- [x] **L2.1 (C)** Claude (L2). `crates/render`: sprite DSL decoder (format in `crates/render/FORMAT.md`, from `docs/ART_DESCRIPTION.md`), rasterize commands, auto shading, outline, layer order, ramps -> RGBA. (#7)
- [x] **L2.2 (C)** Claude (L2). PNG encoder (stored deflate blocks, CRC32, Adler32), integer scaling. (#7)
- [x] **L2.3 (C)** Claude (L2). C ABI exports + `wasm32-unknown-unknown` build, size budget 60 KB after `wasm-opt -Oz`. (#7)
- [x] **L2.4 (G)** Claude (lane L3). `tools/art`: CLI that renders a DSL file to PNG (x8) for preview; golden image tests. (#6)
- [x] **L2.5 (C)** Claude (L2). `programs/render`: `init_rasterizer`, `write_chunk(offset, bytes)`, `finalize_rasterizer` (SHA-256, freeze; no CPI, registry pulls, see L4.1), `init_palette`, `write_sprites`. (#9)
- [x] **L2.6 (G)** Claude (L2). `tools/scripts/upload-rasterizer.ts` and `upload-sprites.ts` (chunking, resume, verify hash). (#9)
- [ ] P1 **L2.7** Tombstone and wound overlays.

## L3. Art (mostly Grok, short sessions)

Each task = one fresh Grok session, writes only in `assets/sprites/<name>.dsl`, validated by `tools/art`.
- [x] **L3.1 (G)** Claude (lane L3). Body human, body forest elf (skin and hair ramps). (#6)
- [x] **L3.2 (G)** Claude (lane L3). Warrior kit: helmet, armor, pants, boots, sword, shield. (#6)
- [x] **L3.3 (G)** Claude (lane L3). Mage kit: hood, robe, staff with element core. (#6)
- [x] **L3.4 (G)** Claude (lane L3). Support kit. (#6)
- [x] **L3.5 (G)** Claude (lane L3). Monsters: wolf, goblin, goblin chief (boss), tombstone. (#6)
- [x] **L3.6 (G)** Claude (lane L3). Tiles: floor, wall, water, door, entrance, exit, trap. (#6)
- [x] **L3.7 (C)** Claude (lane L3). Review pass: silhouettes, anchors, consistency. (#6)

## L4. Registry program

- [x] **L4.1 (C)** Claude (lane L4). `init_config`, `set_programs`, `set_rasterizer`, `set_uri_base`. `set_rasterizer` is authority-only and pulls (no CPI from render, decided in L2): it takes the `RasterizerCode` account, checks `owner == GameConfig.programs[Render]` and `finalized`, then copies version, address and sha256 (header offsets in `programs/render/README.md`). Same pattern later for `Palette` and `SpriteBook`.
- [x] **L4.2 (C)** Claude (lane L4). Tables: aspects, resonances, bestiary, tileset, prefabs (`write_table(kind, offset, bytes)`).
- [x] **L4.3 (G)** Grok, reviewed by Claude (lane L4). `assets/registry/*.json` from docs (aspects, resonance pairs, bestiary P0 monsters, prefab rooms) + `tools/scripts/seed-registry.ts`.

## L5. Character program + skillcheck

- [x] **L5.1 (C)** Claude (lane L5). `create_profile`, `create_character` (race, class, predisposition, stats, preset skills, appearance) + Core NFT mint via CPI (feature `nft`). (#8)
- [x] **L5.2 (C)** Claude (lane L5). CPI targets from `CONTRACTS.md` section 5 with session PDA check. (#8)
- [x] **L5.3 (C)** Claude (lane L5). `crates/skillcheck`: preset skills valid, class restriction on starting skills. (#8)
- [x] **L5.4 (G)** Grok, reviewed by Claude (lane L5). Tests for skillcheck rejections. (#8)
- [ ] P1 **L5.5** NFT `uri` update with `state_version`. P2 skill builder instructions, lenses.

## L6. Map program + mapcheck

- [x] **L6.1 (C)** Claude (lane L6). `crates/mapcheck`: bounds, entrance/exit, spawns on walkable cells, path entrance -> exit (BFS), packs. (#11)
- [x] **L6.2 (G)** Grok, reviewed by Claude (lane L6). BFS + tests in `crates/mapcheck/src/path.rs` only. (#11)
- [x] **L6.3 (C)** Claude (lane L6). `programs/map`: `create_map`, `write_layout`, `finalize_map` (runs mapcheck, stores hash), `set_status` (session PDA only). (#11)
- [x] **L6.4 (G)** Grok, reviewed by Claude (lane L6). `assets/registry/maps/demo_16x16.json` preset (wolves pack, goblins pack, goblin chief + helpers). (#11)
- [ ] P1 **L6.5** Commit-reveal trap (`docs/MAP_DESCRIPTION.md`, section 8).

## L7. Session program (critical path)

- [x] **L7.1 (C)** Claude (lane L7). Accounts + `open_session`, `join_session` (escrow), `set_ready`, `start_session`. (#10)
- [x] **L7.2 (C)** Claude (lane L7). `auth::check_actor`: Session Keys token or fallback actor. (#10)
- [x] **L7.3 (C)** Claude (lane L7). Turn order (round-robin first, initiative timeline P1), action points from speed, `move`, `end_turn`. (#10)
- [x] **L7.4 (C)** Claude (lane L7). `act` / `monster_act` -> VRF request; `consume_randomness` callback -> `deadroll_combat::resolve_hit` -> apply HP, states, death CPI, boss victory. (#13, #15)
- [x] **L7.5 (C)** Claude (lane L7). Outcomes: victory rewards CPI, wipe, flee by unanimity, DM claims escrow. (#13)
- [x] **L7.6 (G)** Rejection tests (wrong actor, wrong turn, dead, out of AP, not adjacent, ended session). (in `tests/session.test.ts`, #13, #23, #25)
- [ ] P1 **L7.7** Initiative timeline, temp stash loot, exit penalty.

## L8. SDK + integration

- [ ] **L8.1 (C)** Codama generation script for all IDLs into `sdk/`.
- [x] **L8.2 (C)** Claude (lane L8). `tests/demo.test.ts`: the full demo script on local validator with cloned MagicBlock programs; `tools/scripts/deploy-local.ts` sets the stack up. (#17)
- [x] **L8.3 (C)** Claude (lane L8). `tools/scripts/deploy-devnet.ts` (programs without mock-vrf, registry, rasterizer, sprites, demo map; cost estimate first). Devnet smoke test with the real VRF oracle: `tests/smoke-devnet.ts`. (#17, #19)

## L9. App

- [x] **L9.1 (C)** Claude (L9). App shell: wallet adapter, session key start, routes (Character, Map builder, Party), data layer (`app/src/chain/` until `sdk/` is generated). (#14)
- [x] **L9.2 (C)** Claude (L9). Rasterizer loader: read `GameConfig`, fetch `RasterizerCode`, verify SHA-256, cache in IndexedDB, render to canvas. (#14)
- [x] **L9.3 (G)** Grok, reviewed by Claude (L9). Grid canvas component (tiles, units, move highlight, click to move/target), props only, mock data. (#14)
- [x] **L9.4 (G)** Grok, reviewed by Claude (L9). Dice cascade animation + hit breakdown panel from a `HitResolved` event, mock data. (#14)
- [x] **L9.5 (G)** Grok, reviewed by Claude (L9). Character creation form, map builder palette UI, party lobby UI (mock data). (#14)
- [x] **L9.6 (C)** Wire components to chain, event subscriptions, error toasts. (#18, #20)
- [ ] **L9.7 (H)** Visual pass for the video.

## After the demo: app polish, turns, phase 2

- [x] **App fixes (L9):** high-roll dice display, resilient log subscriptions, dead-monster move fix, character detail view. (#20)
- [x] **Skill builder (L9):** skill builder at character creation, TypeScript mirror of `crates/skillcheck` tested against Rust fixtures. (#21)
- [x] **RPC policy (L9):** stream-first refresh, HTTP 429 retry, Custom RPC in the header, no provider key in hosted builds. (#22)
- [x] **Turn rules (L7):** one action per turn, skill cooldowns, permissionless `start_turn` (DoT ticks, Freeze and Stun skip the turn). (#23)
- [x] **Turn rules in the app (L9):** automatic `start_turn`, one action per turn, cooldown labels, turn log. (#24)
- [x] **L7 phase 2 (L7):** support and control skills: Heal, buffs and debuffs as timed modifiers, effects of damage skills, all on the action's VRF value. (#25)
- [x] **Phase 2 in the app (L9):** ally targeting, buffs and debuffs panel, effect roll cascade, new events and errors. (#26)

## P2 (skeletons only during the deadline)

- [ ] items: `ItemState`, item level, affixes, NPC master craft, listings.
- [ ] city: buildings, storage, masters guild.
- [ ] worker for wallets.
- [x] **Balance bench:** `crates/rules` (session rules shared with the program, one `BalanceConfig`), `crates/sim` (fights, presets, rayon), `tools/balance` (egui app).
