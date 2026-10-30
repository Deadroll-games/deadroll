import { useMemo, useState } from "react";
import type { Address } from "@solana/kit";
import { useConnectedWallet } from "@solana/kit-plugin-wallet/react";
import { HitCard } from "../components/dice/HitCard";
import { Grid } from "../components/grid/Grid";
import { StatsPanel } from "../components/stats/StatsPanel";
import type { StatsUnit } from "../components/stats/types";
import type { GridUnit } from "../components/grid/types";
import { PartyLobby } from "../components/forms/PartyLobby";
import type { LobbyPlayer } from "../components/forms/types";
import { freezeTombstoneInstruction } from "../chain/character";
import { DEMO_MAP, MOCK_CHARACTERS, MOCK_HITS, mockRecord } from "../chain/mock";
import { draftPoints } from "../chain/game";
import { useCombat, type CombatWasm } from "../chain/combatWasm";
import { bestiaryName } from "../chain/names";
import { useAutoFreezeTombstone } from "../chain/useAutoFreezeTombstone";
import type { TombstoneLog } from "../chain/tombstone";
import { monsterDefense, playerDefense, unitStats } from "../chain/stats";
import { Aspect, Class, MAP_MAX, ModKind, PARTY_MAX, Tile, cellByte, type HitResult, type TimedMod } from "../chain/types";
import { DATA_SOURCE } from "../config";
import { client } from "../wallet/client";
import { PartyView } from "./PartyChain";

const PRICE = 100_000_000n;
const DM = "DMw4Ns7ZqX5o8b2bJm1yQe3kV9tRgHc6uLpAa1dF2sKe";

/** Arena update: the mock party in pack `pack`'s arena (players at its north-west corner, the
 * pack where the DM put it), or on the map with each pack's visible monster (`pack` null). The
 * monsters keep their spawn index in the id (4 + spawn). */
function initialUnits(pack: number | null = null): GridUnit[] {
  const players: GridUnit[] = [
    { id: 0, kind: "player", x: pack === null ? 1 : 0, y: pack === null ? 2 : 0, hp: 30, hpMax: 30, alive: true, label: "Bran" },
    { id: 1, kind: "player", x: pack === null ? 2 : 1, y: pack === null ? 1 : 0, hp: 22, hpMax: 22, alive: true, label: "Aria" },
  ];
  const monster = (i: number, x: number, y: number): GridUnit => {
    const sp = DEMO_MAP.spawns[i]!;
    return { id: 4 + i, kind: "monster", x, y, hp: 12, hpMax: 12, alive: true, label: bestiaryName(sp.bestiaryId) };
  };
  if (pack === null) {
    return [...players, ...DEMO_MAP.packs.flatMap((pk) => (pk ? [monster(pk.visible, pk.x, pk.y)] : []))];
  }
  return [...players, ...DEMO_MAP.spawns.flatMap((sp, i) => (sp.pack === pack ? [monster(i, sp.x, sp.y)] : []))];
}

/** The cells of pack `pack`'s arena at the map's stride (32), the rest walls. */
function arenaTiles(pack: number): Uint8Array {
  const pk = DEMO_MAP.packs[pack]!;
  const out = new Uint8Array(MAP_MAX * MAP_MAX).fill(cellByte(Tile.Wall));
  for (let y = 0; y < pk.height; y++) for (let x = 0; x < pk.width; x++) out[y * MAP_MAX + x] = pk.cells[y * pk.width + x]!;
  return out;
}

/** Mock buffs and debuffs: Bran empowered, Aria hasted, the first wolf exposed to Fire. */
const MOCK_MODS: Record<number, TimedMod[]> = {
  0: [{ kind: ModKind.Empowerment, aspect: Aspect.Blunt, turns: 2, powerBp: 3_000, source: 1 }],
  1: [{ kind: ModKind.Haste, aspect: Aspect.Lightning, turns: 3, powerBp: 3_400, source: 1 }],
  4: [{ kind: ModKind.Vulnerability, aspect: Aspect.Fire, turns: 2, powerBp: 2_500, source: 1 }],
};

function mockStats(units: GridUnit[], active: number, combat: CombatWasm | null): StatsUnit[] {
  return units
    .filter((u) => u.alive)
    .flatMap((u): StatsUnit[] => {
      const profile =
        u.kind === "player"
          ? playerDefense(MOCK_CHARACTERS[u.id]?.class ?? Class.Warrior, [10, 7, 5, 8])
          : (() => {
              const spawn = DEMO_MAP.spawns[u.id - 4];
              return spawn && combat ? monsterDefense(combat, mockRecord(spawn.bestiaryId), spawn.level, spawn.flags) : null;
            })();
      if (!profile) return [];
      const apBase = u.kind === "player" ? 4 : 6;
      const stats = unitStats({ hp: u.hp, hpMax: u.hpMax, apBase, apNow: u.id === active ? apBase : null, profile, mods: MOCK_MODS[u.id] ?? [] });
      return [{ unit: u.id, label: u.label, kind: u.kind, current: u.id === active, stats }];
    });
}

/** The dungeon screen of one party; mock data without a validator. */
export function PartyScreen({ address }: { address: Address | null }) {
  return DATA_SOURCE === "chain" && address ? <PartyView address={address} /> : <MockParty />;
}

/** Mock mode (VITE_DATA=mock): the lobby and battle on synthetic data. */
function MockParty() {
  const connected = useConnectedWallet(client);
  const payer = connected?.signer ?? null;
  const chainLog = useAutoFreezeTombstone(payer, (connected?.account.address as Address | undefined) ?? null);
  const [mockLog, setMockLog] = useState<TombstoneLog[]>([]);
  const combat = useCombat();

  const [role, setRole] = useState<"dm" | "player" | "guest">("guest");
  const [players, setPlayers] = useState<LobbyPlayer[]>([]);
  const [started, setStarted] = useState(false);

  const [units, setUnits] = useState(() => initialUnits());
  // Arena update: the map, or the arena of a pack (mock: entered with a button).
  const [arena, setArena] = useState<number | null>(null);
  const goTo = (pack: number | null) => {
    setArena(pack);
    setUnits(initialUnits(pack));
    setTarget(null);
  };
  const [active, setActive] = useState(1);
  const [target, setTarget] = useState<number | null>(null);
  const [hit, setHit] = useState<{ result: HitResult; attacker: string; target: string; key: number; attackerId: number; targetId: number } | null>(null);

  const joined = players.map((p) => p.slot);
  const myChars = MOCK_CHARACTERS.filter((_, i) => !joined.includes(i)).map((c) => ({ address: c.address, name: c.name }));

  const join = (address: string) => {
    const i = MOCK_CHARACTERS.findIndex((c) => c.address === address);
    const c = MOCK_CHARACTERS[i];
    if (!c || players.length >= PARTY_MAX) return;
    setPlayers((ps) => [
      ...ps,
      { slot: i, player: connected?.account.address ?? `Player${i}11111111111111111111111111111111`, characterName: c.name, race: c.race, class: c.class, ready: false, paid: PRICE },
    ]);
    setRole("player");
  };

  const activeUnit = units.find((u) => u.id === active);
  const targets = useMemo(
    () =>
      activeUnit?.kind === "player"
        ? units.filter((u) => u.kind === "monster" && u.alive && Math.abs(u.x - activeUnit.x) + Math.abs(u.y - activeUnit.y) <= 6).map(({ x, y }) => ({ x, y }))
        : [],
    [units, activeUnit],
  );

  const attack = (kind: keyof typeof MOCK_HITS) => {
    const t = units.find((u) => u.id === target);
    if (!activeUnit || !t) return;
    const result = MOCK_HITS[kind];
    setHit({ result, attacker: activeUnit.label, target: t.label, key: Date.now(), attackerId: activeUnit.id, targetId: t.id });
    setUnits((us) => us.map((u) => (u.id === t.id ? { ...u, hp: Math.max(0, u.hp - result.damage), alive: u.hp - result.damage > 0 } : u)));
  };

  // Mock death of the active player: the chain emits CharacterDied from mark_dead; we show the
  // freeze_tombstone instruction the app would send.
  const killActive = () => {
    if (!activeUnit || activeUnit.kind !== "player") return;
    setUnits((us) => us.map((u) => (u.id === activeUnit.id ? { ...u, hp: 0, alive: false } : u)));
    const c = MOCK_CHARACTERS[activeUnit.id];
    if (!c || !payer) {
      setMockLog((l) => [{ character: (c?.address ?? "") as Address, status: "skipped", detail: "start a session key to sign freeze_tombstone" }, ...l]);
      return;
    }
    const ix = freezeTombstoneInstruction({ character: c.address as Address, asset: c.address as Address, payer });
    setMockLog((l) => [
      { character: c.address as Address, status: "seen", detail: `CharacterDied -> freeze_tombstone (${ix.accounts?.length} accounts, payer ${payer.address.slice(0, 4)}..)` },
      ...l,
    ]);
  };

  const log = DATA_SOURCE === "chain" ? chainLog : mockLog;

  return (
    <section>
      <h1>Party</h1>
      {!started ? (
        <>
          <p className="muted">
            Viewing as{" "}
            {(["guest", "player", "dm"] as const).map((r) => (
              <button key={r} className="btn" aria-pressed={role === r} onClick={() => setRole(r)}>
                {r}
              </button>
            ))}
          </p>
          <PartyLobby
            mapName="demo_16x16"
            dm={DM}
            priceLamports={PRICE}
            collected={players.reduce((n, p) => n + p.paid, 0n)}
            players={players}
            maxPlayers={PARTY_MAX}
            role={role}
            mySlot={players[players.length - 1]?.slot ?? null}
            myCharacters={myChars}
            onJoin={join}
            onReady={(ready) => setPlayers((ps) => ps.map((p, i) => (i === ps.length - 1 ? { ...p, ready } : p)))}
            onStart={() => setStarted(true)}
            mapPreview={<Grid width={16} height={16} tiles={DEMO_MAP.tiles} units={[]} cellPx={12} />}
          />
          <p className="muted">Mock lobby: join with both characters (guest), ready each (player), then start (dm).</p>
        </>
      ) : (
        <div style={{ display: "flex", gap: 20, alignItems: "flex-start", flexWrap: "wrap" }}>
          <Grid
            width={arena === null ? 16 : DEMO_MAP.packs[arena]!.width}
            height={arena === null ? 16 : DEMO_MAP.packs[arena]!.height}
            tiles={arena === null ? DEMO_MAP.tiles : arenaTiles(arena)}
            points={arena === null ? draftPoints(DEMO_MAP) : []}
            units={units}
            activeUnitId={active}
            selectedUnitId={target}
            targets={targets}
            cellPx={36}
            onUnitClick={(u) => (u.kind === "monster" ? setTarget(u.id) : setActive(u.id))}
          />
          <div style={{ flex: "1 1 360px", display: "grid", gridTemplateColumns: "minmax(0, 1fr)", gap: 12, minWidth: 0 }}>
            <div className="panel">
              <p>
                {arena === null ? "Map: step next to a pack's monster to fight it." : `Arena: pack ${arena}.`}{" "}
                {arena === null
                  ? DEMO_MAP.packs.flatMap((pk, n) =>
                      pk ? [
                        <button key={n} type="button" className="btn" onClick={() => goTo(n)}>
                          Arena of pack {n}
                        </button>,
                      ] : [],
                    )
                  : (
                    <button type="button" className="btn" onClick={() => goTo(null)}>
                      Win: back to the map
                    </button>
                  )}
              </p>
              <p>
                Active: <b>{activeUnit?.label}</b> · Target: <b>{units.find((u) => u.id === target)?.label ?? "click a monster"}</b>
              </p>
              <button className="btn" disabled={target === null} onClick={() => attack("resonance")}>
                Cast (resonance)
              </button>{" "}
              <button className="btn" disabled={target === null} onClick={() => attack("crit")}>
                Strike (crit)
              </button>{" "}
              <button className="btn" disabled={target === null} onClick={() => attack("miss")}>
                Attack (miss)
              </button>{" "}
              <button className="btn" onClick={killActive}>
                Kill active player
              </button>
            </div>
            {hit && (
              <HitCard
                key={hit.key}
                rollKey={String(hit.key)}
                header={`${hit.attacker} → ${hit.target}`}
                hit={hit.result}
                breakdown={{ mods: { attacker: MOCK_MODS[hit.attackerId] ?? [], target: MOCK_MODS[hit.targetId] ?? [] } }}
              />
            )}
            <StatsPanel units={mockStats(units, active, combat)} />
            {log.length > 0 && (
              <div className="panel">
                <h3>Tombstones</h3>
                <ul className="mono">
                  {log.map((e, i) => (
                    <li key={i}>
                      {e.status} {e.character.slice(0, 6)}.. {e.detail}
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
