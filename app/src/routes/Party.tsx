import { useMemo, useState } from "react";
import type { Address } from "@solana/kit";
import { useConnectedWallet } from "@solana/kit-plugin-wallet/react";
import { DiceCascade } from "../components/dice/DiceCascade";
import { HitBreakdown } from "../components/dice/HitBreakdown";
import { Grid } from "../components/grid/Grid";
import type { GridUnit } from "../components/grid/types";
import { PartyLobby } from "../components/forms/PartyLobby";
import type { LobbyPlayer } from "../components/forms/types";
import { freezeTombstoneInstruction } from "../chain/character";
import { DEMO_MAP, MOCK_CHARACTERS, MOCK_HITS } from "../chain/mock";
import { bestiaryName } from "../chain/names";
import { useAutoFreezeTombstone } from "../chain/useAutoFreezeTombstone";
import type { TombstoneLog } from "../chain/tombstone";
import { PARTY_MAX, type HitResult } from "../chain/types";
import { DATA_SOURCE } from "../config";
import { useSessionKey } from "../session-key/SessionKeyProvider";
import { client } from "../wallet/client";

const PRICE = 100_000_000n;
const DM = "DMw4Ns7ZqX5o8b2bJm1yQe3kV9tRgHc6uLpAa1dF2sKe";

function initialUnits(): GridUnit[] {
  return [
    { id: 0, kind: "player", x: 1, y: 2, hp: 30, hpMax: 30, alive: true, label: "Bran" },
    { id: 1, kind: "player", x: 2, y: 1, hp: 22, hpMax: 22, alive: true, label: "Aria" },
    ...DEMO_MAP.spawns.map((s, i) => ({
      id: 4 + i,
      kind: "monster" as const,
      x: s.x,
      y: s.y,
      hp: 12,
      hpMax: 12,
      alive: true,
      label: bestiaryName(s.bestiaryId),
    })),
  ];
}

export function PartyPage() {
  const connected = useConnectedWallet(client);
  const sessionKey = useSessionKey();
  const payer = sessionKey.signer ?? connected?.signer ?? null;
  const chainLog = useAutoFreezeTombstone(payer, (connected?.account.address as Address | undefined) ?? null);
  const [mockLog, setMockLog] = useState<TombstoneLog[]>([]);

  const [role, setRole] = useState<"dm" | "player" | "guest">("guest");
  const [players, setPlayers] = useState<LobbyPlayer[]>([]);
  const [started, setStarted] = useState(false);

  const [units, setUnits] = useState(initialUnits);
  const [active, setActive] = useState(1);
  const [target, setTarget] = useState<number | null>(null);
  const [hit, setHit] = useState<{ result: HitResult; attacker: string; target: string; key: number } | null>(null);

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
    setHit({ result, attacker: activeUnit.label, target: t.label, key: Date.now() });
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
            width={16}
            height={16}
            tiles={DEMO_MAP.tiles}
            units={units}
            activeUnitId={active}
            selectedUnitId={target}
            targets={targets}
            cellPx={36}
            onUnitClick={(u) => (u.kind === "monster" ? setTarget(u.id) : setActive(u.id))}
          />
          <div style={{ flex: "1 1 360px", display: "grid", gap: 12 }}>
            <div className="panel">
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
              <>
                <DiceCascade key={hit.key} hit={hit.result} attackerLabel={hit.attacker} targetLabel={hit.target} />
                <HitBreakdown hit={hit.result} defaultOpen={false} />
              </>
            )}
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
