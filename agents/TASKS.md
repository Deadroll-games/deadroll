# TASKS: board for parallel agents

Legend: **C** = Claude agent, **G** = Grok (via `delegate-to-grok`, reviewed by the lane's Claude), **H** = human (Ivan).
Each lane works in its own worktree: `git worktree add ../deadroll-<lane> -b lane/<lane>`.
Write your name next to a task when you take it. Tick when merged to `main`.

## Checkpoints (Warsaw time)

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

- [ ] **L0.1 (C)** Workspace: `Anchor.toml` with 7 programs, root `Cargo.toml` workspace, `rust-toolchain.toml`, pnpm workspace (`sdk`, `app`, `tests`, `tools/scripts`), `.gitignore` (keys, `target/`, `node_modules/`, `.env`, `test-ledger/`, `.anchor/`). Empty programs build.
- [ ] **L0.2 (C)** Fill program IDs in `agents/CONTRACTS.md` and `Anchor.toml`. Keypairs stay local; tell H to back them up.
- [ ] **L0.3 (C)** `crates/types` exactly as `CONTRACTS.md` section 3-4, with Borsh + `InitSpace`-compatible sizes and round-trip tests.
- [ ] **L0.4 (C) Dependency spike, 60 min max**: a throwaway program that compiles with Anchor 1.0 + `ephemeral-vrf-sdk` + `session-keys` + `mpl-core`. Report per crate: OK / needs pin / fallback. Lead decides and writes the decision at the top of this file.
- [ ] **L0.5 (G)** Zone `README.md` + `CLAUDE.md` stubs in every program and crate folder from the template in `agents/zone-claude-template.md`.
- [ ] **L0.6 (C)** CI skeleton: `crates.yml` (fmt, clippy, test). Others later.

## L1. Combat crate (`crates/combat`)

- [ ] **L1.1 (C)** `rng.rs`: `RollSource`, `SeededRoll`, `VrfRoll(seed: [u8;32])` with `hash(seed ‖ index)`, `d20`, `d100`, `chance`.
- [ ] **L1.2 (C)** Modifiers, packets, conversion by shares, armor, resistances, counterweights.
- [ ] **L1.3 (C)** States P0 (burning, chill -> freeze, bleeding), one instance per kind.
- [ ] **L1.4 (C)** Resonance pairs (d100 per tint per skill aspect, no cap).
- [ ] **L1.5 (C)** `resolve_hit -> HitResult` + golden test.
- [ ] **L1.6 (G)** Property tests and edge-case tests (only `crates/combat/tests/**`).
- [ ] **L1.7 (C)** WASM bindings `crates/combat/wasm` (C ABI or wasm-bindgen, lead's choice) for the app.
- [ ] P1 **L1.8** Alloys, collisions, reactions. P2 triples.

## L2. Render crate + render program

- [ ] **L2.1 (C)** `crates/render`: sprite DSL decoder (format in `crates/render/FORMAT.md`, from `docs/ART_DESCRIPTION.md`), rasterize commands, auto shading, outline, layer order, ramps -> RGBA.
- [ ] **L2.2 (C)** PNG encoder (stored deflate blocks, CRC32, Adler32), integer scaling.
- [ ] **L2.3 (C)** C ABI exports + `wasm32-unknown-unknown` build, size budget 60 KB after `wasm-opt -Oz`.
- [ ] **L2.4 (G)** `tools/art`: CLI that renders a DSL file to PNG (x8) for preview; golden image tests.
- [ ] **L2.5 (C)** `programs/render`: `init_rasterizer`, `write_chunk(offset, bytes)`, `finalize_rasterizer` (SHA-256, freeze, write to `GameConfig` via registry authority), `init_palette`, `write_sprites`.
- [ ] **L2.6 (G)** `tools/scripts/upload-rasterizer.ts` and `upload-sprites.ts` (chunking, resume, verify hash).
- [ ] P1 **L2.7** Tombstone and wound overlays.

## L3. Art (mostly Grok, short sessions)

Each task = one fresh Grok session, writes only in `assets/sprites/<name>.dsl`, validated by `tools/art`.
- [ ] **L3.1 (G)** Body human, body forest elf (skin and hair ramps).
- [ ] **L3.2 (G)** Warrior kit: helmet, armor, pants, boots, sword, shield.
- [ ] **L3.3 (G)** Mage kit: hood, robe, staff with element core.
- [ ] **L3.4 (G)** Support kit.
- [ ] **L3.5 (G)** Monsters: wolf, goblin, goblin chief (boss), tombstone.
- [ ] **L3.6 (G)** Tiles: floor, wall, water, door, entrance, exit, trap.
- [ ] **L3.7 (C)** Review pass: silhouettes, anchors, consistency.

## L4. Registry program

- [ ] **L4.1 (C)** `init_config`, `set_programs`, `set_rasterizer` (callable only by render program PDA or authority), `set_uri_base`.
- [ ] **L4.2 (C)** Tables: aspects, resonances, bestiary, tileset, prefabs (`write_table(kind, offset, bytes)`).
- [ ] **L4.3 (G)** `assets/registry/*.json` from docs (aspects, resonance pairs, bestiary P0 monsters, prefab rooms) + `tools/scripts/seed-registry.ts`.

## L5. Character program + skillcheck

- [ ] **L5.1 (C)** `create_profile`, `create_character` (race, class, predisposition, stats, preset skills, appearance) + Core NFT mint via CPI (feature `nft`).
- [ ] **L5.2 (C)** CPI targets from `CONTRACTS.md` section 5 with session PDA check.
- [ ] **L5.3 (C)** `crates/skillcheck`: preset skills valid, class restriction on starting skills.
- [ ] **L5.4 (G)** Tests for skillcheck rejections.
- [ ] P1 **L5.5** NFT `uri` update with `state_version`. P2 skill builder instructions, lenses.

## L6. Map program + mapcheck

- [ ] **L6.1 (C)** `crates/mapcheck`: bounds, entrance/exit, spawns on walkable cells, path entrance -> exit (BFS), packs.
- [ ] **L6.2 (G)** BFS + tests in `crates/mapcheck/src/path.rs` only.
- [ ] **L6.3 (C)** `programs/map`: `create_map`, `write_layout`, `finalize_map` (runs mapcheck, stores hash), `set_status` (session PDA only).
- [ ] **L6.4 (G)** `assets/registry/maps/demo_16x16.json` preset (wolves pack, goblins pack, goblin chief + helpers).
- [ ] P1 **L6.5** Commit-reveal trap (`docs/MAP_DESCRIPTION.md`, section 8).

## L7. Session program (critical path)

- [ ] **L7.1 (C)** Accounts + `open_session`, `join_session` (escrow), `set_ready`, `start_session`.
- [ ] **L7.2 (C)** `auth::check_actor`: Session Keys token or fallback actor.
- [ ] **L7.3 (C)** Turn order (round-robin first, initiative timeline P1), action points from speed, `move`, `end_turn`.
- [ ] **L7.4 (C)** `act` / `monster_act` -> VRF request; `consume_randomness` callback -> `deadroll_combat::resolve_hit` -> apply HP, states, death CPI, boss victory.
- [ ] **L7.5 (C)** Outcomes: victory rewards CPI, wipe, flee by unanimity, DM claims escrow.
- [ ] **L7.6 (G)** Rejection tests (wrong actor, wrong turn, dead, out of AP, not adjacent, ended session).
- [ ] P1 **L7.7** Initiative timeline, temp stash loot, exit penalty.

## L8. SDK + integration

- [ ] **L8.1 (C)** Codama generation script for all IDLs into `sdk/`.
- [ ] **L8.2 (C)** `tests/demo.spec.ts`: the full demo script on local validator with cloned MagicBlock programs.
- [ ] **L8.3 (G)** `tools/scripts/deploy-devnet.sh` + `seed-devnet.ts` (registry, rasterizer, sprites, demo map).

## L9. App

- [ ] **L9.1 (C)** App shell: wallet adapter, session key start, routes (Character, Map builder, Party), data layer over `sdk/`.
- [ ] **L9.2 (C)** Rasterizer loader: read `GameConfig`, fetch `RasterizerCode`, verify SHA-256, cache in IndexedDB, render to canvas.
- [ ] **L9.3 (G)** Grid canvas component (tiles, units, move highlight, click to move/target), props only, mock data.
- [ ] **L9.4 (G)** Dice cascade animation + hit breakdown panel from a `HitResolved` event, mock data.
- [ ] **L9.5 (G)** Character creation form, map builder palette UI, party lobby UI (mock data).
- [ ] **L9.6 (C)** Wire components to chain, event subscriptions, error toasts.
- [ ] **L9.7 (H)** Visual pass for the video.

## P2 (skeletons only during the deadline)

- [ ] items: `ItemState`, item level, affixes, NPC master craft, listings.
- [ ] city: buildings, storage, masters guild.
- [ ] worker for wallets.
- [ ] simulator `tools/sim`.
