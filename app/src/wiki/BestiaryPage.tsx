// The bestiary: every monster drawn so far (assets/sprites/monsters), its name from the sprite.
// Stats, text and skills only for the monsters of the registry bestiary (the ones in the game);
// the others show their picture and name. Built with the site, no RPC.
import wiki from "virtual:wiki";
import { MONSTER_SPRITES } from "../chain/appearance";
import { artUrl } from "../game/art";
import "./wiki.css";

type Record = (typeof wiki.bestiary)[number] & {
  role?: string;
  biome?: string;
  size?: number;
  min_tier?: number;
  speed?: number;
  attack_aspect?: string;
  attack_form?: string;
  hp?: number;
  hp_per_level?: number;
  armor?: number;
  evasion?: number;
  accuracy?: number;
  initiative?: number;
  damage_min?: number;
  damage_max?: number;
  xp?: number;
  pool_start?: number;
  pool_len?: number;
  resistance?: string[];
  immunity?: string[];
  kin?: string;
};

/** Sprite id -> the bestiary record drawn with it (`MONSTER_SPRITES` of the battle screen). */
const BY_SPRITE = new Map<number, Record>(
  wiki.bestiary.flatMap((r) => {
    const sprite = MONSTER_SPRITES[r.id]?.[0];
    return sprite === undefined ? [] : [[sprite, r as Record] as const];
  }),
);

export default function BestiaryPage() {
  const known = wiki.sprites.filter((s) => BY_SPRITE.has(s.id));
  const others = wiki.sprites.filter((s) => !BY_SPRITE.has(s.id));
  return (
    <div className="bestiary">
      <h1>Bestiary</h1>
      <p className="muted">
        {known.length} monsters are in the game; {others.length} more are drawn and wait for their place in the bestiary.
      </p>
      <div className="bestiary-grid">
        {known.map((s) => (
          <Monster key={s.id} sprite={s} record={BY_SPRITE.get(s.id)!} />
        ))}
      </div>
      <h2>Drawn, not in the game yet</h2>
      <div className="bestiary-grid small">
        {others.map((s) => (
          <figure key={s.id} className="panel bestiary-card">
            <img className="pixel" src={artUrl(s.art)} alt={s.name} />
            <figcaption>{s.name}</figcaption>
          </figure>
        ))}
      </div>
    </div>
  );
}

function Monster({ sprite, record: r }: { sprite: (typeof wiki.sprites)[number]; record: Record }) {
  const pool = wiki.monsterSkills.slice(r.pool_start ?? 0, (r.pool_start ?? 0) + (r.pool_len ?? 0));
  return (
    <figure className="panel bestiary-card wide">
      <img className="pixel" src={artUrl(sprite.art)} alt={sprite.name} />
      <figcaption>
        <h3>{sprite.name}</h3>
        <p>{r.about}</p>
        <p className="muted">
          {r.role} · {r.biome} · size {r.size} · from tier {r.min_tier}
          {r.kin ? ` · kin ${r.kin}` : ""}
        </p>
        <dl className="bestiary-stats">
          <dt>HP</dt>
          <dd>
            {r.hp} (+{r.hp_per_level} per level)
          </dd>
          <dt>Attack</dt>
          <dd>
            {r.attack_aspect} {r.attack_form?.toLowerCase()}, {r.damage_min}-{r.damage_max}
          </dd>
          <dt>Armor</dt>
          <dd>{r.armor}</dd>
          <dt>Evasion</dt>
          <dd>{r.evasion}</dd>
          <dt>Accuracy</dt>
          <dd>{r.accuracy}</dd>
          <dt>Initiative</dt>
          <dd>{r.initiative}</dd>
          <dt>Speed</dt>
          <dd>{r.speed}</dd>
          <dt>XP</dt>
          <dd>{r.xp}</dd>
          {!!r.resistance?.length && (
            <>
              <dt>Resists</dt>
              <dd>{r.resistance.join(", ")}</dd>
            </>
          )}
          {!!r.immunity?.length && (
            <>
              <dt>Immune</dt>
              <dd>{r.immunity.join(", ")}</dd>
            </>
          )}
        </dl>
        {pool.length > 0 && (
          <>
            <h4>Skills</h4>
            <ul>
              {pool.map((s) => (
                <li key={s.id}>
                  <b>{s.name}</b> ({String(s.category).toLowerCase()} {String(s.purpose).toLowerCase()}, {(s.aspects as string[]).join(" + ")}
                  {s.effect ? `, ${String(s.effect)}` : ""}): {s.about}
                </li>
              ))}
            </ul>
          </>
        )}
      </figcaption>
    </figure>
  );
}
