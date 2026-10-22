// One character in full: sprite, identity, stats, HP, skills, party and its items: what is
// worn by slot, the soulbound items, Wear / Take off (the owner's wallet or session key).
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { Link, useParams } from "react-router-dom";
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
import { spendStatPointsIx } from "../chain/game";
import { equipmentOf } from "../chain/items";
import { HeroSheetPanel } from "../components/stats/HeroSheetPanel";
import type { HeroSheetPanelProps } from "../components/stats/types";
import { Inventory } from "../components/items/Inventory";
import type { InventoryItemView, InventoryProps } from "../components/items/types";
import { useAppWallet } from "../wallet/AppWallet";
import { aspectName, className, formatBp, raceName, STAT_NAMES } from "../chain/names";
import { skillSetPda } from "../chain/pda";
import { Aspect, Class, Race } from "../chain/types";
import { DATA_SOURCE, explorerUrl } from "../config";
import { OldCharacterLabel } from "../raster/OldCharacter";
import { useRenderImage } from "../raster/RasterizerProvider";
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

export function CharacterDetailPage() {
  const { address } = useParams<{ address: string }>();
  const [c, setC] = useState<CharacterAcc | null | "missing">(null);
  const [skills, setSkills] = useState<SkillRecordAcc[]>([]);
  const [party, setParty] = useState<PartyInfo>(null);
  const [error, setError] = useState<string | null>(null);
  const wallet = useAppWallet();
  const { send, busy } = useTx();
  const combat = useCombat();
  // Level-up points placed but not saved yet, by Stat; Save sends them all at once.
  const [plan, setPlan] = useState<number[]>([0, 0, 0, 0]);

  const load = useCallback(async () => {
    if (DATA_SOURCE !== "chain" || !address) return;
    const addr = address as Address;
    try {
      const [acc, set] = await Promise.all([
        getAccount("Character", addr),
        getAccount("SkillSet", await skillSetPda(addr)),
      ]);
      setC(acc ?? "missing");
      setPlan([0, 0, 0, 0]);
      setSkills(set ? set.skills.slice(0, set.count) : []);
      const session = acc ? optional(acc.core.inSession) : null;
      if (session) {
        const p = await readParty(session);
        const me = p?.players.find((x) => x.character === addr);
        setParty(p && me ? { session, status: status(p.session), slot: me.slot, hp: me.part.core.hp } : null);
      } else {
        setParty(null);
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

  if (DATA_SOURCE !== "chain") return <p className="muted">Character details need chain mode.</p>;
  return (
    <section>
      <p>
        <Link to="/character">← My characters</Link>{" "}
        <button type="button" onClick={() => void load()}>
          Refresh
        </button>
      </p>
      {error && <p className="error">{error}</p>}
      {c === null && !error && <p className="muted">Loading…</p>}
      {c === "missing" && <p className="error">No character at this address.</p>}
      {c && c !== "missing" && <CharacterDetailView address={address as Address} c={c} skills={skills} party={party} items={items} sheet={sheet} />}
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
}: {
  address: Address;
  c: CharacterAcc;
  skills: SkillRecordAcc[];
  party: PartyInfo;
  /** The inventory panel; null while it loads. */
  items?: Omit<InventoryProps, "icon"> | null;
  /** Derived stats and the level-up point plan; null: the core stats only. */
  sheet?: HeroSheetPanelProps | null;
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
            {party.status} · slot {party.slot} · HP {party.hp} · <Link to={`/party?s=${party.session}`}>open party</Link>
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
      </section>

      <section className="panel char-detail-wide">
        <h2>Inventory</h2>
        {items ? <Inventory {...items} icon={itemIcon} /> : <p className="muted">Loading items…</p>}
      </section>
    </div>
  );
}
