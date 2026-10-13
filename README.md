# Deadroll

Deadroll is a fully on-chain tabletop RPG on Solana. Real Dungeon Masters run live sessions, and everything that matters happens on-chain: characters, skills, maps, turn-based combat, dice, deaths, and even the sprite renderer that draws the pixel art. Every dice roll is public. Every death leaves a mark.

- Video: TODO
- Website: TODO

## The game today

- **Modular skill builder.** At character creation a player builds up to 6 skills block by block: category, purpose, source, form, target, 1 to 3 aspects, effect, trigger and aura. Class presets are one click away. The character program checks every skill with `crates/skillcheck`. The website runs a TypeScript copy of the same rules, tested against fixtures generated from the Rust crate. It shows the first broken rule as you edit, along with power, cooldown and range.
- **Turns.** Every turn opens with a permissionless `start_turn`, which the app sends on its own. A unit gets one attack or active skill per turn and can still move with the action points it has left. Skills have cooldowns ("ready in N turns"). The battle log shows ticks, skips and skill use.
- **Support and control.** The game has:
  - Heal and buffs on allies: Empowerment, Protection, Haste.
  - Debuffs on enemies: Weakening, Vulnerability, Slow.
  - Freeze and Stun, which skip the target's turn.
  - Burning and Bleeding, which tick at the start of a turn.

  A damage skill can also carry an effect that rolls after the hit. Debuffs never subtract: they divide, as counterweights.
- **All dice through VRF.** Each action asks MagicBlock VRF for one random value. Every roll of that action (hit, crit, resonance, effect chance, heal amount) is derived from that value as `hash(seed || roll_index)`. The website replays each roll as a dice cascade with the full hit breakdown, and every event links to its transaction.
- **Player economy.** The DM builds a map, opens a party and sets its price. Players pay into an on-chain escrow when they join. At the end the surviving players get XP and loot, and the DM claims the escrow.
- **Permadeath.** A character that dies becomes a tombstone: its sprite changes and its Metaplex Core NFT is frozen.
- **On-chain art.** The rasterizer WASM, the palette and the sprite books are stored on-chain. The website loads the rasterizer from the chain and checks its SHA-256 against `GameConfig` before it draws anything.
- **No wallet popups in play.** Players start a MagicBlock session key once per party.

### Status

| Area | State |
|---|---|
| Shared crates (`types`, `combat`, `render`, `mapcheck`, `skillcheck`) | merged |
| `registry`, `render`, `character`, `map` programs | merged |
| `session`: lobby, escrow, VRF combat, death, victory, flee, payout | merged |
| `session`: turn rules (one action per turn, cooldowns, `start_turn`) | merged (#23), app #24 |
| `session`: heals, buffs, debuffs, skill effects | merged (#25), app #26 |
| App: full game on the local validator (DM and two players in one browser) | merged |
| Devnet | deployed, but runs older builds (see [Devnet](#devnet)) |
| TypeScript SDK (Codama), hosted website, CI for `app/` | next |
| `items`, `city`, worker for wallets, balance simulator | skeletons |

The task board, including what comes next, is [agents/TASKS.md](agents/TASKS.md).

## Devnet

These program ids are the same on every cluster (see `app/src/config.ts`):

| Program | Address |
|---|---|
| `registry` | `dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG` |
| `render` | `DgQCsXAe7Q7h8zdtToiTvk8XqNjjbXuD6xith2BmEiRB` |
| `character` | `9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL` |
| `items` | `48FtbrDegNrAUHFza9HdJzZAvUo3uUNqs1kNnZxFsqJS` |
| `map` | `2YeyBfQ5xkRKD2kagxRS2yZYCQdWt5qidikaKfQhLBXH` |
| `session` | `3gzfih2WaBgXcYZ57jWokmZM3FDE6QHX5y4R9EvKZoNJ` |
| `city` | `2Cr5vB5awqDVx6XRcxtFFYQEG6Zy3aSAcKcqzh1xbVjS` |

`GameConfig` on devnet: `HYGYnuyssMfrHr2gT3AC3hkQpD6xt37a5hrCc2Z97JHN`.

**Devnet is behind `main`.** It still runs the `session` and `character` builds from before #23 and #25. Turn rules, cooldowns, heals, buffs and debuffs work only on the local validator for now. Upgrading devnet is the first "Next" item on the task board.

## Architecture

```mermaid
flowchart LR
    subgraph Solana["Solana (on-chain)"]
        registry
        character
        items
        map
        session
        city
        render
    end
    subgraph Offchain["Off-chain"]
        app["app (website)"]
        worker["worker (for wallets, later)"]
        tools["tools (simulator, art, scripts)"]
    end
    VRF["MagicBlock VRF"]
    SK["Session Keys (MagicBlock)"]

    app --> session
    app --> character
    app --> items
    app --> map
    app --> city
    app -. reads .-> registry
    app -. reads .-> render
    session --> character
    session --> items
    session --> map
    session --> VRF
    session --> SK
    items --> character
    city --> character
    worker -. reads .-> render
    worker -. reads .-> character
```

There are seven Anchor programs, split by domain so that each part can change on its own:

| Program | Owns |
|---|---|
| `registry` | `GameConfig`, aspects, resonance and bestiary tables, tileset, prefabs |
| `character` | profile, character, skills, bag, Core NFT mint and freeze |
| `items` | item state, affixes, crafting (post-demo) |
| `map` | maps, validation, finalization, layout |
| `session` | party, turns, combat, buffs and debuffs, VRF callback, party price escrow |
| `city` | city and buildings (post-demo) |
| `render` | the rasterizer WASM, palette and sprite books, stored on-chain |

Shared logic lives in pure Rust crates with no Anchor dependency. The programs, the website (through WASM or a fixture-tested mirror) and the simulator all run the same rules:

| Crate | Purpose |
|---|---|
| `types` | shared types, frozen interfaces |
| `combat` | integer-only hit, heal, buff and debuff resolution; all dice go through `RollSource` |
| `skillcheck` | skill and loadout validation, derived power, cooldown and range |
| `mapcheck` | map validation (bounds, spawns, path from entrance to exit) |
| `render` | sprite DSL decoder, shading, outline, PNG encoder; builds to WASM |

Principles:

- a monorepo with one folder per zone;
- no backend: the site talks to the chain through the player's wallet and session key;
- the rasterizer is always loaded from the chain and checked against the hash in `GameConfig`.

Details are in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Tech stack

- **Programs:** Anchor 1.2.1, Solana CLI 4.1.2 (Agave), Rust 1.92.0, platform tools v1.54
- **On-chain services:** MagicBlock VRF (`ephemeral-vrf-sdk`) and Session Keys (`session-keys`), Metaplex Core (`mpl-core`)
- **Client:** Vite 8, React 19, TypeScript, `@solana/kit` 8 with the Wallet Standard plugin
- **Tooling:** Node 22 or newer, pnpm 10, Surfpool or a local validator

## Quick start

### 1. Quick look, no validator

The app has a mock mode that needs no chain. It still runs the real rasterizer loader, including the SHA-256 check, on synthetic accounts.

```
pnpm install
cargo test -p deadroll-render --test golden      # writes target/render-golden/*.bin
bash crates/render/wasm/build.sh                 # builds target/render-wasm/deadroll_render.wasm
pnpm --filter @deadroll/app dev                  # VITE_DATA=mock by default
```

Open `/dev/grid`, `/dev/dice` and `/dev/forms` to see the UI components on mock data.

### 2. Full game on a local validator

Native Windows `solana-test-validator` cannot create its ledger, so run the validator in WSL at `http://127.0.0.1:8899`. Clone Session Keys and Metaplex Core into it from devnet. Local VRF uses the `session` program's `mock-vrf` feature, which the deploy script turns on for localnet only.

One command deploys the whole stack:

```
pnpm --filter @deadroll/scripts run deploy:local
```

It does the following, and skips any step that is already done:

- builds the programs, with `mock-vrf` on for `session`;
- deploys the 7 programs and creates `GameConfig`;
- seeds the registry;
- uploads the rasterizer, palette and sprites;
- creates a demo map.

Then start the app in localnet mode. It reads chain data from `127.0.0.1:8899` and plays the mock VRF oracle:

```
pnpm --filter @deadroll/app dev:localnet
```

Open `http://localhost:5173/?dev=1` in three tabs or windows: DM, warrior, mage. With a local RPC, `?dev=1` gives each tab its own keypair and airdrops 5 SOL to it, so no wallet extension is needed. The app never offers it with a non-local RPC.

1. **Character:** each player creates a character and builds skills or takes the class presets. This mints a Core NFT.
2. **Map builder:** the DM validates and finalizes a map, then opens a party on it with a price.
3. **Party:** players join (they pay into escrow and bind a session key) and press Ready. The DM starts the session.
4. **Battle:** click a cell to move, a monster to attack, or an ally to heal or buff, then press End turn. The DM moves the monsters or lets them play on their own.
5. **End:** claim the rewards. The DM then claims the escrow.

[tools/scripts/README.md](tools/scripts/README.md) describes the script arguments and environment. [app/README.md](app/README.md) has the full play flow.

### 3. Devnet

```
pnpm --filter @deadroll/app dev:devnet
```

Devnet mode reads its non-secret values from `app/.env.devnet`. Set the RPC in one of two ways:

- **Custom RPC** in the app header: an RPC URL and an optional WebSocket URL. They stay in your browser's localStorage and are never bundled.
- `VITE_RPC_URL` in an untracked `app/.env.devnet.local`.

If neither is set, a hosted build uses the public devnet RPC. `vite build` refuses any `VITE_RPC_URL` other than the public devnet RPC or localhost, so no provider key ends up in the bundle. Devnet still runs the older builds (see [Devnet](#devnet)).

### Tests

```
cargo test -p deadroll-combat                    # one crate; same for the others
pnpm --filter @deadroll/app test
DEADROLL_LOCALNET=1 pnpm --filter @deadroll/app exec vitest run src/chain/localnet.e2e.test.ts
cd tests && ANCHOR_PROVIDER_URL=http://127.0.0.1:8899 ANCHOR_WALLET=<funded keypair json> \
  node --import tsx --test --test-concurrency=1 session.test.ts
```

Always pass `--tools-version v1.54` to `anchor build`. The default platform tools make `cargo-build-sbf` panic on Windows, and using one version everywhere keeps the builds identical.

## Repository layout

```
programs/   registry, character, items, map, session, city, render   (Anchor)
crates/     types, combat, skillcheck, mapcheck, render               (pure Rust, no Anchor)
sdk/        TypeScript client generated by Codama from the IDLs (not generated yet)
app/        Vite + React + TypeScript website
tests/      cross-program integration tests, devnet VRF smoke test
tools/      art/ (sprite preview and validator), scripts/ (deploy, uploads, registry seeding)
assets/     sprites/ (geometry DSL sources), registry/ (aspects, tables, bestiary, presets as JSON)
agents/     task board, frozen interfaces, agent guides
docs/       design documents
```

Every program, crate and app folder has its own `README.md` and a short `CLAUDE.md` with zone rules.

## Documentation

The design documents in [docs/](docs/) are the source of truth for game rules.

| Topic | Doc |
|---|---|
| Code split, CPI, trust, crates | [ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| Character, stats, races, classes | [CHARACTER.md](docs/CHARACTER.md) |
| Aspects, states, resonances, reactions | [DAMAGE_TYPES_AND_MIXES.md](docs/DAMAGE_TYPES_AND_MIXES.md) |
| Hit calculation, dice, modifiers | [COMBAT_MATH.md](docs/COMBAT_MATH.md) |
| Skills, lenses, loadout | [SKILL_BUILDER.md](docs/SKILL_BUILDER.md) |
| Party, turns, action points, rewards | [PARTY_AND_TURNS.md](docs/PARTY_AND_TURNS.md) |
| Monsters, packs, bosses | [BESTIARY.md](docs/BESTIARY.md) |
| Items, affixes, craft | [ITEMS_AND_CRAFT.md](docs/ITEMS_AND_CRAFT.md) |
| Map, tiles, spawns | [MAP_DESCRIPTION.md](docs/MAP_DESCRIPTION.md) |
| City and buildings | [CITY_DESCRIPTION.md](docs/CITY_DESCRIPTION.md) |
| Sprite DSL, slots, shading | [ART_DESCRIPTION.md](docs/ART_DESCRIPTION.md) |
| Rasterizer storage, upload, NFT uri | [STORAGE_AND_DELIVERY.md](docs/STORAGE_AND_DELIVERY.md) |
| Fees, session keys, VRF, party price | [GAME_COMMISSIONS.md](docs/GAME_COMMISSIONS.md) |

Contributor and agent rules are in [CLAUDE.md](CLAUDE.md). The frozen interfaces are in [agents/CONTRACTS.md](agents/CONTRACTS.md).
