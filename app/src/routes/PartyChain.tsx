// Party screen on chain (L9.6): open a party on a finalized map, pay and join, session keys,
// ready, start, initiative, moves, attacks with the dice cascade from HitResolved, monster turns
// by the DM, death + freeze_tombstone, victory and claims. The calls mirror tests/demo.test.ts.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import type { Address, TransactionSigner } from "@solana/kit";
import { DiceCascade } from "../components/dice/DiceCascade";
import { EffectCascade } from "../components/dice/EffectCascade";
import { HitBreakdown } from "../components/dice/HitBreakdown";
import type { HitMods } from "../components/dice/types";
import { StatsPanel } from "../components/stats/StatsPanel";
import type { StatsUnit } from "../components/stats/types";
import { Grid } from "../components/grid/Grid";
import type { GridUnit } from "../components/grid/types";
import { PartyLobby } from "../components/forms/PartyLobby";
import type { LobbyPlayer } from "../components/forms/types";
import { monsterRequest, renderRequest, tileRequest, TOMBSTONE } from "../chain/appearance";
import {
  BASE_ATTACK,
  CLASS_KEYS,
  MONSTER_BASE,
  RACE_KEYS,
  actIx,
  bytesText,
  claimEscrowIx,
  claimRewardsIx,
  consumeIx,
  currentUnit,
  endTurnIx,
  enumIndex,
  getAccount,
  joinSessionIx,
  mapStatus,
  mapsOf,
  monsterActIx,
  monsterMoveIx,
  moveIx,
  myCharacters,
  openSessionIx,
  openSessions,
  pendingTargetOf,
  pickSeed,
  playerAp,
  readParty,
  sessionAccounts,
  setReadyIx,
  startSessionIx,
  startTurnIx,
  status,
  type Actor,
  type CharacterAcc,
  type SkillSetAcc,
  type GameSessionAcc,
  type MapAcc,
  type Party,
  type SeedKind,
} from "../chain/game";
import { toHitResolved } from "../chain/hit";
import { bestiaryName, playerCritChanceBp } from "../chain/names";
import { errorName } from "../chain/send";
import { statusLabel, type SubStatus } from "../chain/subscribe";
import { cooldownLabel, skillCooldowns, skillUsedText, skipText, startTurnFor, tickText, turnActed, turnStarted } from "../chain/turns";
import { skillSetPda } from "../chain/pda";
import { affordable, cheb, reachable, route, type Step } from "../chain/route";
import { useAutoFreezeTombstone } from "../chain/useAutoFreezeTombstone";
import { useParty, type PartyEvent } from "../chain/useParty";
import { Purpose, inReach } from "../chain/skillcheck";
import { MONSTER_DEFENSE, monsterBaseAp, playerDefense, unitStats } from "../chain/stats";
import { activeMods, combatSkill, effectText, modsAt, modsTickText, toEffectResolved, type CombatSkill, type EffectResolved } from "../chain/support";
import { useTx } from "../chain/useTx";
import { MAP_MAX, PARTY_MAX, type Cell, type Class, type HitResult, type Race, type Tile } from "../chain/types";
import { MOCK_VRF, explorerUrl } from "../config";
import { useSprites } from "../raster/useSprites";
import { useSessionKey } from "../session-key/SessionKeyProvider";
import { useAppWallet } from "../wallet/AppWallet";
import { formatSol, shortAddress } from "../wallet/ConnectButton";

const CELL_PX = 32;

export function PartyChain() {
  const [params] = useSearchParams();
  const s = params.get("s") as Address | null;
  return s ? <PartyView address={s} /> : <PartyPicker map={params.get("map") as Address | null} />;
}

// ---------------------------------------------------------------------------------------------
// no party selected: open one (DM) or pick one (players)

function PartyPicker({ map }: { map: Address | null }) {
  const wallet = useAppWallet();
  const sk = useSessionKey();
  const { send, busy } = useTx();
  const [, setParams] = useSearchParams();
  const [price, setPrice] = useState("0.1");
  const [mapAcc, setMapAcc] = useState<MapAcc | null | "missing">(null);
  const [sessions, setSessions] = useState<{ address: Address; data: GameSessionAcc }[]>([]);
  const [myMaps, setMyMaps] = useState<{ address: Address; data: MapAcc }[]>([]);

  const refresh = useCallback(async () => {
    try {
      setSessions(await openSessions());
      if (wallet.address) setMyMaps((await mapsOf(wallet.address)).filter((m) => mapStatus(m.data) === "finalized"));
      if (map) setMapAcc((await getAccount<MapAcc>("map", "Map", map)) ?? "missing");
    } catch {
      // lists stay as they are
    }
  }, [map, wallet.address]);
  useEffect(() => void refresh(), [refresh]);

  const open = async () => {
    if (!map || !wallet.signer) return void (await send("open_session", wallet.signer, []));
    // The DM's session key moves monsters and ends their turns without popups.
    const dmKey = sk.signer ?? (await sk.start());
    if (!dmKey) return;
    const lamports = BigInt(Math.round(Number(price) * 1e9));
    const res = await send("open_session", wallet.signer, async () => [await openSessionIx(wallet.signer!, map, lamports, dmKey.address)]);
    if (res) setParams({ s: (await sessionAccounts(map)).session });
  };

  const used = mapAcc && mapAcc !== "missing" ? mapStatus(mapAcc) : null;
  return (
    <section>
      <h1>Party</h1>
      {map && (
        <div className="panel">
          <h3>Open a party (DM)</h3>
          <p className="mono">
            Map{" "}
            <a href={explorerUrl("address", map)} target="_blank" rel="noreferrer">
              {shortAddress(map)}
            </a>{" "}
            {mapAcc === "missing" ? "not found" : used ? `· ${used}` : "..."}
          </p>
          {used && used !== "finalized" && (
            <p className="error">
              This map is already {used}. Build a fresh one in the <Link to="/map">Map builder</Link>. (For the deployed demo map: rerun{" "}
              <code>deploy-local --map-index N</code>.)
            </p>
          )}
          {mapAcc && mapAcc !== "missing" && wallet.address && mapAcc.core.dm !== wallet.address && (
            <p className="error">Only the map's DM ({shortAddress(mapAcc.core.dm)}) can open a party on it.</p>
          )}
          <div className="row">
            <label>
              Price (SOL){" "}
              <input value={price} onChange={(e) => setPrice(e.target.value)} size={6} inputMode="decimal" />
            </label>
            <button className="btn primary" disabled={busy !== null || used !== "finalized"} onClick={() => void open()}>
              {busy ?? "Open party"}
            </button>
          </div>
          <p className="muted">Starts your session key if needed: it moves the monsters and ends their turns.</p>
        </div>
      )}
      <div className="panel">
        <div className="row">
          <h3 style={{ margin: 0 }}>Parties</h3>
          <button className="btn" onClick={() => void refresh()}>
            Refresh
          </button>
        </div>
        {sessions.length === 0 && <p className="muted">No open parties. A DM opens one from a finalized map.</p>}
        <ul className="mono">
          {sessions.map((x) => (
            <li key={x.address}>
              <Link to={`/party?s=${x.address}`}>{shortAddress(x.address)}</Link> · {status(x.data)} · DM {shortAddress(x.data.core.dm)} · {x.data.core.playerCount}/
              {PARTY_MAX} players · {formatSol(x.data.core.priceLamports)}
            </li>
          ))}
        </ul>
      </div>
      {myMaps.length > 0 && (
        <div className="panel">
          <h3>My finalized maps</h3>
          <ul className="mono">
            {myMaps.map((m) => (
              <li key={m.address}>
                {shortAddress(m.address)} {m.data.core.width}x{m.data.core.height} · <Link to={`/party?map=${m.address}`}>open a party</Link>
              </li>
            ))}
          </ul>
        </div>
      )}
      {!map && myMaps.length === 0 && (
        <p className="muted">
          DM: build and finalize a map in the <Link to="/map">Map builder</Link> first.
        </p>
      )}
    </section>
  );
}

// ---------------------------------------------------------------------------------------------
// one party

type Hit = { result: HitResult; attacker: string; target: string; critChanceBp?: number; mods: HitMods };
type EffectView = { effect: EffectResolved; caster: string; target: string; skill: string; critChanceBp?: number };
/** The dice panel: a hit, a heal / buff / debuff, or a hit followed by its skill's effect. */
type Roll = { key: string; hit: Hit | null; effect: EffectView | null };

function PartyView({ address }: { address: Address }) {
  const wallet = useAppWallet();
  const sk = useSessionKey();
  const { send, busy } = useTx();
  const { party, error, events, ingest, refresh, live } = useParty(address);
  const draw = useSprites();
  const [seedKind, setSeedKind] = useState<SeedKind>("random");
  const [skill, setSkill] = useState<number>(0);
  const [hover, setHover] = useState<Cell | null>(null);
  const [autoMonsters, setAutoMonsters] = useState(false);
  const [roll, setRoll] = useState<Roll | null>(null);
  const [myChars, setMyChars] = useState<{ address: Address; data: CharacterAcc }[]>([]);
  const [skillNames, setSkillNames] = useState<Record<string, CombatSkill[]>>({});
  const lock = useRef(false);

  const me = wallet.address;
  const s = party?.session ?? null;
  const st = s ? status(s) : null;
  const isDm = !!s && s.core.dm === me;
  const mine = party?.players.filter((p) => p.part.player === me) ?? [];
  const unit = s ? currentUnit(s) : null;
  const activePlayer = unit !== null && unit < MONSTER_BASE ? party?.players.find((p) => p.slot === unit) ?? null : null;
  const myTurn = !!activePlayer && activePlayer.part.player === me;
  const monsterTurn = unit !== null && unit >= MONSTER_BASE;

  const payer: TransactionSigner | null = sk.signer ?? wallet.signer;
  const tombLog = useAutoFreezeTombstone(payer, me);

  const actorFor = useCallback(
    (actor: Address): Actor | null => {
      if (sk.signer && sk.token && sk.signer.address === actor) return { signer: sk.signer, token: sk.token };
      return wallet.signer ? { signer: wallet.signer, token: null } : null;
    },
    [sk.signer, sk.token, wallet.signer],
  );

  const label = useCallback(
    (u: number): string => {
      if (u >= MONSTER_BASE + 24) return "initiative";
      if (!party) return `#${u}`;
      if (u < MONSTER_BASE) return party.players.find((p) => p.slot === u) ? bytesText(party.players.find((p) => p.slot === u)!.char.name) : `slot ${u}`;
      const m = party.monsters[u - MONSTER_BASE];
      return m ? `${bestiaryName(m.bestiaryId)} ${u - MONSTER_BASE}` : `monster ${u - MONSTER_BASE}`;
    },
    [party],
  );

  const skillName = useCallback(
    (u: number, slot: number): string => {
      const pl = party?.players.find((p) => p.slot === u);
      return (pl && skillNames[pl.character]?.find((k) => k.slot === slot)?.name) || `skill ${slot}`;
    },
    [party, skillNames],
  );

  // My characters (lobby join list) and skill names of party characters (attack buttons).
  useEffect(() => {
    if (!me) return;
    void myCharacters(me).then(setMyChars, () => setMyChars([]));
  }, [me, st, party?.session.core.playerCount]);
  const partyChars = party?.players.map((p) => p.character).join(",") ?? "";
  useEffect(() => {
    if (!party) return;
    void Promise.all(
      party.players.map(async (p) => {
        const set = await getAccount<SkillSetAcc>(
          "character",
          "SkillSet",
          await skillSetPda(p.character),
        );
        const list = (set?.skills ?? []).slice(0, set?.count ?? 0).map((k, slot) => combatSkill(slot, k));
        return [p.character, list] as const;
      }),
    ).then((entries) => setSkillNames(Object.fromEntries(entries)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [partyChars]);

  // The dice panel follows the newest HitResolved or EffectResolved of this party (any window's
  // action). A damage skill's effect (rider) comes after its hit on the same action: both show.
  const lastRollKey = useRef<string | null>(null);
  useEffect(() => {
    const lastOf = (pred: (e: PartyEvent) => boolean, before = events.length) => {
      for (let j = before - 1; j >= 0; j--) if (pred(events[j]!)) return j;
      return -1;
    };
    const i = lastOf((e) => e.name === "HitResolved" || e.name === "EffectResolved");
    const ev = events[i];
    if (!ev) return;
    const key = `${ev.signature}:${i}`;
    if (key === lastRollKey.current) return;
    lastRollKey.current = key;
    // Crit chance is not in the events; players derive it from Dexterity, monsters never roll crit.
    const critOf = (u: number) => {
      const dex = u < MONSTER_BASE ? party?.players.find((p) => p.slot === u)?.char.core.stats[1] : undefined;
      return dex == null ? undefined : playerCritChanceBp(dex);
    };
    // Mods are not in HitResolved: take each side's from the events before the hit, else the accounts.
    const modsNow = (u: number) =>
      !party ? [] : u < MONSTER_BASE ? activeMods(party.players.find((p) => p.slot === u)?.part.mods) : activeMods(party.monsterMods[u - MONSTER_BASE]);
    const hitView = (data: unknown, at: number): Hit => {
      const h = toHitResolved(data);
      const mods = { attacker: modsAt(events, at, h.attacker, modsNow(h.attacker)), target: modsAt(events, at, h.target, modsNow(h.target)) };
      return { result: h.result, attacker: label(h.attacker), target: label(h.target), critChanceBp: critOf(h.attacker), mods };
    };
    if (ev.name === "HitResolved") return setRoll({ key, hit: hitView(ev.data, i), effect: null });
    const e = toEffectResolved(ev.data);
    const effect: EffectView = { effect: e, caster: label(e.caster), target: label(e.target), skill: skillName(e.caster, e.skillSlot), critChanceBp: critOf(e.caster) };
    const hitAt = e.rider ? lastOf((x) => x.name === "HitResolved" && x.data.actionCounter === e.actionCounter, i) : -1;
    const hitEv = events[hitAt];
    setRoll({ key, hit: hitEv ? hitView(hitEv.data, hitAt) : null, effect });
  }, [events, label, party, skillName]);

  // ---- the localnet VRF oracle (mock-vrf build): answers the pending roll of this party.
  const consume = useCallback(
    async (kind: SeedKind = seedKind, quietFail = false) => {
      const p = await readParty(address, party?.tiles ?? null);
      if (!p?.session.core.pendingVrf) return;
      const pend = p.session.pending.participation;
      const pl = p.players.find((x) => x.participation === pend);
      if (!pl) return;
      const seed = await pickSeed(kind);
      const res = await send("VRF roll (localnet oracle)", sk.signer ?? wallet.signer, async () => [await consumeIx(address, pend, pl.character, seed, pendingTargetOf(p.session))], {
        quiet: true,
        silent: quietFail,
      });
      if (res) ingest(res.signature, res.events);
    },
    [address, party?.tiles, seedKind, send, sk.signer, wallet.signer, ingest],
  );

  // DM window fallback: a roll still pending after 3.5 s (the acting window closed) gets answered.
  const pendingSince = useRef<{ counter: number; at: number } | null>(null);
  useEffect(() => {
    if (!MOCK_VRF || !isDm || !s?.core.pendingVrf) {
      pendingSince.current = null;
      return;
    }
    const counter = s.core.actionCounter;
    if (pendingSince.current?.counter !== counter) {
      pendingSince.current = { counter, at: Date.now() };
      return;
    }
    if (Date.now() - pendingSince.current.at > 3500 && !lock.current) {
      lock.current = true;
      void consume(seedKind, true).finally(() => {
        lock.current = false;
        pendingSince.current = null;
      });
    }
  }, [s, isDm, consume, seedKind]);

  // ---- start_turn: every turn begins with it. This tab sends it for its own player, the DM tab
  // for monsters. A Freeze / Stun skip or a death from over-time damage ends the turn inside it,
  // so the loop goes on while the next unit is this tab's too; other tabs start theirs.
  const startRun = useRef<Promise<Party | null> | null>(null);
  const startFailed = useRef<{ seq: number; at: number } | null>(null);
  const ensureStarted = useCallback((): Promise<Party | null> => {
    startRun.current ??= (async () => {
      try {
        let p = await readParty(address, party?.tiles ?? null);
        for (let i = 0; p && i < 2 * PARTY_MAX + 24; i++) {
          const next = startTurnFor(p, me ?? null);
          if (!next) break;
          // A failed start (no SOL, RPC down) is retried after 3 s, by the poll or the next action.
          if (startFailed.current?.seq === next.turnSeq && Date.now() - startFailed.current.at < 3_000) break;
          const actor = actorFor(next.player ? next.player.part.core.actor : p.session.dmActor);
          if (!actor) break;
          const player = next.player ? { participation: next.player.participation, character: next.player.character } : null;
          let failed = false;
          const res = await send(`start turn ${label(next.unit)}`, actor.signer, async () => [await startTurnIx(address, player)], {
            quiet: true,
            onError: (m) => {
              // Another tab started it first: nothing to show.
              if (errorName(m) === "TurnAlreadyStarted") return true;
              failed = true;
              return false;
            },
          });
          if (res) ingest(res.signature, res.events);
          if (failed) {
            startFailed.current = { seq: next.turnSeq, at: Date.now() };
            break;
          }
          p = await readParty(address, p.tiles);
        }
        if (p) await refresh();
        return p;
      } finally {
        startRun.current = null;
      }
    })();
    return startRun.current;
  }, [address, party?.tiles, me, actorFor, send, ingest, refresh, label]);

  useEffect(() => {
    if (party && startTurnFor(party, me ?? null)) void ensureStarted();
  }, [party, me, ensureStarted]);

  /** Sends one game action, then (localnet) answers its VRF request at once. */
  const act = async (what: string, signer: TransactionSigner | null, ixs: () => Promise<Parameters<typeof send>[2]>, needsRoll: boolean) => {
    if (lock.current) return;
    lock.current = true;
    try {
      // A stale read can let an action through before start_turn: start it and retry, never show it.
      let notStarted = false;
      const opts = { quiet: true, onError: (m: string) => (notStarted = errorName(m) === "TurnNotStarted") };
      let res = await send(what, signer, async () => (await ixs()) as never, opts);
      if (!res && notStarted) {
        await ensureStarted();
        res = await send(what, signer, async () => (await ixs()) as never, opts);
      }
      if (!res) {
        await refresh();
        return;
      }
      ingest(res.signature, res.events);
      if (needsRoll && MOCK_VRF) await consume();
      await refresh();
    } finally {
      lock.current = false;
    }
  };

  // ---- derived grid data
  const tileCost = useMemo(() => (s ? Array.from(s.tileCost) : []), [s]);
  const taken = useCallback(
    (p: Party, mover: number) => {
      const t = new Set<number>();
      p.monsters.forEach((m, i) => m.alive && MONSTER_BASE + i !== mover && t.add(m.y * MAP_MAX + m.x));
      p.players.forEach((pl) => p.session.aliveMask & (1 << pl.slot) && pl.slot !== mover && t.add(pl.part.core.y * MAP_MAX + pl.part.core.x));
      return t;
    },
    [],
  );

  const units: GridUnit[] = useMemo(() => {
    if (!party) return [];
    return [
      ...party.players.map((p) => ({
        id: p.slot,
        kind: "player" as const,
        x: p.part.core.x,
        y: p.part.core.y,
        hp: p.part.core.hp,
        // Participation.hpMax since #25 (heals stop there); 0 on a party joined before it.
        hpMax: p.part.hpMax || p.char.core.hpMax,
        alive: (party.session.aliveMask & (1 << p.slot)) !== 0,
        label: bytesText(p.char.name),
      })),
      ...party.monsters.map((m, i) => ({
        id: MONSTER_BASE + i,
        kind: "monster" as const,
        x: m.x,
        y: m.y,
        hp: m.hp,
        hpMax: m.hpMax,
        alive: m.alive,
        label: `${bestiaryName(m.bestiaryId)} ${i}`,
        size: (m.bestiaryId === 2 ? 2 : 1) as 1 | 2,
      })),
    ];
  }, [party]);

  const moverPos: Cell | null = useMemo(() => {
    if (!party || unit === null) return null;
    if (myTurn && activePlayer) return { x: activePlayer.part.core.x, y: activePlayer.part.core.y };
    if (monsterTurn && isDm) {
      const m = party.monsters[unit - MONSTER_BASE];
      return m ? { x: m.x, y: m.y } : null;
    }
    return null;
  }, [party, unit, myTurn, activePlayer, monsterTurn, isDm]);
  const moverAp = !party || !s || unit === null ? 0 : myTurn && activePlayer ? playerAp(activePlayer.part, s) : monsterTurn ? (party.monsters[unit - MONSTER_BASE]?.actionPoints ?? 0) : 0;
  // This tab moves the current unit; nothing goes out before its start_turn (sent on its own above).
  const controls = myTurn || (monsterTurn && isDm);
  const started = !!s && turnStarted(s);
  const canMove = controls && started && !s?.core.pendingVrf;
  // One attack or active skill per turn; movement spends the AP left before or after it.
  const acted = !!s && turnActed(s);
  const canAttack = canMove && !acted;
  // Active skills of the current player; one on cooldown cannot be picked, the weapon never is.
  const activeSkills = activePlayer ? (skillNames[activePlayer.character] ?? []).filter((k) => k.usable) : [];
  const cooldowns = activePlayer && s ? skillCooldowns(activePlayer.part, events, activePlayer.slot, s.turnSeq) : [];
  const chosen = activeSkills.find((k) => k.slot === skill && !cooldowns[k.slot]) ?? null;
  const chosenSkill = chosen ? chosen.slot : BASE_ATTACK;
  // Heals and buffs go to an ally (self included), everything else to a monster.
  const chosenSide = chosen?.side ?? "enemy";

  const pathTo = useCallback(
    (to: Cell): Step[] | null => {
      if (!party?.tiles || !s || !moverPos || unit === null) return null;
      const steps = route({ from: moverPos, goal: (x, y) => x === to.x && y === to.y, tiles: party.tiles, width: s.width, height: s.height, tileCost, taken: taken(party, unit) });
      return steps ? affordable(steps, moverAp) : null;
    },
    [party, s, moverPos, unit, tileCost, taken, moverAp],
  );
  const reach = useMemo(
    () => (party?.tiles && s && moverPos && unit !== null && canMove ? reachable({ from: moverPos, ap: moverAp, tiles: party.tiles, width: s.width, height: s.height, tileCost, taken: taken(party, unit) }) : []),
    [party, s, moverPos, unit, moverAp, tileCost, taken, canMove],
  );
  const preview = hover && moverPos ? (pathTo(hover)?.map((st) => st.to) ?? []) : [];

  // ---- actions
  const movePlayer = (to: Cell) => {
    if (!activePlayer || !s) return;
    const steps = pathTo(to);
    if (!steps?.length) return;
    const actor = actorFor(activePlayer.part.core.actor);
    void act(`move ${label(activePlayer.slot)}`, actor?.signer ?? null, async () => [await moveIx(actor!, address, activePlayer.character, s.layout, steps)], false);
  };
  const attack = (monster: number) => {
    if (!activePlayer || !canAttack || chosenSide !== "enemy") return;
    const actor = actorFor(activePlayer.part.core.actor);
    void act(`attack ${label(MONSTER_BASE + monster)}`, actor?.signer ?? null, async () => [await actIx(actor!, address, activePlayer.character, chosenSkill, monster, null)], true);
  };
  /** A heal or buff on an ally: the target is its player slot, its Participation unless a self-cast. */
  const support = (slot: number) => {
    const ally = party?.players.find((p) => p.slot === slot);
    if (!activePlayer || !canAttack || chosenSide !== "ally" || !ally || !chosen) return;
    const actor = actorFor(activePlayer.part.core.actor);
    const allyAccount = slot === activePlayer.slot ? null : ally.participation;
    void act(`${chosen.name} on ${label(slot)}`, actor?.signer ?? null, async () => [await actIx(actor!, address, activePlayer.character, chosenSkill, slot, allyAccount)], true);
  };
  const endTurn = () => {
    if (!s) return;
    if (activePlayer) {
      const actor = actorFor(activePlayer.part.core.actor);
      void act("end turn", actor?.signer ?? null, async () => [await endTurnIx(actor!, address, activePlayer.character)], false);
    } else {
      const actor = actorFor(s.dmActor);
      void act("end monster turn", actor?.signer ?? null, async () => [await endTurnIx(actor!, address, null)], false);
    }
  };
  const moveMonster = (to: Cell) => {
    if (!s || unit === null) return;
    const steps = pathTo(to);
    if (!steps?.length) return;
    const actor = actorFor(s.dmActor);
    void act(`move ${label(unit)}`, actor?.signer ?? null, async () => [await monsterMoveIx(actor!, address, s.layout, unit - MONSTER_BASE, steps)], false);
  };
  const monsterAttack = (slot: number) => {
    const target = party?.players.find((p) => p.slot === slot);
    if (!s || unit === null || !target || !canAttack) return;
    const actor = actorFor(s.dmActor);
    void act(`${label(unit)} attacks ${label(slot)}`, actor?.signer ?? null, async () => [await monsterActIx(actor!, address, unit - MONSTER_BASE, slot, target.character)], true);
  };

  /** The DM's monster: walk toward the nearest living player, bite if adjacent, end the turn (as demo.test.ts). */
  const autoMonsterTurn = useCallback(async () => {
    if (lock.current) return;
    lock.current = true;
    try {
      // start_turn first: a Freeze / Stun skip or a death from over-time damage ends the turn there.
      let p = await ensureStarted();
      if (!p?.tiles || !turnStarted(p.session)) return;
      const u = currentUnit(p.session);
      if (u === null || u < MONSTER_BASE || p.session.core.pendingVrf) return;
      const seq = p.session.turnSeq;
      const mi = u - MONSTER_BASE;
      const m = p.monsters[mi];
      const actor = actorFor(p.session.dmActor);
      if (!actor) return;
      const living = p.players.filter((pl) => p!.session.aliveMask & (1 << pl.slot));
      if (m?.alive && living.length) {
        const target = living.reduce((a, b) => (cheb(m, { x: b.part.core.x, y: b.part.core.y }) < cheb(m, { x: a.part.core.x, y: a.part.core.y }) ? b : a));
        const tpos = { x: target.part.core.x, y: target.part.core.y };
        let ap = m.actionPoints;
        if (cheb(m, tpos) > 1) {
          const steps = route({ from: m, goal: (x, y) => cheb({ x, y }, tpos) === 1, tiles: p.tiles, width: p.session.width, height: p.session.height, tileCost: Array.from(p.session.tileCost), taken: taken(p, u) });
          const take = steps ? affordable(steps, ap, 2) : [];
          if (take.length) {
            const r = await send(`move ${label(u)}`, actor.signer, async () => [await monsterMoveIx(actor, address, p!.session.layout, mi, take)], { quiet: true });
            if (r) ingest(r.signature, r.events);
            ap -= take.reduce((n, x) => n + x.cost, 0);
          }
        }
        p = (await readParty(address, p.tiles))!;
        const mm = p.monsters[mi]!;
        if (ap >= 2 && mm.alive && cheb(mm, tpos) === 1 && !turnActed(p.session)) {
          const r = await send(`${label(u)} attacks ${label(target.slot)}`, actor.signer, async () => [await monsterActIx(actor, address, mi, target.slot, target.character)], { quiet: true });
          if (r) {
            ingest(r.signature, r.events);
            if (MOCK_VRF) {
              lock.current = false;
              await consume();
              lock.current = true;
            }
          }
        }
      }
      p = (await readParty(address, p.tiles))!;
      if (p.session.turnSeq === seq && !p.session.core.pendingVrf) {
        const r = await send("end monster turn", actor.signer, async () => [await endTurnIx(actor, address, null)], { quiet: true });
        if (r) ingest(r.signature, r.events);
      }
      await refresh();
    } finally {
      lock.current = false;
    }
  }, [address, ensureStarted, actorFor, send, ingest, consume, refresh, taken, label]);

  // Auto-play monster turns from the DM window (toggle).
  useEffect(() => {
    if (!autoMonsters || !isDm || !monsterTurn || s?.core.pendingVrf || busy) return;
    const t = setTimeout(() => void autoMonsterTurn(), 700);
    return () => clearTimeout(t);
  }, [autoMonsters, isDm, monsterTurn, s?.core.pendingVrf, s?.turnSeq, s?.startedSeq, busy, autoMonsterTurn]);

  // Dev server only: a plain snapshot for browser automation (tools drive the demo through clicks).
  useEffect(() => {
    if (!import.meta.env.DEV || !party) return;
    (window as unknown as { __deadrollParty?: unknown }).__deadrollParty = {
      status: status(party.session),
      unit: currentUnit(party.session),
      turnSeq: party.session.turnSeq,
      started: turnStarted(party.session),
      acted: turnActed(party.session),
      pending: party.session.core.pendingVrf,
      aliveMask: party.session.aliveMask,
      width: party.session.width,
      height: party.session.height,
      tiles: party.tiles ? Array.from(party.tiles) : [],
      tileCost: Array.from(party.session.tileCost),
      xpPool: party.session.xpPool,
      players: party.players.map((p) => ({ slot: p.slot, x: p.part.core.x, y: p.part.core.y, hp: p.part.core.hp, ap: playerAp(p.part, party.session), name: bytesText(p.char.name), player: p.part.player, nftFrozen: p.char.nftFrozen, cooldowns: Array.from(p.part.cooldowns), hpMax: p.part.hpMax, mods: activeMods(p.part.mods) })),
      monsters: party.monsters.map((m, i) => ({ i, bestiaryId: m.bestiaryId, x: m.x, y: m.y, hp: m.hp, alive: m.alive, ap: m.actionPoints, mods: activeMods(party.monsterMods[i]) })),
      events: events.map((e) => e.name),
    };
  }, [party, events]);

  if (error && !party) return <p className="error">{error}</p>;
  if (!party || !s) return <p className="muted">Loading party {shortAddress(address)}...</p>;

  const tiles = party.tiles ?? new Uint8Array(MAP_MAX * MAP_MAX);
  const unitSprite = (u: GridUnit) => {
    if (u.kind === "player") {
      const p = party.players.find((x) => x.slot === u.id);
      if (!p) return null;
      return u.alive ? draw(`c:${p.character}`, renderRequest(p.char.core.appearance)) : draw("tomb", TOMBSTONE);
    }
    const m = party.monsters[u.id - MONSTER_BASE];
    return m ? draw(`m:${m.bestiaryId}:${m.alive}`, monsterRequest(m.bestiaryId, !m.alive)) : null;
  };
  const tileSprite = (t: Tile) => draw(`t:${t}`, tileRequest(t));

  const header = (
    <p className="mono">
      Party{" "}
      <a href={explorerUrl("address", address)} target="_blank" rel="noreferrer">
        {shortAddress(address)}
      </a>{" "}
      · {st} · round {s.core.round} · DM {shortAddress(s.core.dm)}
      {isDm ? " (you)" : ""} · price {formatSol(s.core.priceLamports)} · escrow {formatSol(party.escrow?.collected ?? 0n)}
      {party.escrow?.claimed ? " (claimed)" : ""}
    </p>
  );

  // ---- lobby
  if (st === "lobby") {
    const players: LobbyPlayer[] = party.players.map((p) => ({
      slot: p.slot,
      player: p.part.player,
      characterName: bytesText(p.char.name),
      race: enumIndex(p.char.core.race, RACE_KEYS) as Race,
      class: enumIndex(p.char.core.class, CLASS_KEYS) as Class,
      ready: p.part.core.ready,
      paid: p.part.contribution,
    }));
    const myJoined = mine[mine.length - 1] ?? null;
    const joinable = myChars.filter((c) => c.data.core.alive && !c.data.core.inSession).map((c) => ({ address: c.address, name: bytesText(c.data.name) }));
    const remaining = s.core.priceLamports - (party.escrow?.collected ?? 0n);
    const share = (s.core.priceLamports + 1n) / 2n;
    const contribution = remaining < share ? remaining : share;
    const join = async (character: string) => {
      if (!wallet.signer) return void (await send("join_session", null, []));
      const key = sk.signer ?? (await sk.start());
      if (!key) return;
      const res = await send(`join_session (pay ${formatSol(contribution)})`, wallet.signer, async () => [
        await joinSessionIx(wallet.signer!, address, character as Address, contribution, key.address),
      ]);
      if (res) ingest(res.signature, res.events);
      void wallet.refreshBalance();
    };
    return (
      <section>
        <h1>
          Party lobby <LiveBadge status={live} />
        </h1>
        {header}
        <PartyLobby
          mapName={shortAddress(s.core.map)}
          dm={s.core.dm}
          priceLamports={s.core.priceLamports}
          collected={party.escrow?.collected ?? 0n}
          players={players}
          maxPlayers={PARTY_MAX}
          role={isDm ? "dm" : myJoined ? "player" : "guest"}
          mySlot={myJoined?.slot ?? null}
          myCharacters={joinable}
          onJoin={(c) => void join(c)}
          joinLamports={contribution}
          onReady={(ready) =>
            myJoined &&
            void send(ready ? "set_ready" : "set_ready(false)", wallet.signer, async () => [await setReadyIx(wallet.signer!, address, myJoined.character, ready)]).then(
              (r) => r && ingest(r.signature, r.events),
            )
          }
          onStart={() =>
            void act("start_session", wallet.signer, async () => [await startSessionIx(wallet.signer!, address)], true)
          }
          busy={busy !== null}
          error={!isDm && !myJoined && joinable.length === 0 && me ? "No free living character: create one on the Character page." : null}
          mapPreview={<Grid width={s.width} height={s.height} tiles={tiles} units={units} cellPx={12} />}
        />
        {!isDm && !myJoined && (
          <p className="muted">
            Join pays {formatSol(contribution)} into the escrow and binds your session key (started on demand) to the party.
          </p>
        )}
        {isDm && <p className="muted">Share this page's link with the players. Start needs everyone ready and the escrow at the price.</p>}
        <EventLog events={events} label={label} />
      </section>
    );
  }

  // ---- battle and its end
  const ended = st !== "active";
  const livingPlayers = party.players.filter((p) => s.aliveMask & (1 << p.slot));
  // A skill's targets within its reach (the weapon: every living monster, the program checks range).
  const reachOk = (c: Cell) => !chosen || !moverPos || inReach(chosen.side, chosen.form, cheb(moverPos, c));
  const monsterTargets: Cell[] = !canAttack
    ? []
    : myTurn && moverPos
      ? chosenSide === "ally"
        ? livingPlayers.map((p) => ({ x: p.part.core.x, y: p.part.core.y })).filter(reachOk)
        : party.monsters.filter((m) => m.alive).map((m) => ({ x: m.x, y: m.y })).filter(reachOk)
      : monsterTurn && isDm
        ? livingPlayers.map((p) => ({ x: p.part.core.x, y: p.part.core.y }))
        : [];

  return (
    <section>
      <h1>
        {ended ? `Party ended: ${st}` : "Battle"} <LiveBadge status={live} />
      </h1>
      {header}
      <div style={{ display: "flex", gap: 20, alignItems: "flex-start", flexWrap: "wrap" }}>
        <div>
          <Grid
            width={s.width}
            height={s.height}
            tiles={tiles}
            units={units}
            activeUnitId={unit}
            reachable={!ended && canMove ? reach : []}
            targets={!ended ? monsterTargets : []}
            path={preview}
            cellPx={CELL_PX}
            unitSprite={unitSprite}
            tileSprite={tileSprite}
            onCellHover={setHover}
            onCellClick={(c) => (!canMove ? undefined : myTurn ? movePlayer(c) : monsterTurn && isDm ? moveMonster(c) : undefined)}
            onUnitClick={(u) => {
              if (!canAttack) return;
              if (myTurn && chosenSide === "enemy" && u.kind === "monster" && u.alive) attack(u.id - MONSTER_BASE);
              else if (myTurn && chosenSide === "ally" && u.kind === "player" && u.alive) support(u.id);
              else if (monsterTurn && isDm && u.kind === "player" && u.alive) monsterAttack(u.id);
            }}
          />
          <p className="muted" style={{ maxWidth: CELL_PX * s.width }}>
            {controls && acted && started
              ? "Attack used: move with the AP left or end the turn."
              : myTurn
                ? chosenSide === "ally"
                  ? `Your turn: click an ally or yourself to cast ${chosen?.name ?? "the skill"} (one attack or skill per turn).`
                  : "Your turn: click a cell to move, a monster to attack with the selected skill (one attack per turn)."
                : monsterTurn && isDm
                  ? "Monster turn: click a cell to move it, a hero to attack (once), or Auto."
                  : ""}
          </p>
        </div>
        <div style={{ flex: "1 1 380px", display: "grid", gap: 12, minWidth: 320 }}>
          {!ended && (
            <div className="panel">
              <p className="turn-banner">
                Turn: <b className={myTurn || (monsterTurn && isDm) ? "me" : ""}>{unit !== null ? label(unit) : "initiative roll..."}</b>
                {unit !== null && (myTurn || monsterTurn) && <> · {moverAp} AP</>}
                {s.core.pendingVrf && <span className="muted"> · waiting for the VRF roll</span>}
                {unit !== null && !started && !s.core.pendingVrf && <span className="muted"> · {controls ? "starting the turn..." : "waiting for the turn to start"}</span>}
              </p>
              {myTurn && (
                <>
                  <div className="skills">
                    {activeSkills.map((k) => {
                      const cd = cooldowns[k.slot] ?? 0;
                      return (
                        <button
                          key={k.slot}
                          className={cd > 0 ? "btn cooldown" : "btn"}
                          disabled={!canAttack || cd > 0}
                          aria-pressed={chosenSkill === k.slot}
                          title={cd > 0 ? `On cooldown: ${cooldownLabel(cd)}` : k.side === "ally" ? "Heal or buff: click an ally or yourself" : k.purpose === Purpose.Debuff ? "Debuff: click a monster" : undefined}
                          onClick={() => setSkill(k.slot)}
                        >
                          {k.name}
                          {k.side === "ally" ? <span className="muted"> · ally</span> : k.purpose === Purpose.Debuff ? <span className="muted"> · debuff</span> : null}
                          {cd > 0 && <span className="cd"> · {cooldownLabel(cd)}</span>}
                        </button>
                      );
                    })}
                    <button className="btn" disabled={!canAttack} aria-pressed={chosenSkill === BASE_ATTACK} onClick={() => setSkill(BASE_ATTACK)}>
                      Weapon
                    </button>
                  </div>
                  <p className="muted">Session key: {sk.signer && activePlayer && sk.signer.address === activePlayer.part.core.actor ? "signing moves, no popups" : "not bound to this party; the wallet signs"}</p>
                </>
              )}
              <div className="row">
                {(myTurn || (monsterTurn && isDm)) && (
                  <button className="btn" disabled={busy !== null || !canMove} onClick={endTurn}>
                    End turn
                  </button>
                )}
                {monsterTurn && isDm && (
                  <button className="btn" disabled={busy !== null || s.core.pendingVrf} onClick={() => void autoMonsterTurn()}>
                    Auto monster turn
                  </button>
                )}
                {isDm && (
                  <label className="muted">
                    <input type="checkbox" checked={autoMonsters} onChange={(e) => setAutoMonsters(e.target.checked)} /> auto-play monsters
                  </label>
                )}
                {busy && <span className="muted">{busy}...</span>}
              </div>
            </div>
          )}
          {MOCK_VRF && !ended && (
            <div className="panel">
              <p className="muted">
                Localnet VRF oracle (session built with <code>mock-vrf</code>): this window signs <code>consume_randomness</code> after its own action.
              </p>
              <div className="row">
                <label>
                  Next seed{" "}
                  <select value={seedKind} onChange={(e) => setSeedKind(e.target.value as SeedKind)}>
                    <option value="random">random</option>
                    <option value="crit">natural 20</option>
                    <option value="miss">natural 1</option>
                    <option value="resonant">crit + tint (resonance)</option>
                  </select>
                </label>
                {s.core.pendingVrf && (
                  <button className="btn" disabled={busy !== null} onClick={() => void consume()}>
                    Roll now
                  </button>
                )}
              </div>
            </div>
          )}
          {roll?.hit && (
            <>
              <DiceCascade key={roll.key} hit={roll.hit.result} attackerLabel={roll.hit.attacker} targetLabel={roll.hit.target} critChanceBp={roll.hit.critChanceBp} />
              <HitBreakdown hit={roll.hit.result} defaultOpen={false} critChanceBp={roll.hit.critChanceBp} mods={roll.hit.mods} />
            </>
          )}
          {roll?.effect && (
            <EffectCascade
              key={`${roll.key}:effect`}
              effect={roll.effect.effect}
              casterLabel={roll.effect.caster}
              targetLabel={roll.effect.target}
              skillLabel={roll.effect.skill}
              critChanceBp={roll.effect.critChanceBp}
            />
          )}
          <StatsPanel units={statsUnits(party, label)} />
          {ended && <Claims party={party} isDm={isDm} payer={wallet.signer} send={send} ingest={ingest} busy={busy} />}
          {tombLog.length > 0 && (
            <div className="panel">
              <h3>Tombstones</h3>
              <ul className="mono log">
                {tombLog.map((e, i) => (
                  <li key={i}>
                    {e.status} {shortAddress(e.character)} {e.status === "sent" ? <a href={explorerUrl("tx", e.detail)} target="_blank" rel="noreferrer">freeze_tombstone</a> : e.detail}
                  </li>
                ))}
              </ul>
            </div>
          )}
          <EventLog events={events} label={label} skillName={skillName} />
        </div>
      </div>
    </section>
  );
}

function Claims({
  party,
  isDm,
  payer,
  send,
  ingest,
  busy,
}: {
  party: Party;
  isDm: boolean;
  payer: TransactionSigner | null;
  send: ReturnType<typeof useTx>["send"];
  ingest: (sig: string, evs: never[]) => void;
  busy: string | null;
}) {
  const s = party.session;
  return (
    <div className="panel">
      <h3>{status(s) === "victory" ? "Victory" : "The party is over"}</h3>
      <p>
        XP pool {s.xpPool} · every living hero gets all of it; a tombstone gets nothing.
      </p>
      <ul>
        {party.players.map((p) => {
          const settled = (s.settledMask & (1 << p.slot)) !== 0;
          return (
            <li key={p.slot}>
              {bytesText(p.char.name)} ({(s.aliveMask & (1 << p.slot)) !== 0 ? "alive" : "tombstone"}) · {p.char.core.xp} xp{" "}
              {settled ? (
                <span className="ok">claimed</span>
              ) : (
                <button
                  className="btn"
                  disabled={busy !== null}
                  onClick={() => void send(`claim_rewards ${bytesText(p.char.name)}`, payer, async () => [await claimRewardsIx(party.address, p.character)]).then((r) => r && ingest(r.signature, r.events as never[]))}
                >
                  Claim rewards
                </button>
              )}
            </li>
          );
        })}
      </ul>
      {isDm && party.escrow && !party.escrow.claimed && (
        <button
          className="btn primary"
          disabled={busy !== null}
          onClick={() => void send(`claim_escrow ${formatSol(party.escrow!.collected)}`, payer, async () => [await claimEscrowIx(payer!, party.address, s.core.map)]).then((r) => r && ingest(r.signature, r.events as never[]))}
        >
          Claim escrow ({formatSol(party.escrow.collected)})
        </button>
      )}
      {party.escrow?.claimed && <p className="ok">The DM took {formatSol(party.escrow.collected)} from the escrow; the map is finished.</p>}
    </div>
  );
}

type SkillName = (unit: number, slot: number) => string;

/** One log line per event; null for an event with nothing to tell (a quiet turn start). */
function describe(e: PartyEvent, label: (u: number) => string, skillName: SkillName): string | null {
  const d = e.data;
  switch (e.name) {
    case "SessionOpened":
      return `party opened, ${d.monsters} monsters`;
    case "PlayerJoined":
      return `slot ${d.slot} joined, paid ${formatSol(BigInt(d.contribution))}`;
    case "PlayerReady":
      return `slot ${d.slot} ${d.ready ? "ready" : "not ready"}`;
    case "InitiativeRolled":
      return `initiative: ${Array.from(d.order as Uint8Array).slice(0, d.len).map((u, i) => `${label(u)} (d20 ${d.d20[i]})`).join(", ")}`;
    case "TurnStarted":
      return `turn: ${label(d.unit)} (round ${d.round})`;
    case "TurnTicked":
      return tickText(d, label(d.unit));
    case "ModsTicked":
      return modsTickText(d, label(d.unit));
    case "EffectResolved": {
      const ev = toEffectResolved(d);
      return effectText(ev, label(ev.caster), label(ev.target), skillName(ev.caster, ev.skillSlot));
    }
    case "TurnSkipped":
      return skipText(d, label(d.unit));
    case "SkillUsed":
      return skillUsedText(d, label(d.unit), skillName(d.unit, d.skillSlot));
    case "Moved":
      return `${label(d.unit)} moved to ${d.x},${d.y}`;
    case "ActionRequested":
      if (label(d.attacker) === "initiative") return `initiative roll requested (action ${d.action})`;
      return `${label(d.attacker)} -> ${label(d.target)}: VRF requested (action ${d.action})`;
    case "HitResolved": {
      const r = d.result;
      const res = (r.resonances as unknown[]).some((x) => x && (x as { active: boolean }).active);
      return `${label(d.attacker)} -> ${label(d.target)}: d20 ${r.d20}${r.crit ? " CRIT" : ""}, ${Object.keys(r.outcome)[0]}, ${r.damage} dmg${res ? ", RESONANCE" : ""}`;
    }
    case "MonsterKilled":
      return `${label(MONSTER_BASE + d.monster)} killed${d.boss ? " (boss)" : ""}, +${d.xp} xp`;
    case "PlayerDied":
      return `${label(d.slot)} died`;
    case "CharacterDied":
      return `character ${shortAddress(d.character)} is a tombstone`;
    case "SessionEnded":
      return `party ended: ${Object.keys(d.status)[0]}`;
    case "RewardsClaimed":
      return `rewards claimed: ${d.xp} xp`;
    case "EscrowClaimed":
      return `escrow claimed by the DM: ${formatSol(BigInt(d.amount))}`;
    case "FleeVoted":
      return `slot ${d.slot} votes to flee`;
    default:
      return e.name;
  }
}

/**
 * Combat stats of every living unit with its buffs and debuffs (Participation.mods,
 * MonsterState.mods): HP, AP per turn (Haste / Slow), defense, armor, resistances, vulnerabilities.
 */
function statsUnits(party: Party, label: (u: number) => string): StatsUnit[] {
  const s = party.session;
  const current = currentUnit(s);
  const running = turnStarted(s);
  const players = party.players
    .filter((p) => s.aliveMask & (1 << p.slot))
    .map((p): StatsUnit => {
      const mine = current === p.slot && running;
      const stats = unitStats({
        hp: p.part.core.hp,
        hpMax: p.part.hpMax || p.char.core.hpMax,
        apBase: p.part.maxAp,
        apNow: mine ? playerAp(p.part, s) : null,
        profile: playerDefense(enumIndex(p.char.core.class, CLASS_KEYS) as Class, p.char.core.stats),
        mods: activeMods(p.part.mods),
      });
      return { unit: p.slot, label: label(p.slot), kind: "player", current: current === p.slot, stats };
    });
  const monsters = party.monsters.flatMap((m, i): StatsUnit[] => {
    const profile = MONSTER_DEFENSE[m.bestiaryId];
    if (!m.alive || !profile) return [];
    const u = MONSTER_BASE + i;
    const stats = unitStats({
      hp: m.hp,
      hpMax: m.hpMax,
      apBase: monsterBaseAp(s.monsterSpeed[i] ?? m.actionPoints),
      apNow: current === u && running ? m.actionPoints : null,
      profile,
      mods: activeMods(party.monsterMods[i]),
    });
    return [{ unit: u, label: label(u), kind: "monster", current: current === u, stats }];
  });
  return [...players, ...monsters];
}

function EventLog({ events, label, skillName = (_, slot) => `skill ${slot}` }: { events: PartyEvent[]; label: (u: number) => string; skillName?: SkillName }) {
  const lines = events.flatMap((e, i) => {
    const text = describe(e, label, skillName);
    return text === null ? [] : [{ e, i, text }];
  });
  if (lines.length === 0) return null;
  return (
    <div className="panel">
      <h3>Every roll is public</h3>
      <ol className="mono log" reversed>
        {[...lines].reverse().map(({ e, i, text }) => (
          <li key={`${e.signature}-${i}`}>
            {text}{" "}
            <a href={explorerUrl("tx", e.signature)} target="_blank" rel="noreferrer">
              tx
            </a>
          </li>
        ))}
      </ol>
    </div>
  );
}

/** Log subscription state: events (dice, deaths) arrive live only while this says "live". */
function LiveBadge({ status }: { status: SubStatus }) {
  const tone = status.state === "live" ? " chain" : status.state === "reconnecting" ? " reconnecting" : "";
  const title = status.state === "reconnecting" ? `log subscription dropped: ${status.reason}` : "session program log subscription";
  return (
    <span className={`badge${tone}`} title={title}>
      {statusLabel(status)}
    </span>
  );
}
