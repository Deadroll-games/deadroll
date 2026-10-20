import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import type { Address } from "@solana/kit";
import { CharacterForm } from "../components/forms/CharacterForm";
import type { CharacterDraft } from "../components/forms/types";
import { characterAppearance, isCurrentAppearance, renderRequest } from "../chain/appearance";
import { characterRequest, dressed, starterEquipment } from "../chain/items";
import {
  RACE_PREDISPOSITIONS,
  bytesText,
  createCharacterIxs,
  optional,
  skillDescriptionIxs,
  SKILL_PRESETS,
  myCharacters,
  type CharacterAcc,
} from "../chain/game";
import { aspectName, className, raceName } from "../chain/names";
import { Aspect, Class, Race } from "../chain/types";
import { useTx } from "../chain/useTx";
import { DATA_SOURCE, DEV_TOOLS } from "../config";
import { OldCharacterLabel } from "../raster/OldCharacter";
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
  // A new character wears its class starter kit.
  const chainPreview = DATA_SOURCE === "chain" ? renderRequest(dressed(characterAppearance(draft.race, draft.predisposition), starterEquipment(draft.class))) : null;
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
    // The class starter items come with the character (create_character), worn.
    const res = await send(`create_character ${d.name} (Core NFT + starter items)`, owner, [c.ix]);
    if (res) {
      // The skill descriptions follow in their own transactions (six of 200 bytes do not fit
      // the creation one); each is written once.
      const batches = await skillDescriptionIxs(owner, c.character, d.skills ?? SKILL_PRESETS[d.class]);
      for (const [i, ixs] of batches.entries()) {
        if (!(await send(`skill descriptions ${i + 1}/${batches.length}`, owner, ixs))) break;
      }
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
        preview={<SpriteCanvas image={preview.image} scale={2} label="character preview" />}
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
  const current = isCurrentAppearance(c.core.appearance);
  const req = current ? characterRequest(c) : null;
  const img = useRenderImage(req);
  const race = c.core.race as Race;
  const cls = c.core.class as Class;
  const pred = c.core.predisposition as Aspect;
  // The whole card opens the detail view; explorer links (PDA, Core NFT) live there.
  return (
    <Link to={`/character/${address}`} className="char-card-link" aria-label={`Open ${bytesText(c.name)}`}>
      <figure className={`panel char-card${dead ? " dead" : ""}`}>
        {current ? <SpriteCanvas image={img.image} scale={1} label={bytesText(c.name)} /> : <OldCharacterLabel />}
        <figcaption>
          <b>{bytesText(c.name)}</b>
          <span className="muted">
            {raceName(race)} {className(cls)} · {aspectName(pred)} · lvl {c.core.level} · {c.core.xp} xp
          </span>
          <span className={dead ? "error" : "ok"}>
            {dead ? `Tombstone${c.nftFrozen ? " · NFT frozen" : " · freeze pending"}` : optional(c.core.inSession) ? "In a party" : "Alive"}
          </span>
          <span className="muted">Details →</span>
        </figcaption>
      </figure>
    </Link>
  );
}
