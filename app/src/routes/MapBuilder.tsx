import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import type { Address } from "@solana/kit";
import { Grid } from "../components/grid/Grid";
import type { GridChest, GridPointKind, GridTile, GridUnit } from "../components/grid/types";
import { MapPalette } from "../components/forms/MapPalette";
import { PrepayConfirm } from "../components/forms/PrepayConfirm";
import type { BuilderTool } from "../components/forms/types";
import { MAP_PRESETS, presetToLayout } from "../chain/mock";
import { BESTIARY_NAMES, BIOME_NAMES, bestiaryName } from "../chain/names";
import { chestRequest, lookRequest, pointLook, tileLook, tileVariants } from "../chain/appearance";
import { useSprites } from "../raster/useSprites";
import { SPAWNS_MAX, Tile, cellByte, cellIndex, type Cell, type Spawn } from "../chain/types";
import { MAPCHECK_MESSAGES, useMapcheck, type MapCheckDraft, type MapcheckWasm } from "../chain/mapcheckWasm";
import { buildMapIxs, draftPoints, mapRent, mapStatus, mapsOf, type MapAcc } from "../chain/game";
import {
  CHESTS_MAX,
  CHEST_RADIUS_MAX,
  CHEST_RADIUS_MIN,
  MIN_PLAYERS_MAX,
  SETTLE_TIMEOUT_MAX_SECS,
  SETTLE_TIMEOUT_MIN_SECS,
  NO_TABLE,
  budgetOf,
  levelsLabel,
  rewardTables,
  settleLabel,
  spentOf,
  tableAllowed,
  tierLoot,
  type Chest,
  type RewardTable,
} from "../chain/loot";
import { useTx } from "../chain/useTx";
import { DATA_SOURCE, explorerUrl } from "../config";
import { useAppWallet } from "../wallet/AppWallet";
import { useCombat } from "../chain/combatWasm";
import { SPAWN_ELITE, spawnView, useBuilderRegistry, type SpawnView } from "../chain/monsters";
import { SKILL_ERRORS, SKILL_ERROR_MESSAGES } from "../chain/skillcheck";

const CATEGORY_NAMES = ["active", "reactive", "passive"];

/** The verdict on a spawn's skills as a message, null = valid (or not known yet). */
export function spawnSkillError(view: SpawnView | null): string | null {
  const code = view?.check?.error ?? 0;
  if (!code) return null;
  const err = SKILL_ERRORS[code - 1];
  return err ? SKILL_ERROR_MESSAGES[err] : `skill error ${code}`;
}

const BESTIARY = Object.entries(BESTIARY_NAMES).map(([id, name]) => ({ id: Number(id), name }));
const PRESETS = MAP_PRESETS.map((p) => ({ name: p.name, width: p.width, height: p.height }));
const same = (a: Cell | null, b: Cell) => a !== null && a.x === b.x && a.y === b.y;

/**
 * The verdict finalize_map would give the draft: crates/mapcheck through WASM, on the registry
 * tables the program reads (docs/MAP_DESCRIPTION.md 1, 10). Empty = the map is valid. A spawn's skill
 * choice also gets its own line (the combat module's check, `spawnSkillError`).
 */
export function mapVerdict(mapcheck: MapcheckWasm | null, draft: MapCheckDraft, skillError: (s: Spawn) => string | null = () => null): string[] {
  if (!mapcheck) return ["The map checker is still loading"];
  const rule = mapcheck.check(draft);
  if (!rule) return [];
  const spawns = rule.startsWith("MonsterSkill")
    ? draft.spawns.flatMap((sp, i) => {
        const e = skillError(sp);
        return e ? [`Spawn ${i} (${bestiaryName(sp.bestiaryId)}): ${e}`] : [];
      })
    : [];
  return [MAPCHECK_MESSAGES[rule], ...spawns];
}

export function MapBuilderPage() {
  const first = MAP_PRESETS[0]!;
  const [size, setSize] = useState({ width: first.width, height: first.height });
  const [layout, setLayout] = useState(() => presetToLayout(first));
  const [tool, setTool] = useState<BuilderTool>({ kind: "tile", tile: Tile.StoneFloor, variant: 0 });
  // The tiles in the map's biome (the on-chain rasterizer), flat colors until it is loaded.
  const draw = useSprites();
  const [errors, setErrors] = useState<string[]>([]);
  const [validated, setValidated] = useState(false);
  const [finalized, setFinalized] = useState(false);
  const [tier, setTier] = useState(first.tier ?? 1);
  const wallet = useAppWallet();
  const { send, busy } = useTx();
  const [built, setBuilt] = useState<Address | null>(null);
  // The bestiary pools and monster skills (registry), checked by the combat module.
  const combat = useCombat();
  const registry = useBuilderRegistry();
  const tables = useMemo(() => (registry ? rewardTables(registry.rewards) : []), [registry]);
  // Patch 13: the builder rules of finalize_map, from the same registry tables.
  const loadedMapcheck = useMapcheck();
  const mapcheck = useMemo(() => {
    if (!loadedMapcheck || !registry) return null;
    loadedMapcheck.setTables({ bestiary: registry.bestiary, monsterSkills: registry.skills, rewards: registry.rewards, tileset: registry.tileset });
    return loadedMapcheck;
  }, [loadedMapcheck, registry]);
  const [rent, setRent] = useState<bigint | null>(null);
  const viewOf = useCallback((sp: Pick<Spawn, "bestiaryId" | "flags" | "skills">) => (combat && registry ? spawnView(combat, registry, sp) : null), [combat, registry]);
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

  // create_map, write_layout, write_spawns, write_points, finalize_map: the chain runs
  // crates/mapcheck in finalize_map.
  const finalizeOnChain = async () => {
    setRent(null);
    const dm = wallet.signer;
    if (!dm) return void (await send("finalize_map", null, []));
    const b = await buildMapIxs(dm, { width: size.width, height: size.height, tier, ...layout });
    const names = ["create_map", "write_layout", "write_spawns", "write_points", "finalize_map"];
    for (const [i, ix] of b.ixs.entries()) {
      if (!(await send(names[i]!, dm, [ix], { quiet: i < b.ixs.length - 1 }))) return;
    }
    setBuilt(b.map);
    setFinalized(true);
    await refreshMaps();
    void wallet.refreshBalance();
  };

  // create_map pays rent for two accounts: show the amount before anything is signed.
  const askRent = async () => {
    try {
      setRent(await mapRent());
    } catch {
      setErrors(["Could not read the rent for the map accounts"]);
    }
  };

  const edit = (next: typeof layout) => {
    setLayout(next);
    setValidated(false);
    setFinalized(false);
  };

  const onCell = ({ x, y }: Cell) => {
    const cell = { x, y };
    const onPoint = same(layout.entrance, cell) || same(layout.exit, cell);
    if (tool.kind === "tile") {
      const tiles = layout.tiles.slice();
      // The brush overwrites the whole cell: its tile type and the variant the DM picked.
      tiles[cellIndex(x, y)] = cellByte(tool.tile, tool.variant);
      edit({ ...layout, tiles });
    } else if (tool.kind === "entrance" || tool.kind === "exit") {
      const taken = layout.chests.some((c) => c.x === x && c.y === y) || layout.spawns.some((s) => s.x === x && s.y === y) || onPoint;
      if (!taken) edit({ ...layout, [tool.kind]: cell });
    } else if (tool.kind === "chest") {
      const limit = Math.min(CHESTS_MAX, tierLoot(tier)?.chests ?? 0);
      const taken = layout.chests.some((c) => c.x === x && c.y === y) || layout.spawns.some((s) => s.x === x && s.y === y);
      if (taken || onPoint || layout.chests.length >= limit) return;
      // The first table the tier allows; the DM picks another in the loot panel.
      const table = tables.find((t) => t.cost > 0 && tableAllowed(t, tier))?.id ?? NO_TABLE;
      edit({ ...layout, chests: [...layout.chests, { x, y, table }] });
    } else if (tool.kind === "spawn") {
      if (onPoint || layout.chests.some((c) => c.x === x && c.y === y)) return;
      if (layout.spawns.some((s) => s.x === x && s.y === y) || layout.spawns.length >= SPAWNS_MAX) return;
      const pack = Math.max(0, ...layout.spawns.map((s) => s.pack)) + 1;
      // The default choice fills the slots (the first skills of each kind of the pool).
      const skills = viewOf({ bestiaryId: tool.bestiaryId, flags: 0, skills: 0 })?.check?.defaultMask ?? 0;
      edit({ ...layout, spawns: [...layout.spawns, { bestiaryId: tool.bestiaryId, x, y, level: 1, skills, flags: 0, aspects: 0, pack }] });
    } else {
      edit({
        ...layout,
        spawns: layout.spawns.filter((s) => s.x !== x || s.y !== y),
        chests: layout.chests.filter((c) => c.x !== x || c.y !== y),
        entrance: same(layout.entrance, cell) ? null : layout.entrance,
        exit: same(layout.exit, cell) ? null : layout.exit,
      });
    }
  };

  const chests: GridChest[] = useMemo(
    () => layout.chests.map((c) => ({ x: c.x, y: c.y, opened: false, label: `Chest: ${levelsLabel(tables.find((t) => t.id === c.table))}` })),
    [layout.chests, tables],
  );

  const tileSprite = (cell: GridTile) => {
    const look = tileLook(layout.biome, cell);
    return look ? draw(`t:${look.join(",")}`, lookRequest(look)) : null;
  };
  const pointSprite = (kind: GridPointKind) => {
    const look = pointLook(kind, layout.biome);
    return draw(`p:${look.join(",")}`, lookRequest(look));
  };
  const chestSprite = (opened: boolean) => draw(`chest:${opened}`, chestRequest(opened));
  const tilePreview = (tile: Tile, variant: number) => tileSprite({ tile, variant, mask: 0 });

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
          chests={chests}
          points={draftPoints(layout)}
          cellPx={32}
          tileSprite={tileSprite}
          pointSprite={pointSprite}
          chestSprite={chestSprite}
          onCellClick={onCell}
          onUnitClick={(u) => onCell({ x: u.x, y: u.y })}
        />
        <div style={{ flex: "1 1 320px" }}>
          <MapPalette
            tool={tool}
            onToolChange={setTool}
            biome={layout.biome}
            biomes={BIOME_NAMES}
            onBiomeChange={(biome) => edit({ ...layout, biome })}
            tileVariants={(tile) => tileVariants(layout.biome, tile)}
            tilePreview={tilePreview}
            bestiary={BESTIARY}
            presets={PRESETS}
            onLoadPreset={(name) => {
              const p = MAP_PRESETS.find((m) => m.name === name);
              if (!p) return;
              setSize({ width: p.width, height: p.height });
              setTier(p.tier ?? 1);
              edit(presetToLayout(p));
            }}
            loot={<LootEditor tier={tier} tables={tables} locked={busy !== null} value={layout} onChange={(next) => edit({ ...layout, ...next })} />}
            spawns={layout.spawns}
            onRemoveSpawn={(i) => edit({ ...layout, spawns: layout.spawns.filter((_, j) => j !== i) })}
            renderSpawn={(spawn, i) => (
              <SpawnEditor
                spawn={spawn}
                view={viewOf(spawn)}
                viewOf={viewOf}
                locked={busy !== null}
                onChange={(next) => edit({ ...layout, spawns: layout.spawns.map((x, j) => (j === i ? next : x)) })}
              />
            )}
            errors={errors}
            onValidate={() => {
              const e = mapVerdict(mapcheck, { width: size.width, height: size.height, tier, ...layout }, (sp) => spawnSkillError(viewOf(sp)));
              setErrors(e);
              setValidated(e.length === 0);
            }}
            canFinalize={validated && !finalized && busy === null && rent === null}
            onFinalize={() => (DATA_SOURCE === "chain" ? void askRent() : setFinalized(true))}
            busy={busy !== null}
          />
          {rent !== null && (
            <PrepayConfirm
              action="Finalize"
              lamports={rent}
              detail="as rent for the map's two accounts"
              busy={busy !== null}
              onConfirm={() => void finalizeOnChain()}
              onCancel={() => setRent(null)}
            />
          )}
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
                    {m.address.slice(0, 8)}.. {m.data.core.width}x{m.data.core.height} · {BIOME_NAMES[m.data.biome] ?? BIOME_NAMES[0]} · {mapStatus(m.data)}
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

/** A spawn's level, elite flag and skills from its template's pool (docs/BESTIARY.md 12,
 * MAP_DESCRIPTION.md 7.2). The tier's levels and the elite limit are finalize_map's to check. */
function SpawnEditor(p: {
  spawn: Spawn;
  view: SpawnView | null;
  viewOf: (s: Pick<Spawn, "bestiaryId" | "flags" | "skills">) => SpawnView | null;
  locked: boolean;
  onChange: (s: Spawn) => void;
}) {
  const { spawn, view } = p;
  const elite = (spawn.flags & SPAWN_ELITE) !== 0;
  const error = spawnSkillError(view);
  const slots = view?.check?.slots;
  return (
    <div className="forms-spawn-editor">
      <label>
        Level{" "}
        <input
          type="number"
          min={1}
          max={255}
          value={spawn.level}
          disabled={p.locked}
          onChange={(e) => p.onChange({ ...spawn, level: Math.max(1, Math.min(255, Number(e.target.value) || 1)) })}
        />
      </label>{" "}
      <label title={view?.boss ? "A boss cannot be elite" : undefined}>
        <input
          type="checkbox"
          checked={elite}
          disabled={p.locked || !view || view.boss}
          onChange={() => {
            // Elite changes the slots: start from the new default choice.
            const flags = spawn.flags ^ SPAWN_ELITE;
            const skills = p.viewOf({ ...spawn, flags })?.check?.defaultMask ?? spawn.skills;
            p.onChange({ ...spawn, flags, skills });
          }}
        />{" "}
        Elite
      </label>
      {view && view.pool.length > 0 && (
        <fieldset className="forms-spawn-skills" disabled={p.locked}>
          <legend className="muted">
            Skills{slots ? `: ${slots.map((n, c) => `${n} ${CATEGORY_NAMES[c]}`).join(", ")}` : ""}
          </legend>
          {view.pool.map((k) => (
            <label key={k.bit}>
              <input type="checkbox" checked={(spawn.skills & (1 << k.bit)) !== 0} onChange={() => p.onChange({ ...spawn, skills: spawn.skills ^ (1 << k.bit) })} /> {k.name}{" "}
              <span className="muted">({CATEGORY_NAMES[k.category] ?? k.category})</span>
            </label>
          ))}
        </fieldset>
      )}
      {error && <p className="error">{error}</p>}
    </div>
  );
}

/** What the loot panel edits. */
type LootValue = { minPlayers: number; chests: Chest[]; bossTable: number; chestRadius: number; settleTimeoutSecs: number };

/** Times to settle the builder offers: 10 minutes to 10 hours (LootTuning limits). */
const SETTLE_CHOICES = [600, 1800, 3600, 7200, 14_400, 36_000].filter((x) => x >= SETTLE_TIMEOUT_MIN_SECS && x <= SETTLE_TIMEOUT_MAX_SECS);

/** Patch 11: the map's minimum players, the boss table and each chest's table, with the budget
 * (docs/MAP_DESCRIPTION.md 7.1, docs/PARTY_AND_TURNS.md 9.2). Only tables the tier allows. */
function LootEditor(p: {
  tier: number;
  tables: RewardTable[];
  locked: boolean;
  value: LootValue;
  onChange: (next: LootValue) => void;
}) {
  const { minPlayers, chests, bossTable } = p.value;
  const allowed = p.tables.filter((t) => tableAllowed(t, p.tier));
  // Body tables (cost 0) are dropped by monsters, not put on maps.
  const chestTables = allowed.filter((t) => t.cost > 0);
  const bossTables = chestTables.filter((t) => t.levelMax > t.levelBase);
  const spent = spentOf(
    chests.map((c) => c.table),
    bossTable,
    p.tables,
  );
  const budget = budgetOf(p.tier, minPlayers);
  const limit = Math.min(CHESTS_MAX, tierLoot(p.tier)?.chests ?? 0);
  const option = (t: RewardTable) => (
    <option key={t.id} value={t.id}>
      #{t.id} {levelsLabel(t)}, cost {t.cost}
    </option>
  );
  return (
    <fieldset className="forms-field" disabled={p.locked}>
      <legend>Loot</legend>
      <div className="forms-row">
        <label>
          Minimum players{" "}
          <input
            type="number"
            min={1}
            max={MIN_PLAYERS_MAX}
            value={minPlayers}
            onChange={(e) => p.onChange({ ...p.value, minPlayers: Math.max(1, Math.min(MIN_PLAYERS_MAX, Number(e.target.value) || 1)) })}
          />
        </label>{" "}
        <label>
          Boss table{" "}
          <select value={bossTable} onChange={(e) => p.onChange({ ...p.value, bossTable: Number(e.target.value) })}>
            <option value={NO_TABLE}>none</option>
            {bossTables.map(option)}
          </select>
        </label>
      </div>
      <div className="forms-row">
        <label title="No living monster this close to a chest or to any hero when a chest opens">
          Chest radius{" "}
          <input
            type="number"
            min={CHEST_RADIUS_MIN}
            max={CHEST_RADIUS_MAX}
            value={p.value.chestRadius}
            onChange={(e) => p.onChange({ ...p.value, chestRadius: Math.max(CHEST_RADIUS_MIN, Math.min(CHEST_RADIUS_MAX, Number(e.target.value) || CHEST_RADIUS_MIN)) })}
          />
        </label>{" "}
        <label title="After the party, then each player claims their own and gets their contribution back">
          Time to hand out the rewards{" "}
          <select value={p.value.settleTimeoutSecs} onChange={(e) => p.onChange({ ...p.value, settleTimeoutSecs: Number(e.target.value) })}>
            {SETTLE_CHOICES.map((secs) => (
              <option key={secs} value={secs}>
                {settleLabel(secs)}
              </option>
            ))}
          </select>
        </label>
      </div>
      <p className={spent > budget ? "error" : "muted"}>
        Budget: {spent} / {budget} · chests {chests.length}/{limit}
      </p>
      <ul className="forms-spawns">
        {chests.map((c, i) => (
          <li key={`${c.x}-${c.y}`}>
            <span>
              Chest at ({c.x}, {c.y})
            </span>{" "}
            <select
              aria-label={`chest ${i} table`}
              value={c.table}
              onChange={(e) => p.onChange({ ...p.value, chests: chests.map((x, j) => (j === i ? { ...x, table: Number(e.target.value) } : x)) })}
            >
              {chestTables.map(option)}
            </select>{" "}
            <button type="button" className="btn" aria-label={`remove chest ${i}`} onClick={() => p.onChange({ ...p.value, chests: chests.filter((_, j) => j !== i) })}>
              Remove
            </button>
          </li>
        ))}
      </ul>
    </fieldset>
  );
}
