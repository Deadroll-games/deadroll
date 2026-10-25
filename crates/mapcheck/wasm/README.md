# deadroll-mapcheck-wasm

The map builder rules of `deadroll-mapcheck` for the website (docs/MAP_DESCRIPTION.md 1, 10): the page
gets the verdict `finalize_map` would give a draft before it sends anything. Patch 13.

`check` decodes the registry tables with `deadroll_rules` (`tiles::tile_rules`, `map_tables`, the tier
loot of `BalanceConfig::DEFAULT`) and runs `check_map`, the same calls as `finalize_map`.

## ABI (version 1)

Static buffers, no imports. The module has a 4096-byte input buffer.

| Export | In | Returns |
|---|---|---|
| `abi_version()` | | `ABI_VERSION` |
| `buffer_len()` | | 4096 |
| `input_ptr()` | | address of the input buffer |
| `set_table(kind, len)` | `input[..len]`: a registry table body as written on chain | 0, or an error |
| `check_map(len)` | `input[..len]`: borsh `MapCheckRequest` | 0 = valid, `1 + MapCheckError::code` = the first broken rule, or an error |

`kind`: 0 `bestiary`, 1 `monster_skills`, 2 `rewards`, 3 `tileset`. The bodies stay loaded until replaced.

`MapCheckRequest` = `width u8, height u8, tier u8, min_players u8` (the `Map` fields), then
`MapLayoutDataV2` (1301 bytes: version, map key, 1024 cell bytes, 24 spawns of 9 bytes, spawn count,
6 points of 4 bytes, point count, boss table, bump). The version, the map key and the bump are not
checked.

Errors: -1 input longer than the buffer, -2 not a `MapCheckRequest` (or trailing bytes), -3 unknown
table kind.

## Build and check

```
bash crates/mapcheck/wasm/build.sh          # rebuild app/src/chain/wasm/mapcheck.wasm (docker)
bash crates/mapcheck/wasm/build.sh --check  # CI: fail if the committed file differs
UPDATE_TS_FIXTURES=1 cargo test -p deadroll-mapcheck-wasm --test ts_fixtures   # app/src/chain/__fixtures__/mapcheck-wasm.json
```

The website side is `app/src/chain/mapcheckWasm.ts`; its test replays every fixture case on the
committed module. The app localnet e2e checks that `finalize_map` gives the same verdict.
