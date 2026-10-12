// One character in full: sprite, identity, stats, HP, skills, party and (empty) equipment.
import { useCallback, useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import type { Address } from "@solana/kit";
import { renderRequest, TOMBSTONE } from "../chain/appearance";
import {
  ASPECT_KEYS,
  CLASS_KEYS,
  RACE_KEYS,
  bytesText,
  enumIndex,
  getAccount,
  readParty,
  status,
  variantKey,
  variantLabel,
  type CharacterAcc,
  type SkillRecordAcc,
  type SkillSetAcc,
} from "../chain/game";
import { aspectName, className, formatBp, raceName, STAT_NAMES } from "../chain/names";
import { skillSetPda } from "../chain/pda";
import { Aspect, Class, Race } from "../chain/types";
import { DATA_SOURCE, explorerUrl } from "../config";
import { useRenderImage } from "../raster/RasterizerProvider";
import { SpriteCanvas } from "../raster/SpriteCanvas";

/** Where this character is right now, read from the party it is in. */
export type PartyInfo = { session: Address; status: string; slot: number; hp: number } | null;

/** Equipment slots from docs/ART_DESCRIPTION.md 6.3. */
export const EQUIPMENT_SLOTS = [
  "Primary weapon",
  "Shield / second weapon",
  "Helmet",
  "Torso armor",
  "Pants",
  "Boots",
  "Gloves",
  "Necklace",
  "Cloak",
  "Rings",
] as const;

/** Range by form (programs/session rules.rs): Strike 1, Projectile 6. */
const RANGE: Record<string, number> = { strike: 1, projectile: 6 };

/** "Cold 65% · Fire 35%" */
export function skillAspects(s: SkillRecordAcc): string {
  return s.blocks.aspects
    .slice(0, s.blocks.aspectCount)
    .map((a, i) => `${aspectName(enumIndex(a, ASPECT_KEYS) as Aspect)} ${formatBp(s.sharesBp[i] ?? 0)}`)
    .join(" · ");
}

/** The session `act` instruction accepts only Active + Damage skills today. */
export const usableInCombat = (s: SkillRecordAcc): boolean =>
  variantKey(s.blocks.category) === "active" && variantKey(s.blocks.purpose) === "damage";

export function CharacterDetailPage() {
  const { address } = useParams<{ address: string }>();
  const [c, setC] = useState<CharacterAcc | null | "missing">(null);
  const [skills, setSkills] = useState<SkillRecordAcc[]>([]);
  const [party, setParty] = useState<PartyInfo>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (DATA_SOURCE !== "chain" || !address) return;
    const addr = address as Address;
    try {
      const [acc, set] = await Promise.all([
        getAccount<CharacterAcc>("character", "Character", addr),
        getAccount<SkillSetAcc>("character", "SkillSet", await skillSetPda(addr)),
      ]);
      setC(acc ?? "missing");
      setSkills(set ? set.skills.slice(0, set.count) : []);
      const session = acc?.core.inSession ?? null;
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
      {c && c !== "missing" && <CharacterDetailView address={address as Address} c={c} skills={skills} party={party} />}
    </section>
  );
}

export function CharacterDetailView({ address, c, skills, party }: { address: Address; c: CharacterAcc; skills: SkillRecordAcc[]; party: PartyInfo }) {
  const dead = !c.core.alive;
  const img = useRenderImage(dead ? TOMBSTONE : renderRequest(c.core.appearance, false));
  const race = enumIndex(c.core.race, RACE_KEYS) as Race;
  const cls = enumIndex(c.core.class, CLASS_KEYS) as Class;
  const pred = enumIndex(c.core.predisposition, ASPECT_KEYS) as Aspect;
  const name = bytesText(c.name);
  const hp = dead ? `0 / ${c.core.hpMax}` : party ? `${party.hp} / ${c.core.hpMax}` : `${c.core.hpMax} (max)`;

  return (
    <div className="char-detail">
      <header className={`panel char-detail-head${dead ? " dead" : ""}`}>
        <SpriteCanvas image={img.image} scale={8} label={name} />
        <div>
          <h1>{name}</h1>
          <p>
            {raceName(race)} {className(cls)} · {aspectName(pred)} predisposition
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
            {c.core.nft && (
              <>
                {" · "}
                <a href={explorerUrl("address", c.core.nft)} target="_blank" rel="noreferrer">
                  Core NFT
                </a>
              </>
            )}
          </p>
        </div>
      </header>

      <section className="panel">
        <h2>Stats</h2>
        <dl className="char-stats">
          {STAT_NAMES.map((s, i) => (
            <div key={s}>
              <dt>{s}</dt>
              <dd>{c.core.stats[i] ?? 0}</dd>
            </div>
          ))}
          <div>
            <dt>HP</dt>
            <dd>{hp}</dd>
          </div>
        </dl>
      </section>

      <section className="panel">
        <h2>Current party</h2>
        {party ? (
          <p>
            {party.status} · slot {party.slot} · HP {party.hp} · <Link to={`/party?s=${party.session}`}>open party</Link>
          </p>
        ) : c.core.inSession ? (
          <p className="muted">
            In party <span className="mono">{c.core.inSession}</span>, details unavailable.
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
                      {!usableInCombat(s) && <span className="muted"> · not usable in combat yet</span>}
                    </td>
                    <td>
                      {variantLabel(s.blocks.category)} {variantLabel(s.blocks.purpose).toLowerCase()} · {variantLabel(s.blocks.source)}
                    </td>
                    <td>{skillAspects(s)}</td>
                    <td>
                      {variantLabel(s.blocks.form)} (range {RANGE[variantKey(s.blocks.form)] ?? "?"})
                    </td>
                    <td>
                      {variantLabel(s.blocks.target)}
                      {s.blocks.auraRadius > 0 ? ` · aura ${s.blocks.auraRadius}` : ""}
                    </td>
                    <td>{formatBp(s.powerBp)}</td>
                    <td>{variantLabel(s.blocks.effect)}</td>
                    <td>{variantLabel(s.blocks.trigger)}</td>
                    <td>{s.cooldown}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <section className="panel char-detail-wide">
        <h2>Equipment</h2>
        <ul className="equip-grid">
          {EQUIPMENT_SLOTS.map((slot) => (
            <li key={slot} className="equip-slot">
              {slot}
            </li>
          ))}
        </ul>
        <p className="muted">Items coming soon.</p>
      </section>
    </div>
  );
}
