// One character in full: sprite, identity, stats, HP, skills, party and its items: what is
// worn by slot, the soulbound items, Wear / Take off (the owner's wallet or session key). Patch 11:
// the bag next to the settled cache, laid out freely and sent once with "Done".
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import type { Address } from "@solana/kit";
import { isCurrentAppearance } from "../chain/appearance";
import { Category, Effect, Form, Purpose, Source, Target, Trigger } from "@deadroll/sdk/character";
import {
  bytesText,
  getAccount,
  optional,
  readParty,
  status,
  variantKey,
  variantLabel,
  type CharacterAcc,
  type GearAcc,
  type SkillRecordAcc,
} from "../chain/game";
import {
  EQUIP_SLOTS,
  bonusText,
  equipIx,
  inventory,
  itemLabel,
  characterRequest,
  itemRequest,
  slotsFor,
  unequipIx,
} from "../chain/items";
import { useTx } from "../chain/useTx";
import { heroInputOf, useCombat } from "../chain/combatWasm";
import { CACHE_STATUS, closeCacheIx, leavePartyIx, spendStatPointsIx } from "../chain/game";
import { BagPanel } from "../components/bag/BagPanel";
import {
  checkBagLayout,
  discard,
  layoutArgs,
  loadLocks,
  moveSlot,
  replace,
  saveLocks,
  slotKey,
  split,
  take,
  type Bag,
  type BagState,
} from "../chain/bagLayout";
import { rewardLabel, type Reward } from "../chain/loot";
import { shortAddress } from "../components/forms/format";
import { equipmentOf } from "../chain/items";
import { HeroSheetPanel } from "../components/stats/HeroSheetPanel";
import type { HeroSheetPanelProps } from "../components/stats/types";
import { Inventory } from "../components/items/Inventory";
import type { InventoryItemView, InventoryProps } from "../components/items/types";
import { useAppWallet } from "../wallet/AppWallet";
import { aspectName, className, formatBp, raceName, STAT_NAMES } from "../chain/names";
import { bagPda, rewardCachePda, skillSetPda } from "../chain/pda";
import { Aspect, Class, Race } from "../chain/types";
import { DATA_SOURCE, explorerUrl } from "../config";
import { OldCharacterLabel } from "../raster/OldCharacter";
import { useRenderImage } from "../raster/RasterizerProvider";
import { useSprites } from "../raster/useSprites";
import { iconRequest } from "../chain/icons";
import { PREVIEW_SCALE, SpriteCanvas, usePreviewFit } from "../raster/SpriteCanvas";

/** Where this character is right now, read from the party it is in. */
export type PartyInfo = { session: Address; status: string; slot: number; hp: number } | null;

/** Equipment slots (`EquipSlot` from the character IDL, docs/ART_DESCRIPTION.md 6.3). */
export const EQUIPMENT_SLOTS = EQUIP_SLOTS.map((key) => ({ key, label: itemLabel(key) }));

/** The character's gear as the inventory panel shows it. */
export function inventoryView(g: GearAcc): InventoryItemView[] {
  return inventory(g).items.map((i) => ({
    index: i.index,
    base: i.base,
    label: itemLabel(i.base),
    level: i.level,
    bonus: bonusText(i.base),
    fits: slotsFor(i.base),
    worn: i.worn,
  }));
}

/** The panel's state for a character: who may change it, and when not. */
export function inventoryLock(c: CharacterAcc, viewer: Address | null): string | null {
  if (!c.core.alive) return "A tombstone: its soulbound items vanished with it.";
  if (viewer !== c.core.owner) return "Only the owner can change the equipment.";
  if (optional(c.core.inSession)) return "In a party: the equipment is locked until it ends.";
  return null;
}

/** Why level-up points cannot be spent from this screen (null = they can): the owner's main
 * wallet only, a living character outside a party (programs/character spend_stat_points). */
export function spendLock(c: CharacterAcc, viewer: Address | null): string | null {
  if (!c.core.alive) return "A tombstone spends nothing.";
  if (viewer !== c.core.owner) return "Only the owner's wallet can spend them.";
  if (optional(c.core.inSession)) return "In a party: spend them after it ends.";
  return null;
}

/** An item's own sprite from the on-chain sprite book. */
function ItemIcon({ base }: { base: string }) {
  const img = useRenderImage(itemRequest(base));
  return <SpriteCanvas image={img.image} scale={1} label={itemLabel(base)} />;
}
const itemIcon = (base: string): ReactNode => <ItemIcon base={base} />;

/** Range by form (programs/session rules.rs): Strike 1, Projectile 6. */
const RANGE: Record<string, number> = { strike: 1, projectile: 6 };

/** "Cold 65% · Fire 35%" */
export function skillAspects(s: SkillRecordAcc): string {
  return s.blocks.aspects
    .slice(0, s.blocks.aspectCount)
    .map((a, i) => `${aspectName(a as Aspect)} ${formatBp(s.sharesBp[i] ?? 0)}`)
    .join(" · ");
}

/** The session `act` instruction takes every Active skill (a buff or debuff needs its effect). */
export const usableInCombat = (s: SkillRecordAcc): boolean =>
  s.blocks.category === Category.Active && (s.blocks.purpose === Purpose.Damage || s.blocks.effect !== Effect.None);

/**
 * One character (the characters window), or only its bag (the side panel's bag). `onOpenParty`
 * goes to the dungeon of the party it is in.
 */
export function CharacterScreen({ address, only, onOpenParty }: { address: Address; only?: "bag"; onOpenParty?: (session: Address) => void }) {
  const [c, setC] = useState<CharacterAcc | null | "missing">(null);
  const [skills, setSkills] = useState<SkillRecordAcc[]>([]);
  const [party, setParty] = useState<PartyInfo>(null);
  const [error, setError] = useState<string | null>(null);
  const wallet = useAppWallet();
  const { send, busy } = useTx();
  const combat = useCombat();
  // Level-up points placed but not saved yet, by Stat; Save sends them all at once.
  const [plan, setPlan] = useState<number[]>([0, 0, 0, 0]);
  // Patch 11: the bag on chain, the settled caches by party (one is laid out at a time), the
  // parties still waiting for their settlement, and the layout in progress.
  const [bagBase, setBagBase] = useState<BagState | null>(null);
  const [claimed, setClaimed] = useState<{ session: Address; xp: number; slots: Reward[] }[]>([]);
  const [waiting, setWaiting] = useState<Address[]>([]);
  const [chosen, setChosen] = useState<Address | null>(null);
  const [endedLock, setEndedLock] = useState<Address | null>(null);
  const cacheBase = claimed.find((x) => x.session === chosen)?.slots ?? null;
  const [work, setWork] = useState<BagState | null>(null);
  const draw = useSprites();
  const [picked, setPicked] = useState<number | null>(null);
  const [locks, setLocks] = useState<Set<string>>(() => (address ? loadLocks(address) : new Set()));

  const load = useCallback(async () => {
    if (DATA_SOURCE !== "chain" || !address) return;
    const addr = address as Address;
    try {
      const [acc, set, bag, cache] = await Promise.all([
        getAccount("Character", addr),
        getAccount("SkillSet", await skillSetPda(addr)),
        getAccount("Bag", await bagPda(addr)),
        getAccount("RewardCache", await rewardCachePda(addr)),
      ]);
      const plain = (x: { kind: number; id: number; qty: number }): Reward => ({ kind: x.kind, id: x.id, qty: x.qty });
      const base: Bag = bag ? { resources: bag.layout.resources.map(plain), consumables: bag.layout.consumables.map(plain) } : { resources: [], consumables: [] };
      const entries = cache?.entries ?? [];
      const done = entries
        .filter((e) => e.status === CACHE_STATUS.Claimed)
        .map((e) => ({ session: e.session, xp: e.xp, slots: e.slots.map(plain).filter((x) => x.qty > 0) }));
      const first = done[0] ?? null;
      setClaimed(done);
      setWaiting(entries.filter((e) => e.status === CACHE_STATUS.Open).map((e) => e.session));
      setChosen(first?.session ?? null);
      setBagBase({ bag: base, cache: first?.slots ?? [] });
      setWork({ bag: base, cache: first?.slots ?? [] });
      setPicked(null);
      setC(acc ?? "missing");
      setPlan([0, 0, 0, 0]);
      setSkills(set ? set.skills.slice(0, set.count) : []);
      const session = acc ? optional(acc.core.inSession) : null;
      if (session) {
        const p = await readParty(session);
        const me = p?.players.find((x) => x.character === addr);
        setParty(p && me ? { session, status: status(p.session), slot: me.slot, hp: me.part.core.hp } : null);
        // Patch 11: locked to a party that ended: free it (`leave_party`) before "Done".
        setEndedLock(p && acc?.core.alive && ["victory", "fled", "wiped"].includes(status(p.session)) ? session : null);
      } else {
        setParty(null);
        setEndedLock(null);
      }
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [address]);
  useEffect(() => void load(), [load]);

  // Wear / Take off: outside a party, so the owner's wallet (a session key lives for one party only).
  const actor = { signer: wallet.signer, token: null };
  const act = async (what: string, build: () => Promise<Awaited<ReturnType<typeof equipIx>>>) => {
    if (await send(what, actor.signer, async () => [await build()])) await load();
  };
  const addr = address as Address;
  const items: Omit<InventoryProps, "icon"> | null =
    c && c !== "missing"
      ? {
          slots: EQUIPMENT_SLOTS,
          items: inventoryView(c.gear),
          locked: inventoryLock(c, wallet.address ?? null),
          busy: busy !== null,
          onEquip: (index, slot) => void act(`Wear in ${itemLabel(slot)}`, () => equipIx(actor.signer!, addr, index, slot, actor.token)),
          onUnequip: (slot) => void act(`Take off ${itemLabel(slot)}`, () => unequipIx(actor.signer!, addr, slot, actor.token)),
        }
      : null;
  // Derived stats from the combat module (the rules the programs run), now and with the plan.
  const sheet: HeroSheetPanelProps | null =
    c && c !== "missing"
      ? (() => {
          const equipment = equipmentOf(c.gear);
          const plannedStats = c.core.stats.map((s, i) => s + (plan[i] ?? 0));
          return {
            stats: c.core.stats,
            plan,
            unspent: c.unspentPoints,
            sheet: combat ? combat.heroSheet(heroInputOf(c, equipment)) : null,
            planned: combat ? combat.heroSheet(heroInputOf(c, equipment, plannedStats)) : null,
            locked: spendLock(c, wallet.address ?? null),
            busy: busy !== null,
            onPlan: (stat, delta) =>
              setPlan((p) => {
                const left = c.unspentPoints - p.reduce((a, b) => a + b, 0);
                const next = [...p];
                next[stat] = Math.max(0, (next[stat] ?? 0) + (delta > 0 && left === 0 ? 0 : delta));
                return next;
              }),
            onReset: () => setPlan([0, 0, 0, 0]),
            // The owner's main wallet: spending points is not a game move (no session key).
            onSave: () =>
              void send("Spend stat points", wallet.signer, async () => [spendStatPointsIx(wallet.signer!, addr, plan)]).then((ok) => ok && load()),
          };
        })()
      : null;

  const owner = c && c !== "missing" && wallet.address === c.core.owner;
  const bagError = useMemo(() => (work && bagBase ? checkBagLayout(bagBase.bag, cacheBase ?? [], work.bag) : null), [work, bagBase, cacheBase]);
  const apply = (next: BagState | null) => next && setWork(next);
  const bagPanel =
    work && c && c !== "missing" ? (
      <BagPanel
        bag={work.bag}
        cache={work.cache}
        editable={!!owner && cacheBase !== null}
        locks={locks}
        picked={picked}
        error={bagError ? `The chain would refuse this layout (${bagError})` : null}
        busy={busy !== null}
        label={rewardLabel}
        icon={(r) => {
          const req = iconRequest(r);
          return req ? draw(`i:${r.kind}:${r.id}`, req) : null;
        }}
        onPick={setPicked}
        onTakeAll={(i) => apply(take(work, i, null, undefined, locks))}
        onPlace={(i, to) => apply(replace(work, i, to, locks))}
        onMove={(from, to) => apply(moveSlot(work, from, to))}
        onSplit={(at) => apply(split(work, at))}
        onDiscard={(at) => apply(discard(work, at, locks))}
        onToggleLock={(at) =>
          setLocks((l) => {
            const next = new Set(l);
            const key = slotKey(at);
            if (next.has(key)) next.delete(key);
            else next.add(key);
            saveLocks(addr, next);
            return next;
          })
        }
        onReset={() => bagBase && setWork(bagBase)}
        // "Done": the owner's main wallet; the rest of this party's cache burns. A character still
        // locked to its ended party is freed first, in the same transaction.
        onDone={() =>
          void send("Done (close_cache)", wallet.signer, async () => [
            ...(endedLock ? [await leavePartyIx(wallet.signer!, endedLock, addr)] : []),
            await closeCacheIx(wallet.signer!, addr, chosen!, layoutArgs(work.bag)),
          ]).then((ok) => ok && load())
        }
      />
    ) : null;
  const choose = (session: Address) => {
    const slots = claimed.find((x) => x.session === session)?.slots ?? [];
    setChosen(session);
    if (bagBase) {
      setBagBase({ ...bagBase, cache: slots });
      setWork({ bag: bagBase.bag, cache: slots });
    }
    setPicked(null);
  };
  const bagSection = (
    <>
      {claimed.length > 1 && (
        <p>
          Cache of the party{" "}
          <select value={chosen ?? ""} onChange={(e) => choose(e.target.value as Address)}>
            {claimed.map((x) => (
              <option key={x.session} value={x.session}>
                {shortAddress(x.session)} · {x.xp} xp
              </option>
            ))}
          </select>
        </p>
      )}
      {waiting.length > 0 && (
        <p className="muted">
          {waiting.length} {waiting.length === 1 ? "party waits" : "parties wait"} for the DM to hand out the rewards (they do not burn before the DM's deadline).
        </p>
      )}
      {endedLock && owner && (
        <p>
          <button
            type="button"
            className="btn"
            disabled={busy !== null}
            onClick={() => void send("leave_party", wallet.signer, async () => [await leavePartyIx(wallet.signer!, endedLock, addr)]).then((ok) => ok && load())}
          >
            Leave the ended party
          </button>{" "}
          <span className="muted">frees the character for gear, points and a new party; its rewards still wait.</span>
        </p>
      )}
      {bagPanel}
    </>
  );

  if (DATA_SOURCE !== "chain") return <p className="muted">Character details need chain mode.</p>;
  if (only === "bag") {
    return (
      <section>
        {error && <p className="error">{error}</p>}
        {c === null && !error ? <p className="muted">Loading…</p> : bagSection}
      </section>
    );
  }
  return (
    <section>
      <p>
        <button type="button" className="btn" onClick={() => void load()}>
          Refresh
        </button>
      </p>
      {error && <p className="error">{error}</p>}
      {c === null && !error && <p className="muted">Loading…</p>}
      {c === "missing" && <p className="error">No character at this address.</p>}
      {c && c !== "missing" && (
        <CharacterDetailView address={address} c={c} skills={skills} party={party} items={items} sheet={sheet} bag={bagSection} onOpenParty={onOpenParty} />
      )}
    </section>
  );
}

export function CharacterDetailView({
  address,
  c,
  skills,
  party,
  items = null,
  sheet = null,
  bag = null,
  onOpenParty,
}: {
  address: Address;
  c: CharacterAcc;
  skills: SkillRecordAcc[];
  party: PartyInfo;
  /** The inventory panel; null while it loads. */
  items?: Omit<InventoryProps, "icon"> | null;
  /** Derived stats and the level-up point plan; null: the core stats only. */
  sheet?: HeroSheetPanelProps | null;
  /** Patch 11: the bag screen (with the settled cache); null while it loads. */
  bag?: ReactNode;
  onOpenParty?: (session: Address) => void;
}) {
  const dead = !c.core.alive;
  const current = isCurrentAppearance(c.core.appearance);
  const img = useRenderImage(current ? characterRequest(c) : null);
  const previewFit = usePreviewFit();
  const race = c.core.race as Race;
  const cls = c.core.class as Class;
  const pred = c.core.predisposition as Aspect;
  const nft = optional(c.core.nft);
  const name = bytesText(c.name);
  const hp = dead ? `0 / ${c.core.hpMax}` : party ? `${party.hp} / ${c.core.hpMax}` : `${c.core.hpMax} (max)`;

  return (
    <div className="char-detail">
      <header className={`panel char-detail-head${dead ? " dead" : ""}`}>
        {current ? <SpriteCanvas image={img.image} scale={PREVIEW_SCALE} fit={previewFit} label={name} /> : <OldCharacterLabel />}
        <div>
          <h1>{name}</h1>
          <p>
            {raceName(race)} {className(cls)} · {aspectName(pred)} predisposition
          </p>
          <p className="muted">
            Trait tiers (1 best): race {c.core.predispositionTier} · class {c.classTier}
          </p>
          <p className="muted">
            Level {c.core.level} · {c.core.xp} XP{c.unspentPoints > 0 ? ` · ${c.unspentPoints} unspent points` : ""}
          </p>
          <p className={dead ? "error" : "ok"}>
            {dead ? `Tombstone${c.nftFrozen ? " · NFT frozen" : " · freeze pending"}` : party ? "Alive · in a party" : "Alive"}
          </p>
          <p className="mono">
            <a href={explorerUrl("address", address)} target="_blank" rel="noreferrer">
              PDA
            </a>
            {nft && (
              <>
                {" · "}
                <a href={explorerUrl("address", nft)} target="_blank" rel="noreferrer">
                  Core NFT
                </a>
              </>
            )}
          </p>
        </div>
      </header>

      <section className="panel char-detail-stats">
        <h2>Stats</h2>
        <p className="muted">HP {hp}</p>
        {sheet ? (
          <HeroSheetPanel {...sheet} />
        ) : (
          <dl className="char-stats">
            {STAT_NAMES.map((s, i) => (
              <div key={s}>
                <dt>{s}</dt>
                <dd>{c.core.stats[i] ?? 0}</dd>
              </div>
            ))}
          </dl>
        )}
      </section>

      <section className="panel char-detail-party">
        <h2>Current party</h2>
        {party ? (
          <p>
            {party.status} · slot {party.slot} · HP {party.hp}
            {onOpenParty && (
              <>
                {" · "}
                <button type="button" className="btn" onClick={() => onOpenParty(party.session)}>
                  Go to the dungeon
                </button>
              </>
            )}
          </p>
        ) : optional(c.core.inSession) ? (
          <p className="muted">
            In party <span className="mono">{optional(c.core.inSession)}</span>, details unavailable.
          </p>
        ) : (
          <p className="muted">Not in a party.</p>
        )}
      </section>

      <section className="panel char-detail-wide">
        <h2>Skills</h2>
        <SkillsTable skills={skills} />
      </section>

      <section className="panel char-detail-wide">
        <h2>Bag</h2>
        {bag ?? <p className="muted">Loading the bag…</p>}
      </section>

      <section className="panel char-detail-wide">
        <h2>Inventory</h2>
        {items ? <Inventory {...items} icon={itemIcon} /> : <p className="muted">Loading items…</p>}
      </section>
    </div>
  );
}

/** A character's skills (the character window and the side panel's skills). */
export function SkillsTable({ skills }: { skills: SkillRecordAcc[] }) {
  return (
    <>
    {skills.length === 0 ? (
      <p className="muted">No skills.</p>
    ) : (
      <div className="table-scroll">
        <table className="char-skills">
          <thead>
            <tr>
              <th>Name</th>
              <th>Kind</th>
              <th>Aspects</th>
              <th>Form</th>
              <th>Target</th>
              <th>Power</th>
              <th>Effect</th>
              <th>Trigger</th>
              <th>Cooldown</th>
            </tr>
          </thead>
          <tbody>
            {skills.map((s, i) => (
              <tr key={i}>
                <td>
                  {bytesText(s.name)}
                  {!usableInCombat(s) && <span className="muted"> · not usable in combat</span>}
                </td>
                <td>
                  {variantLabel(Category, s.blocks.category)} {variantLabel(Purpose, s.blocks.purpose).toLowerCase()} · {variantLabel(Source, s.blocks.source)}
                </td>
                <td>{skillAspects(s)}</td>
                <td>
                  {variantLabel(Form, s.blocks.form)} (range {RANGE[variantKey(Form, s.blocks.form)] ?? "?"})
                </td>
                <td>
                  {variantLabel(Target, s.blocks.target)}
                  {s.blocks.auraRadius > 0 ? ` · aura ${s.blocks.auraRadius}` : ""}
                </td>
                <td>{formatBp(s.powerBp)}</td>
                <td>{variantLabel(Effect, s.blocks.effect)}</td>
                <td>{variantLabel(Trigger, s.blocks.trigger)}</td>
                <td>{s.cooldown}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    )}
    </>
  );
}
