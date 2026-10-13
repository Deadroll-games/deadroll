import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import type { Address } from "@solana/kit";
import { CharacterForm } from "../components/forms/CharacterForm";
import type { CharacterDraft } from "../components/forms/types";
import { characterAppearance, renderRequest, TOMBSTONE } from "../chain/appearance";
import {
  ASPECT_KEYS,
  CLASS_KEYS,
  RACE_KEYS,
  RACE_PREDISPOSITIONS,
  bytesText,
  createCharacterIxs,
  enumIndex,
  myCharacters,
  type CharacterAcc,
} from "../chain/game";
import { aspectName, className, raceName } from "../chain/names";
import { Aspect, Class, Race } from "../chain/types";
import { useTx } from "../chain/useTx";
import { DATA_SOURCE, DEV_TOOLS } from "../config";
import { useRasterizer, useRenderImage } from "../raster/RasterizerProvider";
import { SpriteCanvas } from "../raster/SpriteCanvas";
import { useAppWallet } from "../wallet/AppWallet";

/** programs/character rules.rs: STAT_BASE and STAT_POINTS_START. */
const STAT_MIN = 5;
const STAT_POINTS = 10;

export function CharacterPage() {
  const [draft, setDraft] = useState<CharacterDraft>({
    name: "",
    race: Race.Human,
    class: Class.Warrior,
    predisposition: Aspect.Piercing,
    stats: [STAT_MIN, STAT_MIN, STAT_MIN, STAT_MIN],
  });
  const [mockCreated, setMockCreated] = useState<CharacterDraft | null>(null);
  const raster = useRasterizer();
  const chainPreview = DATA_SOURCE === "chain" ? renderRequest(characterAppearance(draft.race, draft.class, draft.predisposition)) : null;
  const preview = useRenderImage(chainPreview ?? (raster.status === "ready" ? raster.sample : null));
  const wallet = useAppWallet();
  const { send, busy } = useTx();
  const [mine, setMine] = useState<{ address: Address; data: CharacterAcc }[]>([]);

  const refresh = useCallback(async () => {
    if (DATA_SOURCE !== "chain" || !wallet.address) return setMine([]);
    try {
      setMine(await myCharacters(wallet.address));
    } catch {
      // shown as an empty list
    }
  }, [wallet.address]);
  useEffect(() => void refresh(), [refresh]);

  const allowed = RACE_PREDISPOSITIONS[draft.race];
  const onChange = (next: CharacterDraft) => {
    const ok = RACE_PREDISPOSITIONS[next.race];
    setDraft(ok.includes(next.predisposition) ? next : { ...next, predisposition: ok[0]! });
  };

  const create = async (d: CharacterDraft) => {
    if (DATA_SOURCE === "mock") return setMockCreated(d);
    const owner = wallet.signer;
    if (!owner) return void (await send("create_character", null, []));
    const c = await createCharacterIxs(owner, {
      name: d.name,
      race: d.race,
      class: d.class,
      predisposition: d.predisposition,
      stats: [...d.stats],
      ...(d.skills ? { skills: d.skills } : {}),
    });
    if (c.profileIx && !(await send("create_profile", owner, [c.profileIx]))) return;
    const res = await send(`create_character ${d.name} (Core NFT)`, owner, [c.ix]);
    if (res) {
      setDraft((x) => ({ ...x, name: "" }));
      await refresh();
      void wallet.refreshBalance();
    }
  };

  return (
    <section>
      <h1>Create a character</h1>
      <CharacterForm
        value={draft}
        onChange={onChange}
        onSubmit={(d) => void create(d)}
        statPoints={STAT_POINTS}
        statMin={STAT_MIN}
        allowedPredispositions={allowed}
        busy={busy !== null}
        preview={<SpriteCanvas image={preview.image} scale={6} label="character preview" />}
      />
      {mockCreated && (
        <p className="ok">
          Mock: {mockCreated.name}, {raceName(mockCreated.race)} {className(mockCreated.class)} with {aspectName(mockCreated.predisposition)} predisposition.
        </p>
      )}
      {DATA_SOURCE === "chain" && (
        <>
          <h2>My characters</h2>
          {!wallet.address && <p className="muted">{DEV_TOOLS ? "Connect a wallet, or open the app with ?dev=1 on localnet." : "Connect a wallet."}</p>}
          {wallet.address && mine.length === 0 && <p className="muted">None yet.</p>}
          <div className="char-list">
            {mine.map((c) => (
              <CharacterCard key={c.address} address={c.address} c={c.data} />
            ))}
          </div>
        </>
      )}
    </section>
  );
}

export function CharacterCard({ address, c }: { address: Address; c: CharacterAcc }) {
  const dead = !c.core.alive;
  const req = dead ? TOMBSTONE : renderRequest(c.core.appearance, false);
  const img = useRenderImage(req);
  const race = enumIndex(c.core.race, RACE_KEYS) as Race;
  const cls = enumIndex(c.core.class, CLASS_KEYS) as Class;
  const pred = enumIndex(c.core.predisposition, ASPECT_KEYS) as Aspect;
  // The whole card opens the detail view; explorer links (PDA, Core NFT) live there.
  return (
    <Link to={`/character/${address}`} className="char-card-link" aria-label={`Open ${bytesText(c.name)}`}>
      <figure className={`panel char-card${dead ? " dead" : ""}`}>
        <SpriteCanvas image={img.image} scale={3} label={bytesText(c.name)} />
        <figcaption>
          <b>{bytesText(c.name)}</b>
          <span className="muted">
            {raceName(race)} {className(cls)} · {aspectName(pred)} · lvl {c.core.level} · {c.core.xp} xp
          </span>
          <span className={dead ? "error" : "ok"}>
            {dead ? `Tombstone${c.nftFrozen ? " · NFT frozen" : " · freeze pending"}` : c.core.inSession ? "In a party" : "Alive"}
          </span>
          <span className="muted">Details →</span>
        </figcaption>
      </figure>
    </Link>
  );
}
