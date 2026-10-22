// Party screen on chain (L9.6): open a party on a finalized map, pay and join, session keys,
// ready, start, initiative, moves, attacks with the dice cascade from HitResolved, monster turns
// by the DM, death + freeze_tombstone, victory and claims. The calls mirror tests/demo.test.ts.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import type { Address, TransactionSigner } from "@solana/kit";
import { HitCard } from "../components/dice/HitCard";
import { rollKeyOf } from "../chain/rollKey";
import type { HitItemSides, HitMods } from "../components/dice/types";
import { StatsPanel } from "../components/stats/StatsPanel";
import type { StatsUnit } from "../components/stats/types";
import { Grid } from "../components/grid/Grid";
import type { GridAura, GridUnit } from "../components/grid/types";
import { PartyLobby } from "../components/forms/PartyLobby";
import { PrepayConfirm } from "../components/forms/PrepayConfirm";
import type { LobbyPlayer } from "../components/forms/types";
import { isCurrentAppearance, monsterRequest, tileRequest, TOMBSTONE } from "../chain/appearance";
import { characterRequest } from "../chain/items";
import { HitOutcome, ModKindData, ReactionKind, SessionStatus, Trigger } from "@deadroll/sdk/session";
import {
  BASE_ATTACK,
  MONSTER_BASE,
  actIx,
  optional,
  variantKey,
  bytesText,
  claimEscrowIx,
  claimRewardsIx,
  deathAccountsOf,
  withDeaths,
  consumeIx,
  currentUnit,
  endTurnIx,
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
  partyAccountsOf,
  pickSeed,
  playerAp,
  readParty,
  sessionAccounts,
  setReadyIx,
  setSessionKeyIx,
  DEFAULT_KEY,
  startSessionIx,
  startTurnIx,
  status,
  type Actor,
  type AuraAcc,
  type CharacterAcc,
  type SkillRecordAcc,
  type GameSessionAcc,
  type MapAcc,
  type Party,
  type SeedKind,
} from "../chain/game";
import { toHitResolved, toResonanceSpread } from "../chain/hit";
import { aspectName, bestiaryName, carrierName, resonanceName, stateName } from "../chain/names";
import {
  blockedOf,
  heroInputOf,
  partyInputOf,
  percent,
  previewOnMonster,
  previewText,
  statesOf,
  useCombat,
  type AccuracyInput,
  type AiUnitInput,
  type CombatWasm,
  type HitPreview,
} from "../chain/combatWasm";
import { ACTIVE_SLOTS, REACTIVE_SLOT, kinOf, monsterBearersOf, monsterSkillName, monsterSkillsOf, skillAction, useMonsterSkillTable, type MonsterSkill } from "../chain/monsters";
import { errorName } from "../chain/send";
import { statusLabel, type SubStatus } from "../chain/subscribe";
import { cooldownLabel, skillCooldowns, skillUsedText, skipText, startTurnFor, takenOntoMap, tickText, turnActed, turnStarted } from "../chain/turns";
import { skillSetPda } from "../chain/pda";
import { affordable, cheb, reachable, route, type Step } from "../chain/route";
import { useAutoFreezeTombstone } from "../chain/useAutoFreezeTombstone";
import { useParty, type PartyEvent } from "../chain/useParty";
import { BASIC_ATTACK_AP, Purpose, inReach, rangeReach, type RangeTag } from "../chain/skillcheck";
import { monsterBaseAp, monsterDefense, playerDefense, unitStats } from "../chain/stats";
import { activeMods, combatSkill, effectText, modsAt, modsTickText, toEffectResolved, type CombatSkill, type EffectResolved } from "../chain/support";
import { useTx } from "../chain/useTx";
import { MAP_MAX, PARTY_MAX, type Cell, type Class, type HitResult, type Race, type Tile } from "../chain/types";
import { MOCK_VRF, PROGRAMS, explorerUrl } from "../config";
import { useSprites } from "../raster/useSprites";
import { fetchSessionToken, revokeSessionInstruction } from "../session-key/magicblock";
import { PartyKeyPanel } from "../session-key/PartyKeyPanel";
import { FEES, readyPrepay, topUpRoom } from "../session-key/sessionKey";
import { commitKey, prepareKey, usePartyKey, type PartyKey, type PreparedKey } from "../session-key/usePartyKey";
import { sessionTokenPda } from "../chain/pda";
import { useAppWallet } from "../wallet/AppWallet";
import { formatSol, shortAddress } from "../wallet/ConnectButton";

const CELL_PX = 64; // sprites: one whole 64 px tile per cell (grid integerCell)

export function PartyChain() {
  const [params] = useSearchParams();
  const s = params.get("s") as Address | null;
  return s ? <PartyView address={s} /> : <PartyPicker map={params.get("map") as Address | null} />;
}

// ---------------------------------------------------------------------------------------------
// no party selected: open one (DM) or pick one (players)

function PartyPicker({ map }: { map: Address | null }) {
  const wallet = useAppWallet();
  const { send, busy } = useTx();
  const [, setParams] = useSearchParams();
  const [price, setPrice] = useState("0.1");
  // Opening moves the DM key's prepayment: the DM confirms the amount before anything is signed.
  const [confirming, setConfirming] = useState(false);
  const [mapAcc, setMapAcc] = useState<MapAcc | null | "missing">(null);
  const [sessions, setSessions] = useState<{ address: Address; data: GameSessionAcc }[]>([]);
  const [myMaps, setMyMaps] = useState<{ address: Address; data: MapAcc }[]>([]);

  const refresh = useCallback(async () => {
    try {
      setSessions(await openSessions());
      if (wallet.address) setMyMaps((await mapsOf(wallet.address)).filter((m) => mapStatus(m.data) === "finalized"));
      if (map) setMapAcc((await getAccount("Map", map)) ?? "missing");
    } catch {
      // lists stay as they are
    }
  }, [map, wallet.address]);
  useEffect(() => void refresh(), [refresh]);

  const open = async () => {
    if (!map || !wallet.signer) return void (await send("open_session", wallet.signer, []));
    // The DM's key for this party moves monsters and ends their turns without popups: its token,
    // its prepayment and the opening are one wallet signature (docs/GAME_COMMISSIONS.md 7).
    const signer = wallet.signer;
    const dmKey = await prepareKey(signer);
    const lamports = BigInt(Math.round(Number(price) * 1e9));
    const res = await send("open_session", signer, async () => [dmKey.instruction, await openSessionIx(signer, map, lamports, dmKey.signer.address)]);
    if (!res) return;
    const { session } = await sessionAccounts(map);
    commitKey(signer.address, session, dmKey);
    setParams({ s: session });
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
            {!confirming && (
              <button className="btn primary" disabled={busy !== null || used !== "finalized"} onClick={() => setConfirming(true)}>
                {busy ?? "Open party"}
              </button>
            )}
          </div>
          {confirming && (
            <PrepayConfirm
              action="Open party"
              lamports={FEES.prepay}
              busy={busy !== null || used !== "finalized"}
              onConfirm={() => {
                setConfirming(false);
                void open();
              }}
              onCancel={() => setConfirming(false)}
            />
          )}
          <p className="muted">
            One signature: the party, plus a session key with {formatSol(FEES.prepay)} for its fees. The key moves the monsters and ends their turns.
          </p>
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

type Hit = { result: HitResult; attacker: string; target: string; critChanceBp?: number; mods: HitMods; items: HitItemSides };
type EffectView = { effect: EffectResolved; caster: string; target: string; skill: string; critChanceBp?: number };
/** The dice panel: a hit, a heal / buff / debuff, or a hit followed by its skill's effect. */
type Roll = { key: string; hit: Hit | null; effect: EffectView | null };

function PartyView({ address }: { address: Address }) {
  const wallet = useAppWallet();
  const pk = usePartyKey(address);
  const { send, busy } = useTx();
  const { party, error, events, ingest, refresh, live } = useParty(address);
  const combat = useCombat();
  // The session's resonance tables (copied from the registry at open_session) into the module,
  // so the preview resolves the same pairs as the program.
  const tables = party?.resonanceTables ?? null;
  const tablesReady = useMemo(() => {
    if (!combat || !tables) return false;
    try {
      combat.setTables(tables);
      return true;
    } catch (err) {
      console.warn(err);
      return false;
    }
  }, [combat, tables]);
  const draw = useSprites();
  const [seedKind, setSeedKind] = useState<SeedKind>("random");
  const [skill, setSkill] = useState<number>(0);
  const [hover, setHover] = useState<Cell | null>(null);
  const [autoMonsters, setAutoMonsters] = useState(false);
  /** The DM's chosen monster action: a skill slot, null = the natural attack. */
  const [monsterSlot, setMonsterSlot] = useState<number | null>(null);
  const skillTable = useMonsterSkillTable();
  const [roll, setRoll] = useState<Roll | null>(null);
  const [myChars, setMyChars] = useState<{ address: Address; data: CharacterAcc }[]>([]);
  const [skillNames, setSkillNames] = useState<Record<string, CombatSkill[]>>({});
  /** The skill records of party characters, for the attack preview. */
  const [skillRecords, setSkillRecords] = useState<Record<string, SkillRecordAcc[]>>({});
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

  // The party's key pays what anyone may send (start_turn, the localnet oracle, tombstones).
  const payer: TransactionSigner | null = (pk.live ? pk.signer : null) ?? wallet.signer;
  const tombLog = useAutoFreezeTombstone(payer, me);

  /** Who signs for the unit whose bound key is `actor`: this browser's key with its token, or the
   * main wallet (a popup). */
  const actorFor = useCallback(
    (actor: Address): Actor | null => {
      const key = pk.signerFor(actor);
      if (key && pk.token) return { signer: key, token: pk.token };
      return wallet.signer ? { signer: wallet.signer, token: null } : null;
    },
    [pk, wallet.signer],
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
      // A monster's skill by its slot: the registry name of its session copy.
      if (u >= MONSTER_BASE) {
        const copy = party?.monsterSkills[u - MONSTER_BASE]?.[slot];
        return copy ? monsterSkillName(copy, skillTable) : `skill ${slot}`;
      }
      const pl = party?.players.find((p) => p.slot === u);
      return (pl && skillNames[pl.character]?.find((k) => k.slot === slot)?.name) || `skill ${slot}`;
    },
    [party, skillNames, skillTable],
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
        const set = await getAccount("SkillSet", await skillSetPda(p.character));
        const records = (set?.skills ?? []).slice(0, set?.count ?? 0);
        return [p.character, records] as const;
      }),
    ).then((entries) => {
      setSkillRecords(Object.fromEntries(entries));
      // The participation's snapshot (set_ready) carries each map skill's range and AP cost.
      setSkillNames(
        Object.fromEntries(
          entries.map(([c, records]) => {
            const part = party.players.find((p) => p.character === c)?.part;
            const map = (slot: number) => {
              const i = part ? Array.from(part.skills).indexOf(slot) : -1;
              return i >= 0 ? (part!.mapSkills[i] ?? null) : null;
            };
            return [c, records.map((k, slot) => combatSkill(slot, k, map(slot)))];
          }),
        ),
      );
    });
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
    const key = rollKeyOf(events, i);
    if (key === lastRollKey.current) return;
    lastRollKey.current = key;
    // Crit chance is not in the events: a player's comes from the combat module (its gear and
    // stats, as the program saw them), monsters never roll crit.
    const critOf = (u: number) => {
      const p = u < MONSTER_BASE ? party?.players.find((x) => x.slot === u) : undefined;
      if (!p || !combat) return undefined;
      return combat.heroSheet(heroInputOf(p.char, p.part.equipment, p.char.core.stats, p.part.classTier)).critRollBp;
    };
    // Mods are not in HitResolved: take each side's from the events before the hit, else the accounts.
    const modsNow = (u: number) =>
      !party ? [] : u < MONSTER_BASE ? activeMods(party.players.find((p) => p.slot === u)?.part.mods) : activeMods(party.monsterMods[u - MONSTER_BASE]);
    /** The action of a hit: the requested one (ActionRequested of its action counter), or the
     * reaction of its attacker on that counter. */
    const actionOf = (counter: number, attacker: number): string | null => {
      const req = events.find((e) => e.name === "ActionRequested" && e.data.action === counter);
      if (req && req.data.attacker === attacker) {
        const slot = req.data.skillSlot as number;
        if (attacker < MONSTER_BASE) return slot === BASE_ATTACK ? "weapon" : skillName(attacker, slot);
        return slot === 0 ? "natural attack" : skillName(attacker, slot - 1);
      }
      const monster = events.find((e) => e.name === "MonsterReactiveTriggered" && e.data.actionCounter === counter && MONSTER_BASE + e.data.bearer === attacker);
      if (monster) return `reaction: ${skillName(attacker, REACTIVE_SLOT)}`;
      const player = events.find((e) => e.name === "ReactiveTriggered" && e.data.actionCounter === counter && e.data.bearer === attacker);
      return player ? `reaction: ${skillName(attacker, player.data.skillSlot)}` : null;
    };
    // A player's state resistance (Endurance and items, patch 9), from the same combat module.
    const stateResistOf = (u: number) => {
      const p = u < MONSTER_BASE ? party?.players.find((x) => x.slot === u) : undefined;
      if (!p || !combat) return 0;
      return combat.heroSheet(heroInputOf(p.char, p.part.equipment, p.char.core.stats, p.part.classTier)).stateResistBp;
    };
    const hitView = (data: unknown, at: number): Hit => {
      const h = toHitResolved(data);
      const mods = {
        attacker: modsAt(events, at, h.attacker, modsNow(h.attacker)),
        target: modsAt(events, at, h.target, modsNow(h.target)),
        targetStateResistBp: stateResistOf(h.target),
      };
      // Items only for player sides: a monster wears nothing.
      const items = { attacker: h.attacker < MONSTER_BASE ? h.items.attacker : null, target: h.target < MONSTER_BASE ? h.items.defender : null };
      const action = actionOf(h.actionCounter, h.attacker);
      const attacker = action ? `${label(h.attacker)} (${action})` : label(h.attacker);
      return { result: h.result, attacker, target: label(h.target), critChanceBp: critOf(h.attacker), mods, items };
    };
    if (ev.name === "HitResolved") return setRoll({ key, hit: hitView(ev.data, i), effect: null });
    const e = toEffectResolved(ev.data);
    const effect: EffectView = { effect: e, caster: label(e.caster), target: label(e.target), skill: skillName(e.caster, e.skillSlot), critChanceBp: critOf(e.caster) };
    const hitAt = e.rider ? lastOf((x) => x.name === "HitResolved" && x.data.actionCounter === e.actionCounter, i) : -1;
    const hitEv = events[hitAt];
    setRoll({ key, hit: hitEv ? hitView(hitEv.data, hitAt) : null, effect });
  }, [events, label, party, skillName, combat]);

  // ---- the localnet VRF oracle (mock-vrf build): answers the pending roll of this party.
  const consume = useCallback(
    async (kind: SeedKind = seedKind, quietFail = false) => {
      const p = await readParty(address, party?.tiles ?? null);
      if (!p?.session.core.pendingVrf) return;
      const pend = p.session.pending.participation;
      const pl = p.players.find((x) => x.participation === pend);
      if (!pl) return;
      const seed = await pickSeed(kind);
      const res = await send("VRF roll (localnet oracle)", payer, async () => [await consumeIx(address, pend, pl.character, seed, p.session.layout, partyAccountsOf(p.session, pend))], {
        quiet: true,
        silent: quietFail,
      });
      if (res) ingest(res.signature, res.events);
    },
    [address, party?.tiles, seedKind, send, payer, ingest],
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
          // Finalizing deaths (patch 6): any tab of the party pays, with its own session key.
          const own = next.finalize ? payer : null;
          const actor = next.finalize ? (own ? { signer: own, token: null } : null) : actorFor(next.player ? next.player.part.core.sessionKey : p.session.dmActor);
          if (!actor) break;
          const player = next.player ? { participation: next.player.participation, character: next.player.character } : null;
          const deaths = await deathAccountsOf(p.session);
          let failed = false;
          const what = next.finalize ? "finalize deaths" : `start turn ${label(next.unit)}`;
          const res = await send(what, actor.signer, async () => withDeaths(deaths, await startTurnIx(address, player, actor.signer, deaths)), {
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
        // Cells per side: the bestiary record (`record::CELLS`).
        size: ((party.monsterRecords[i]?.[3] ?? 1) === 2 ? 2 : 1) as 1 | 2,
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
  // Active skills the current player took onto the map; one on cooldown cannot be picked, the
  // weapon never is.
  const activeSkills = activePlayer ? (skillNames[activePlayer.character] ?? []).filter((k) => k.usable && takenOntoMap(activePlayer.part, k.slot)) : [];
  const cooldowns = activePlayer && s ? skillCooldowns(activePlayer.part, events, activePlayer.slot, s.turnSeq) : [];
  const chosen = activeSkills.find((k) => k.slot === skill && !cooldowns[k.slot]) ?? null;
  const chosenSkill = chosen ? chosen.slot : BASE_ATTACK;
  // Heals and buffs go to an ally (self included), everything else to a monster.
  const chosenSide = chosen?.side ?? "enemy";

  // The current monster's active skills (DM), the chosen one, its side; a buff or heal goes to an
  // allied monster (of its kin for a kin-only skill), everything else to a player.
  const mIndex = monsterTurn && unit !== null ? unit - MONSTER_BASE : -1;
  const mSkills: MonsterSkill[] = combat && party && mIndex >= 0 ? monsterSkillsOf(party, mIndex, combat, skillTable).filter((k) => k.slot < ACTIVE_SLOTS) : [];
  const mCooldowns = party?.monsterCooldowns[mIndex] ?? new Uint8Array(0);
  const mChosen = mSkills.find((k) => k.slot === monsterSlot && !(mCooldowns[k.slot] ?? 0)) ?? null;
  const mSide = mChosen && mChosen.sheet.blocks[1] === Purpose.Buff ? "ally" : "enemy";
  const mSheet = combat && party && mIndex >= 0 && party.monsterRecords[mIndex] ? (() => {
    try {
      return combat.monsterSheet({ record: party.monsterRecords[mIndex]!, level: party.monsters[mIndex]!.level, flags: party.monsters[mIndex]!.flags });
    } catch {
      return null;
    }
  })() : null;
  const mRange = (mChosen ? mChosen.sheet.ext[0]! : (mSheet?.range ?? 0)) as RangeTag;
  const mAp = mChosen ? mChosen.sheet.apCost : BASIC_ATTACK_AP;

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
    const actor = actorFor(activePlayer.part.core.sessionKey);
    void act(`move ${label(activePlayer.slot)}`, actor?.signer ?? null, async () => [await moveIx(actor!, address, activePlayer.character, s.layout, steps)], false);
  };
  const attack = (monster: number) => {
    if (!activePlayer || !canAttack || chosenSide !== "enemy") return;
    const actor = actorFor(activePlayer.part.core.sessionKey);
    void act(`attack ${label(MONSTER_BASE + monster)}`, actor?.signer ?? null, async () => [await actIx(actor!, address, activePlayer.character, chosenSkill, monster, null, s!.layout)], true);
  };
  /** A heal or buff on an ally: the target is its player slot, its Participation unless a self-cast. */
  const support = (slot: number) => {
    const ally = party?.players.find((p) => p.slot === slot);
    if (!activePlayer || !canAttack || chosenSide !== "ally" || !ally || !chosen) return;
    const actor = actorFor(activePlayer.part.core.sessionKey);
    const allyAccount = slot === activePlayer.slot ? null : ally.participation;
    void act(`${chosen.name} on ${label(slot)}`, actor?.signer ?? null, async () => [await actIx(actor!, address, activePlayer.character, chosenSkill, slot, allyAccount, s!.layout)], true);
  };
  const endTurn = () => {
    if (!s) return;
    if (activePlayer) {
      const actor = actorFor(activePlayer.part.core.sessionKey);
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
    if (!s || unit === null || !target || !canAttack || mSide !== "enemy") return;
    const actor = actorFor(s.dmActor);
    const action = mChosen ? skillAction(mChosen.slot) : 0;
    const what = mChosen ? `${label(unit)}: ${mChosen.name} on ${label(slot)}` : `${label(unit)} attacks ${label(slot)}`;
    void act(what, actor?.signer ?? null, async () => [await monsterActIx(actor!, address, unit - MONSTER_BASE, action, slot, target.character, s.layout)], true);
  };
  /** A monster's heal or buff on an allied monster: the callback's participation is the lowest
   * living player's. */
  const monsterSupport = (index: number) => {
    const first = party?.players.filter((p) => s && s.aliveMask & (1 << p.slot)).sort((a, b) => a.slot - b.slot)[0];
    if (!s || unit === null || !first || !canAttack || mSide !== "ally" || !mChosen) return;
    const actor = actorFor(s.dmActor);
    void act(`${label(unit)}: ${mChosen.name} on ${label(MONSTER_BASE + index)}`, actor?.signer ?? null, async () => [await monsterActIx(actor!, address, unit - MONSTER_BASE, skillAction(mChosen.slot), index, first.character, s.layout)], true);
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
      if (m?.alive && living.length && combat) {
        // The plan: what the simulator's monsters do (deadroll_rules::monster_ai), the target and
        // the reach. Walk into reach with the AP the action leaves, then act.
        const unitOf = (cell: Cell, hp: number, hpMax: number, alive: boolean, states: AiUnitInput["states"], mods: AiUnitInput["mods"], kin: number): AiUnitInput => ({ cell: [cell.x, cell.y], hp, hpMax, alive, states, mods, kin });
        const heroes = Array.from({ length: p.session.core.playerCount }, (_, slot) => {
          const pl = p!.players.find((x) => x.slot === slot);
          return pl
            ? unitOf({ x: pl.part.core.x, y: pl.part.core.y }, pl.part.core.hp, pl.part.hpMax || pl.char.core.hpMax, (p!.session.aliveMask & (1 << slot)) !== 0, statesOf(pl.part.core.states), activeMods(pl.part.mods), 0xff)
            : unitOf({ x: 0, y: 0 }, 0, 0, false, [], [], 0xff);
        });
        const monsters = p.monsters.map((x, j) => unitOf({ x: x.x, y: x.y }, x.hp, x.hpMax, x.alive, statesOf(x.states), activeMods(p!.monsterMods[j]), kinOf(p!, j)));
        let plan = null as ReturnType<CombatWasm["monsterPlan"]> | null;
        try {
          plan = combat.monsterPlan({
            record: p.monsterRecords[mi]!,
            level: m.level,
            index: mi,
            ap: m.actionPoints,
            skills: p.monsterSkills[mi] ?? [],
            cooldowns: Array.from(p.monsterCooldowns[mi] ?? []),
            heroes,
            monsters,
          });
        } catch (err) {
          console.warn(err);
        }
        const nearest = living.reduce((a, b) => (cheb(m, { x: b.part.core.x, y: b.part.core.y }) < cheb(m, { x: a.part.core.x, y: a.part.core.y }) ? b : a));
        const targetCell: Cell = plan?.found
          ? plan.ally
            ? { x: p.monsters[plan.target]!.x, y: p.monsters[plan.target]!.y }
            : { x: heroes[plan.target]!.cell[0], y: heroes[plan.target]!.cell[1] }
          : { x: nearest.part.core.x, y: nearest.part.core.y };
        const reachCells = plan?.found ? rangeReach(plan.range as RangeTag) : 1;
        const inRange = (c: Cell) => {
          const d = cheb(c, targetCell);
          return d <= reachCells && (plan?.ally ? true : d >= 1);
        };
        let ap = m.actionPoints;
        const keep = plan?.found ? plan.ap : BASIC_ATTACK_AP;
        if (!inRange(m)) {
          const steps = route({ from: m, goal: (x, y) => inRange({ x, y }), tiles: p.tiles, width: p.session.width, height: p.session.height, tileCost: Array.from(p.session.tileCost), taken: taken(p, u) });
          const take = steps ? affordable(steps, ap, keep) : [];
          if (take.length) {
            const r = await send(`move ${label(u)}`, actor.signer, async () => [await monsterMoveIx(actor, address, p!.session.layout, mi, take)], { quiet: true });
            if (r) ingest(r.signature, r.events);
            ap -= take.reduce((n, x) => n + x.cost, 0);
          }
        }
        p = (await readParty(address, p.tiles))!;
        const mm = p.monsters[mi]!;
        if (plan?.found && ap >= plan.ap && mm.alive && inRange(mm) && !turnActed(p.session) && !p.session.core.pendingVrf) {
          const action = plan.slot === null ? 0 : skillAction(plan.slot);
          const first = p.players.filter((pl) => p!.session.aliveMask & (1 << pl.slot)).sort((a, b) => a.slot - b.slot)[0]!;
          const character = plan.ally ? first.character : p.players.find((pl) => pl.slot === plan!.target)!.character;
          const what = `${label(u)}: ${plan.slot === null ? "natural attack" : skillName(u, plan.slot)} on ${label(plan.ally ? MONSTER_BASE + plan.target : plan.target)}`;
          const r = await send(what, actor.signer, async () => [await monsterActIx(actor, address, mi, action, plan!.target, character, p!.session.layout)], { quiet: true });
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
  }, [address, ensureStarted, actorFor, send, ingest, consume, refresh, taken, label, combat, skillName]);

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
      // A character from before the current graphics is not drawn: the grid shows its marker.
      if (!p || !isCurrentAppearance(p.char.core.appearance)) return null;
      return u.alive ? draw(`c:${p.character}`, characterRequest(p.char)) : draw("tomb", TOMBSTONE);
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
      race: p.char.core.race as Race,
      class: p.char.core.class as Class,
      ready: p.part.core.ready,
      paid: p.part.contribution,
    }));
    const myJoined = mine[mine.length - 1] ?? null;
    const joinable = myChars.filter((c) => c.data.core.alive && !optional(c.data.core.inSession)).map((c) => ({ address: c.address, name: bytesText(c.data.name) }));
    const remaining = s.core.priceLamports - (party.escrow?.collected ?? 0n);
    const share = (s.core.priceLamports + 1n) / 2n;
    const contribution = remaining < share ? remaining : share;
    const join = async (character: string) => {
      const signer = wallet.signer;
      if (!signer) return void (await send("join_session", null, []));
      // Joining pays the share of the price only; the session key and its prepayment come with Ready.
      const res = await send(`join_session (pay ${formatSol(contribution)})`, signer, async () => [
        await joinSessionIx(signer, address, character as Address, contribution, DEFAULT_KEY),
      ]);
      if (res) ingest(res.signature, res.events);
      void wallet.refreshBalance();
    };
    // Ready (docs/PARTY_AND_TURNS.md 2.1) is one main wallet transaction: the session key for this
    // participation (a new token with the prepayment and set_session_key, unless this browser's
    // live key is already bound) and set_ready(true). Not ready leaves the key as it is.
    const myBound = myJoined?.part.core.sessionKey ?? DEFAULT_KEY;
    const ready = async (value: boolean, selection: number[]) => {
      const signer = wallet.signer;
      if (!myJoined || !signer) return void (await send("set_ready", null, []));
      const key: { prepared: PreparedKey | null } = { prepared: null };
      const res = await send(value ? "Ready" : "Not ready", signer, async () => {
        const k = value ? await pk.readyIxs(signer, address, myJoined.character, myBound) : { ixs: [], prepared: null };
        key.prepared = k.prepared;
        return [...k.ixs, await setReadyIx(signer, address, myJoined.character, value, selection)];
      });
      if (res) {
        if (key.prepared) pk.replaced(key.prepared);
        ingest(res.signature, res.events);
      }
      void wallet.refreshBalance();
    };
    return (
      <section>
        <h1>
          Party lobby <LiveBadge status={live} />
        </h1>
        {header}
        <SessionKeyBox party={party} pk={pk} isDm={isDm} me={me} send={send} busy={busy} />
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
          mySkills={myJoined ? (skillNames[myJoined.character] ?? []).map((k) => ({ index: k.slot, name: k.name, category: k.category })) : undefined}
          onReady={(value, selection) => void ready(value, selection)}
          readyPrepayLamports={readyPrepay(pk.readyPlan(myBound))}
          onStart={() =>
            void act("start_session", wallet.signer, async () => [await startSessionIx(wallet.signer!, address)], true)
          }
          busy={busy !== null}
          error={!isDm && !myJoined && joinable.length === 0 && me ? "No free living character: create one on the Character page." : null}
          mapPreview={<Grid width={s.width} height={s.height} tiles={tiles} units={units} cellPx={12} />}
        />
        {!isDm && !myJoined && (
          <p className="muted">
            Join is one signature: {formatSol(contribution)} into the escrow. The session key for this party and its {formatSol(FEES.prepay)} fee prepayment
            come with Ready, after you confirm the amount.
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
  const reachOk = (c: Cell) => !chosen || !moverPos || inReach(chosen.side, chosen.range, cheb(moverPos, c));
  const monsterTargets: Cell[] = !canAttack
    ? []
    : myTurn && moverPos
      ? chosenSide === "ally"
        ? livingPlayers.map((p) => ({ x: p.part.core.x, y: p.part.core.y })).filter(reachOk)
        : party.monsters.filter((m) => m.alive).map((m) => ({ x: m.x, y: m.y })).filter(reachOk)
      : monsterTurn && isDm && moverPos
        ? mSide === "ally"
          ? party.monsters
              .map((m, i) => ({ m, i }))
              .filter(({ m, i }) => m.alive && (!mChosen?.sheet.kinOnly || i === mIndex || kinOf(party, i) === kinOf(party, mIndex)))
              .map(({ m }) => ({ x: m.x, y: m.y }))
              .filter((c) => inReach("ally", mRange, cheb(moverPos, c)))
          : livingPlayers.map((p) => ({ x: p.part.core.x, y: p.part.core.y })).filter((c) => inReach("enemy", mRange, cheb(moverPos, c)))
        : [];
  // A line skill under the pointer (docs/SKILL_BUILDER.md 4.3): from the caster through the
  // hovered target to the skill's reach, a wall ends it (the module runs the program's line).
  const lineCells: Cell[] =
    combat && chosen?.line && moverPos && hover && monsterTargets.some((c) => c.x === hover.x && c.y === hover.y)
      ? (() => {
          try {
            return combat
              .line({
                from: [moverPos.x, moverPos.y],
                through: [hover.x, hover.y],
                reach: rangeReach(chosen.range),
                width: s.width,
                height: s.height,
                blocked: blockedOf(tiles, s.tileCost),
              })
              .map(([x, y]) => ({ x, y }));
          } catch {
            return [];
          }
        })()
      : [];
  const onLine = (c: Cell) => lineCells.some((l) => l.x === c.x && l.y === c.y);
  // The party's auras and the monsters' (patch 6) over every unit.
  const sessionParty = partyInputOf(s, monsterBearersOf(party));
  // Auras on the map (docs/SKILL_BUILDER.md 3.5): every living bearer's passive and temporary aura.
  const gridAuras: GridAura[] = [
    ...livingPlayers.flatMap((p) =>
      (s.auras[p.slot] ?? [])
        .filter((a) => a.turns > 0)
        .map((a) => ({
          center: { x: p.part.core.x, y: p.part.core.y },
          radius: a.selfOnly ? 0 : a.radius,
          hostile: a.hostile,
          label: auraText(a, label(p.slot)),
        })),
    ),
    // A monster's aura: friendly to its allies (its kin for a kin-only one), hostile to the players.
    ...party.monsters.flatMap((m, i) =>
      !m.alive
        ? []
        : (party.monsterAuras[i] ?? [])
            .filter((a) => a.aura.turns > 0)
            .map((a) => ({
              center: { x: m.x, y: m.y },
              radius: a.aura.selfOnly ? 0 : a.aura.radius,
              hostile: !a.aura.hostile,
              label: `${auraText(a.aura, label(MONSTER_BASE + i))}${a.kinOnly ? " (its kin)" : ""}`,
            })),
    ),
  ];
  // Before the dice (docs/COMBAT_MATH.md 13): the weapon or a damage skill on the hovered monster
  // (every monster on a hovered line), else on each monster in reach. The combat module builds the
  // hit as the program does, auras included.
  const attackPreviews =
    combat && tablesReady && activePlayer && myTurn && canAttack && chosenSide === "enemy" && (!chosen || chosen.purpose === Purpose.Damage)
      ? (() => {
          const hero = heroInputOf(activePlayer.char, activePlayer.part.equipment, activePlayer.char.core.stats, activePlayer.part.classTier);
          const heroMods = activeMods(activePlayer.part.mods);
          const accuracy = accuracyOf(activePlayer.part);
          const record = chosen ? (skillRecords[activePlayer.character]?.[chosen.slot] ?? null) : null;
          const inReach = party.monsters
            .map((m, i) => ({ m, i }))
            .filter(({ m }) => m.alive && reachOk({ x: m.x, y: m.y }));
          const hovered = lineCells.length
            ? party.monsters.map((m, i) => ({ m, i })).filter(({ m }) => m.alive && onLine(m))
            : hover
              ? inReach.filter(({ m }) => m.x === hover.x && m.y === hover.y)
              : [];
          return (hovered.length ? hovered : inReach.slice(0, 4)).flatMap(({ m, i }) => {
            const p = previewOnMonster(combat, hero, heroMods, record, m, activeMods(party.monsterMods[i]), party.monsterRecords[i] ?? new Uint8Array(48), i, accuracy, sessionParty, activePlayer.slot);
            return p ? [{ unit: MONSTER_BASE + i, text: `${previewText(p)}${tintPreviewText(p)}` }] : [];
          });
        })()
      : [];

  // Before the dice, the DM's side: the monster's natural attack or damage skill on each player in
  // reach (the combat module builds the program's hit, auras included).
  const monsterPreviews =
    combat && tablesReady && monsterTurn && isDm && canAttack && mIndex >= 0 && mSide === "enemy" && (!mChosen || mChosen.sheet.blocks[1] === Purpose.Damage)
      ? livingPlayers
          .filter((pl) => moverPos && inReach("enemy", mRange, cheb(moverPos, { x: pl.part.core.x, y: pl.part.core.y })))
          .flatMap((pl) => {
            const m = party.monsters[mIndex]!;
            try {
              const pv = combat.previewMonsterAttack({
                monster: { record: party.monsterRecords[mIndex]!, level: m.level, states: statesOf(m.states), mods: activeMods(party.monsterMods[mIndex]) },
                index: mIndex,
                at: [m.x, m.y],
                skill: mChosen?.copy ?? null,
                hero: heroInputOf(pl.char, pl.part.equipment, pl.char.core.stats, pl.part.classTier),
                heroStates: statesOf(pl.part.core.states),
                heroMods: activeMods(pl.part.mods),
                slot: pl.slot,
                party: sessionParty,
              });
              return [{ unit: pl.slot, text: previewText(pv) }];
            } catch {
              return [];
            }
          })
      : [];

  return (
    <section>
      <h1>
        {ended ? `Party ended: ${st}` : "Battle"} <LiveBadge status={live} />
      </h1>
      {header}
      <SessionKeyBox party={party} pk={pk} isDm={isDm} me={me} send={send} busy={busy} />
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
            auras={gridAuras}
            line={!ended ? lineCells : []}
            burning={party.burningCells.filter((c) => c.untilRound >= s.core.round).map((c) => ({ x: c.x, y: c.y }))}
            cellPx={CELL_PX}
            unitSprite={unitSprite}
            tileSprite={tileSprite}
            onCellHover={setHover}
            onCellClick={(c) => (!canMove ? undefined : myTurn ? movePlayer(c) : monsterTurn && isDm ? moveMonster(c) : undefined)}
            onUnitClick={(u) => {
              if (!canAttack) return;
              if (myTurn && chosenSide === "enemy" && u.kind === "monster" && u.alive) attack(u.id - MONSTER_BASE);
              else if (myTurn && chosenSide === "ally" && u.kind === "player" && u.alive) support(u.id);
              else if (monsterTurn && isDm && mSide === "enemy" && u.kind === "player" && u.alive) monsterAttack(u.id);
              else if (monsterTurn && isDm && mSide === "ally" && u.kind === "monster" && u.alive) monsterSupport(u.id - MONSTER_BASE);
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
                  ? mSide === "ally"
                    ? `Monster turn: click an allied monster (or itself) for ${mChosen?.name ?? "the skill"}, a cell to move it, or Auto.`
                    : "Monster turn: click a cell to move it, a hero to attack with the selected action (once), or Auto."
                  : ""}
          </p>
        </div>
        <div style={{ flex: "1 1 380px", display: "grid", gridTemplateColumns: "minmax(0, 1fr)", gap: 12, minWidth: 320 }}>
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
                          disabled={!canAttack || cd > 0 || moverAp < k.apCost}
                          aria-pressed={chosenSkill === k.slot}
                          title={cd > 0 ? `On cooldown: ${cooldownLabel(cd)}` : k.side === "ally" ? "Heal or buff: click an ally or yourself" : k.purpose === Purpose.Debuff ? "Debuff: click a monster" : undefined}
                          onClick={() => setSkill(k.slot)}
                        >
                          {k.name}
                          <span className="muted ap"> · {k.apCost} AP{k.line ? " · line" : ""}</span>
                          {k.side === "ally" ? <span className="muted"> · ally</span> : k.purpose === Purpose.Debuff ? <span className="muted"> · debuff</span> : null}
                          {cd > 0 && <span className="cd"> · {cooldownLabel(cd)}</span>}
                        </button>
                      );
                    })}
                    <button className="btn" disabled={!canAttack || moverAp < BASIC_ATTACK_AP} aria-pressed={chosenSkill === BASE_ATTACK} onClick={() => setSkill(BASE_ATTACK)}>
                      Weapon<span className="muted ap"> · {BASIC_ATTACK_AP} AP</span>
                    </button>
                  </div>
                  {attackPreviews.length > 0 && (
                    <ul className="attack-preview" aria-label="Attack preview">
                      {attackPreviews.map((p) => (
                        <li key={p.unit}>
                          <b>{label(p.unit)}</b>: {p.text}
                        </li>
                      ))}
                    </ul>
                  )}
                </>
              )}
              {monsterTurn && isDm && (
                <>
                  <div className="skills" aria-label="Monster actions">
                    <button className="btn" disabled={!canAttack || moverAp < BASIC_ATTACK_AP} aria-pressed={monsterSlot === null} onClick={() => setMonsterSlot(null)}>
                      Natural attack<span className="muted ap"> · {BASIC_ATTACK_AP} AP</span>
                    </button>
                    {mSkills.map((k) => {
                      const cd = mCooldowns[k.slot] ?? 0;
                      const ally = k.sheet.blocks[1] === Purpose.Buff;
                      return (
                        <button
                          key={k.slot}
                          className={cd > 0 ? "btn cooldown" : "btn"}
                          disabled={!canAttack || cd > 0 || moverAp < k.sheet.apCost}
                          aria-pressed={monsterSlot === k.slot}
                          title={cd > 0 ? `On cooldown: ${cooldownLabel(cd)}` : ally ? "Heal or buff: click an allied monster" : undefined}
                          onClick={() => setMonsterSlot(k.slot)}
                        >
                          {k.name}
                          <span className="muted ap"> · {k.sheet.apCost} AP{k.sheet.blocks[4] === 1 ? " · line" : ""}</span>
                          {ally ? <span className="muted"> · ally{k.sheet.kinOnly ? " (kin)" : ""}</span> : k.sheet.blocks[1] === Purpose.Debuff ? <span className="muted"> · debuff</span> : null}
                          {cd > 0 && <span className="cd"> · {cooldownLabel(cd)}</span>}
                        </button>
                      );
                    })}
                  </div>
                  {mAp > moverAp && <p className="muted">Not enough AP for this action.</p>}
                  {monsterPreviews.length > 0 && (
                    <ul className="attack-preview" aria-label="Monster attack preview">
                      {monsterPreviews.map((p) => (
                        <li key={p.unit}>
                          <b>{label(p.unit)}</b>: {p.text}
                        </li>
                      ))}
                    </ul>
                  )}
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
            <HitCard
              key={roll.key}
              rollKey={roll.key}
              header={`${roll.hit.attacker} → ${roll.hit.target}`}
              hit={roll.hit.result}
              critChanceBp={roll.hit.critChanceBp}
              breakdown={{ mods: roll.hit.mods, items: roll.hit.items }}
            />
          )}
          {roll?.effect && (
            <HitCard
              key={`${roll.key}:effect`}
              rollKey={`${roll.key}:effect`}
              header={roll.effect.effect.rider ? `${roll.effect.skill} effect → ${roll.effect.target}` : `${roll.effect.caster} · ${roll.effect.skill} → ${roll.effect.target}`}
              effect={roll.effect.effect}
              critChanceBp={roll.effect.critChanceBp}
            />
          )}
          <StatsPanel units={statsUnits(party, label, combat)} />
          {ended && <Claims party={party} isDm={isDm} me={me} actorFor={actorFor} payer={wallet.signer} send={send} ingest={ingest} busy={busy} />}
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

export function Claims({
  party,
  isDm,
  me,
  actorFor,
  payer,
  send,
  ingest,
  busy,
}: {
  party: Party;
  isDm: boolean;
  me: Address | null;
  actorFor: (actor: Address) => Actor | null;
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
              ) : p.part.player !== me ? (
                <span className="muted">not claimed yet</span>
              ) : (
                <button
                  className="btn"
                  disabled={busy !== null}
                  onClick={() => {
                    const actor = actorFor(p.part.core.sessionKey);
                    void send(`claim_rewards ${bytesText(p.char.name)}`, actor?.signer ?? null, async () => {
                      // Pending deaths of the party are finalized first (patch 6).
                      const deaths = await deathAccountsOf(s);
                      return withDeaths(deaths, await claimRewardsIx(actor!, party.address, p.character, deaths));
                    }).then((r) => r && ingest(r.signature, r.events as never[]));
                  }}
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
          onClick={() => void send(`claim_escrow ${formatSol(party.escrow!.collected)}`, payer, async () => {
                const deaths = await deathAccountsOf(s);
                return withDeaths(deaths, await claimEscrowIx(payer!, party.address, s.core.map, deaths));
              }).then((r) => r && ingest(r.signature, r.events as never[]))}
        >
          Claim escrow ({formatSol(party.escrow.collected)})
        </button>
      )}
      {party.escrow?.claimed && <p className="ok">The DM took {formatSol(party.escrow.collected)} from the escrow; the map is finished.</p>}
    </div>
  );
}

/**
 * This browser's session key for the party (docs/GAME_COMMISSIONS.md 6.2, 8): top up while
 * playing, replace a lost or expired one, and leave (revoke + refund) when the party is over. Each
 * is one main-wallet transaction.
 */
function SessionKeyBox({ party, pk, isDm, me, send, busy }: { party: Party; pk: PartyKey; isDm: boolean; me: Address | null; send: ReturnType<typeof useTx>["send"]; busy: string | null }) {
  const wallet = useAppWallet();
  const signer = wallet.signer;
  const s = party.session;
  const st = status(s);
  const ended = st !== "lobby" && st !== "active";
  const mine = party.players.filter((p) => p.part.player === me);
  // The keys the party has for my units: my participations' and, for the DM, the monsters'. A
  // participation has none until Ready binds it.
  const boundMine = mine.filter((p) => p.part.core.sessionKey !== DEFAULT_KEY);
  const boundKeys = [...boundMine.map((p) => p.part.core.sessionKey), ...(isDm && s.dmActor !== DEFAULT_KEY ? [s.dmActor] : [])];
  if (!signer || (boundKeys.length === 0 && !pk.signer)) return null;
  const bound = !!pk.signer && boundKeys.length > 0 && boundKeys.every((k) => k === pk.signer!.address);
  const playing = bound && pk.live;
  const room = pk.balance === null ? 0n : topUpRoom(pk.balance);
  const topUpLamports = room < FEES.prepay ? room : FEES.prepay;

  const topUp = () =>
    void send(`top up the session key ${formatSol(topUpLamports)}`, signer, async () => {
      const ix = pk.topUpIx(signer, topUpLamports);
      return ix ? [ix] : [];
    });
  const replace = async () => {
    const next = await prepareKey(signer);
    const res = await send("new session key", signer, async () => {
      // This browser's old key gives back what it holds; a key only another browser has keeps
      // its SOL, but its token is revoked for the rent.
      const ixs = await pk.returnIxs(signer);
      for (const k of new Set(boundKeys)) {
        if (k === pk.signer?.address) continue;
        const token = await sessionTokenPda(PROGRAMS.session, k, signer.address);
        if (await fetchSessionToken(token)) ixs.push(await revokeSessionInstruction(token, signer));
      }
      ixs.push(next.instruction);
      if (isDm) ixs.push(await setSessionKeyIx(signer, party.address, null, next.signer.address));
      // Participations not Ready yet get their key at Ready.
      for (const p of boundMine) ixs.push(await setSessionKeyIx(signer, party.address, p.character, next.signer.address));
      return ixs;
    });
    if (res) pk.replaced(next);
  };
  const leave = async () => {
    const res = await send("leave: revoke the session key and return its SOL", signer, () => pk.returnIxs(signer));
    if (res) pk.replaced(null);
  };

  return (
    <PartyKeyPanel
      keyAddress={pk.signer?.address ?? null}
      bound={bound}
      live={pk.live}
      balance={pk.balance}
      low={pk.low}
      lowLamports={FEES.lowBalance}
      secondsLeft={pk.validUntil === null ? null : pk.validUntil - Math.floor(Date.now() / 1000)}
      ended={ended}
      busy={busy !== null}
      onTopUp={playing && !ended && topUpLamports > 0n ? topUp : undefined}
      topUpLamports={topUpLamports}
      replaceLamports={FEES.prepay}
      onReplace={!ended && !playing && boundKeys.length > 0 ? () => void replace() : undefined}
      onLeave={pk.signer && (ended || !bound) ? () => void leave() : undefined}
    />
  );
}

type SkillName = (unit: number, slot: number) => string;

const TRIGGER_TEXT: Record<string, string> = {
  hit: "hit",
  critHit: "critical hit",
  hpBelow: "HP below its threshold",
  allyHpBelow: "an ally's HP below its threshold",
  controlled: "controlled",
  enemyAdjacent: "an enemy next to it",
};
const triggerText = (t: number) => TRIGGER_TEXT[variantKey(Trigger, t)] ?? (variantKey(Trigger, t) || "?");

/** "Mage: passive Protection aura 25%, radius 2" (a temporary one with its turns left). */
function auraText(a: AuraAcc, bearer: string): string {
  const kind = variantKey(ModKindData, a.kind) || "?";
  const reach = a.selfOnly || a.radius === 0 ? "self" : `radius ${a.radius}`;
  const time = a.passive ? "passive" : `temporary, ${a.turns} turns`;
  return `${bearer}: ${a.hostile ? "hostile " : ""}${kind.charAt(0).toUpperCase()}${kind.slice(1)} aura ${percent(a.powerBp)}, ${reach} (${time})`;
}

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
      const hit = toHitResolved(d).result;
      const res = hit.resonances
        .filter((x) => x?.active)
        .map((x) => `${resonanceName(x!.id, x!.secondTint)} (${carrierName(x!.carrier).toLowerCase()})`);
      return `${label(d.attacker)} -> ${label(d.target)}: d20 ${hit.d20}${hit.crit ? " CRIT" : ""}, ${variantKey(HitOutcome, d.result.outcome)}, ${hit.damage} dmg${res.length ? `, ${res.join(", ")}` : ""}`;
    }
    case "ResonanceSpread":
      return spreadText(toResonanceSpread(d), label);
    case "CellBurned":
      return `${label(d.unit)} stepped into fire at ${d.x},${d.y}: Burning`;
    case "MonsterKilled":
      return `${label(MONSTER_BASE + d.monster)} killed${d.boss ? " (boss)" : ""}, +${d.xp} xp`;
    case "PlayerDied":
      return `${label(d.slot)} died (death pending: the next start_turn finalizes it)`;
    case "DeathFinalized":
      return `${label(d.slot)}: death finalized`;
    case "CharacterDied":
      return `character ${shortAddress(d.character)} is a tombstone`;
    case "SessionEnded":
      return `party ended: ${variantKey(SessionStatus, d.status)}`;
    case "RewardsClaimed":
      return `rewards claimed: ${d.xp} xp`;
    case "EscrowClaimed":
      return `escrow claimed by the DM: ${formatSol(BigInt(d.amount))}`;
    case "FleeVoted":
      return `slot ${d.slot} votes to flee`;
    case "ReactiveTriggered":
      return `${label(d.bearer)} reacts (${triggerText(d.trigger)}): ${skillName(d.bearer, d.skillSlot)}, ${variantKey(ReactionKind, d.reaction)} on ${label(d.target)}, cooldown ${d.cooldownLeft}`;
    case "AuraChanged":
      return d.aura.turns === 0 ? `${label(d.bearer)}: aura ended` : auraText(d.aura, label(d.bearer));
    case "MonsterReactiveTriggered":
      return `${label(MONSTER_BASE + d.bearer)} reacts (${triggerText(d.trigger)}): ${skillName(MONSTER_BASE + d.bearer, d.skillSlot)}, ${variantKey(ReactionKind, d.reaction)} on ${label(d.target)}, cooldown ${d.cooldownLeft}`;
    case "MonsterAuraChanged":
      return d.aura.turns === 0 ? `${label(MONSTER_BASE + d.bearer)}: aura ended` : `${auraText(d.aura, label(MONSTER_BASE + d.bearer))}${d.kinOnly ? " (its kin)" : ""}`;
    default:
      return e.name;
  }
}

/**
 * Combat stats of every living unit with its buffs and debuffs (Participation.mods,
 * MonsterState.mods): HP, AP per turn (Haste / Slow), defense, armor, resistances, vulnerabilities.
 */
function statsUnits(party: Party, label: (u: number) => string, combat: CombatWasm | null): StatsUnit[] {
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
        profile: playerDefense(p.char.core.class as Class, p.char.core.stats, p.part.equipment),
        mods: activeMods(p.part.mods),
        states: statesOf(p.part.core.states),
      });
      return { unit: p.slot, label: label(p.slot), kind: "player", current: current === p.slot, stats };
    });
  const monsters = party.monsters.flatMap((m, i): StatsUnit[] => {
    // Defense, armor and resistances: the combat module over the bestiary record the session copied.
    const record = party.monsterRecords[i];
    const profile = combat && record ? monsterDefense(combat, record, m.level, m.flags) : null;
    if (!m.alive || !profile) return [];
    const u = MONSTER_BASE + i;
    const stats = unitStats({
      hp: m.hp,
      hpMax: m.hpMax,
      apBase: monsterBaseAp(s.monsterSpeed[i] ?? m.actionPoints),
      apNow: current === u && running ? m.actionPoints : null,
      profile,
      mods: activeMods(party.monsterMods[i]),
      states: statesOf(m.states),
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

/** The player's resonance accuracy buff (`Participation.accuracy`). */
function accuracyOf(part: { accuracy?: { aspect: number; turns: number; bonus: number } }): AccuracyInput {
  const a = part.accuracy;
  return a && a.turns > 0 ? { aspect: a.aspect, turns: a.turns, bonus: a.bonus } : { aspect: 0, turns: 0, bonus: 0 };
}

/** " · Piercing Frost 7%: 5-9 (weapon)" for each tint that can trigger on this target. */
function tintPreviewText(p: HitPreview): string {
  const live = p.tints.filter((t) => !t.pure && t.conditionMet);
  if (live.length === 0) return "";
  return live
    .map((t) => ` · ${resonanceName(t.id)} ${t.chancePct}%${t.min !== p.min || t.max !== p.max ? `: ${t.min}-${t.max}` : ""} (${carrierName(t.carrier as never).toLowerCase()})`)
    .join("");
}

/** What a resonance did beyond its target: jumps, a knockback, burning cells, an accuracy buff. */
function spreadText(e: ReturnType<typeof toResonanceSpread>, label: (u: number) => string): string | null {
  const parts: string[] = [];
  for (const h of e.hits) parts.push(`${stateName(h.kind)} jumped to ${label(MONSTER_BASE + h.monster)}`);
  if (e.moved) parts.push(`${label(e.target)} knocked back to ${e.moved.to.x},${e.moved.to.y}`);
  if (e.ignited.length) parts.push(`${e.ignited.length} cells on fire (Burning ${e.ignitePower}) until round ${e.untilRound}`);
  if (e.accuracy.turns > 0) parts.push(`${label(e.attacker)} +${e.accuracy.bonus} accuracy to ${aspectName(e.accuracy.aspect)} attacks`);
  return parts.length ? `resonance: ${parts.join(", ")}` : null;
}
