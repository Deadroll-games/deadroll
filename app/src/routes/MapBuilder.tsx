import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import type { Address } from "@solana/kit";
import { Grid } from "../components/grid/Grid";
import type { GridUnit } from "../components/grid/types";
import { MapPalette } from "../components/forms/MapPalette";
import type { BuilderTool } from "../components/forms/types";
import { MAP_PRESETS, presetToLayout } from "../chain/mock";
import { BESTIARY_NAMES, bestiaryName } from "../chain/names";
import { SPAWNS_MAX, Tile, cellIndex, type Cell, type Spawn } from "../chain/types";
import { buildMapIxs, mapStatus, mapsOf, type MapAcc } from "../chain/game";
import { useTx } from "../chain/useTx";
import { DATA_SOURCE, explorerUrl } from "../config";
import { useAppWallet } from "../wallet/AppWallet";

const BESTIARY = Object.entries(BESTIARY_NAMES).map(([id, name]) => ({ id: Number(id), name }));
const PRESETS = MAP_PRESETS.map((p) => ({ name: p.name, width: p.width, height: p.height }));
const WALKABLE = new Set<number>([Tile.Floor, Tile.Door, Tile.Entrance, Tile.Exit, Tile.Trap]);

/**
 * Quick client-side checks so the DM sees obvious mistakes before paying for transactions.
 * The authority is crates/mapcheck inside map.finalize_map.
 * TODO(L9.6): run crates/mapcheck through WASM for the exact same verdict.
 */
export function quickCheck(tiles: Uint8Array, width: number, height: number, spawns: Spawn[]): string[] {
  const errors: string[] = [];
  let entrances = 0;
  let exits = 0;
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++) {
      const t = tiles[cellIndex(x, y)];
      if (t === Tile.Entrance) entrances++;
      if (t === Tile.Exit) exits++;
    }
  if (entrances !== 1) errors.push(`Need exactly one entrance, found ${entrances}`);
  if (exits !== 1) errors.push(`Need exactly one exit, found ${exits}`);
  if (spawns.length > SPAWNS_MAX) errors.push(`At most ${SPAWNS_MAX} spawns`);
  spawns.forEach((s, i) => {
    if (!WALKABLE.has(tiles[cellIndex(s.x, s.y)] ?? Tile.Wall)) errors.push(`Spawn ${i} (${bestiaryName(s.bestiaryId)}) is not on a walkable tile`);
  });
  return errors;
}

export function MapBuilderPage() {
  const first = MAP_PRESETS[0]!;
  const [size, setSize] = useState({ width: first.width, height: first.height });
  const [layout, setLayout] = useState(() => presetToLayout(first));
  const [tool, setTool] = useState<BuilderTool>({ kind: "tile", tile: Tile.Floor });
  const [errors, setErrors] = useState<string[]>([]);
  const [validated, setValidated] = useState(false);
  const [finalized, setFinalized] = useState(false);
  const [tier, setTier] = useState(first.tier ?? 1);
  const wallet = useAppWallet();
  const { send, busy } = useTx();
  const [built, setBuilt] = useState<Address | null>(null);
  const [myMaps, setMyMaps] = useState<{ address: Address; data: MapAcc }[]>([]);
  const refreshMaps = useCallback(async () => {
    if (DATA_SOURCE !== "chain" || !wallet.address) return setMyMaps([]);
    try {
      setMyMaps(await mapsOf(wallet.address));
    } catch {
      setMyMaps([]);
    }
  }, [wallet.address]);
  useEffect(() => void refreshMaps(), [refreshMaps]);

  // create_map, write_layout, write_spawns, finalize_map: the chain runs crates/mapcheck in finalize_map.
  const finalizeOnChain = async () => {
    const dm = wallet.signer;
    if (!dm) return void (await send("finalize_map", null, []));
    const b = await buildMapIxs(dm, { width: size.width, height: size.height, tier, ...layout });
    const names = ["create_map", "write_layout", "write_spawns", "finalize_map"];
    for (const [i, ix] of b.ixs.entries()) {
      if (!(await send(names[i]!, dm, [ix], { quiet: i < b.ixs.length - 1 }))) return;
    }
    setBuilt(b.map);
    setFinalized(true);
    await refreshMaps();
    void wallet.refreshBalance();
  };

  const edit = (next: { tiles: Uint8Array; spawns: Spawn[] }) => {
    setLayout(next);
    setValidated(false);
    setFinalized(false);
  };

  const onCell = ({ x, y }: Cell) => {
    if (tool.kind === "tile") {
      const tiles = layout.tiles.slice();
      tiles[cellIndex(x, y)] = tool.tile;
      edit({ ...layout, tiles });
    } else if (tool.kind === "spawn") {
      if (layout.spawns.some((s) => s.x === x && s.y === y) || layout.spawns.length >= SPAWNS_MAX) return;
      const pack = Math.max(0, ...layout.spawns.map((s) => s.pack)) + 1;
      edit({ ...layout, spawns: [...layout.spawns, { bestiaryId: tool.bestiaryId, x, y, level: 1, skills: 0, flags: 0, aspects: 0, pack }] });
    } else {
      edit({ ...layout, spawns: layout.spawns.filter((s) => s.x !== x || s.y !== y) });
    }
  };

  const units: GridUnit[] = useMemo(
    () =>
      layout.spawns.map((s, i) => ({ id: 4 + i, kind: "monster", x: s.x, y: s.y, hp: 1, hpMax: 1, alive: true, label: bestiaryName(s.bestiaryId) })),
    [layout.spawns],
  );

  return (
    <section>
      <h1>Map builder</h1>
      <div style={{ display: "flex", gap: 20, alignItems: "flex-start", flexWrap: "wrap" }}>
        <Grid
          width={size.width}
          height={size.height}
          tiles={layout.tiles}
          units={units}
          cellPx={32}
          onCellClick={onCell}
          onUnitClick={(u) => onCell({ x: u.x, y: u.y })}
        />
        <div style={{ flex: "1 1 320px" }}>
          <MapPalette
            tool={tool}
            onToolChange={setTool}
            bestiary={BESTIARY}
            presets={PRESETS}
            onLoadPreset={(name) => {
              const p = MAP_PRESETS.find((m) => m.name === name);
              if (!p) return;
              setSize({ width: p.width, height: p.height });
              setTier(p.tier ?? 1);
              edit(presetToLayout(p));
            }}
            spawns={layout.spawns}
            onRemoveSpawn={(i) => edit({ ...layout, spawns: layout.spawns.filter((_, j) => j !== i) })}
            errors={errors}
            onValidate={() => {
              const e = quickCheck(layout.tiles, size.width, size.height, layout.spawns);
              setErrors(e);
              setValidated(e.length === 0);
            }}
            canFinalize={validated && !finalized && busy === null}
            onFinalize={() => (DATA_SOURCE === "chain" ? void finalizeOnChain() : setFinalized(true))}
            busy={busy !== null}
          />
          {busy && <p className="muted">Sending {busy}...</p>}
          {finalized && DATA_SOURCE === "mock" && <p className="ok">Mock: map finalized.</p>}
          {built && (
            <p className="ok">
              Map finalized on chain:{" "}
              <a href={explorerUrl("address", built)} target="_blank" rel="noreferrer" className="mono">
                {built.slice(0, 8)}..
              </a>{" "}
              · <Link to={`/party?map=${built}`}>Open a party on it</Link>
            </p>
          )}
          {DATA_SOURCE === "chain" && myMaps.length > 0 && (
            <div className="panel">
              <h3>My maps</h3>
              <ul className="mono">
                {myMaps.map((m) => (
                  <li key={m.address}>
                    {m.address.slice(0, 8)}.. {m.data.core.width}x{m.data.core.height} · {mapStatus(m.data)}
                    {mapStatus(m.data) === "finalized" && (
                      <>
                        {" "}
                        · <Link to={`/party?map=${m.address}`}>open a party</Link>
                      </>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
