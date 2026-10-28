import { useState } from "react";
import type { Address, Instruction } from "@solana/kit";
import { CharacterForm } from "../components/forms/CharacterForm";
import { PrepayConfirm } from "../components/forms/PrepayConfirm";
import { StalledAction } from "../components/forms/StalledAction";
import type { CharacterDraft } from "../components/forms/types";
import { characterAppearance, isCurrentAppearance, renderRequest } from "../chain/appearance";
import { characterRequest, dressed, starterEquipment } from "../chain/items";
import {
  RACE_PREDISPOSITIONS,
  bytesText,
  characterSteps,
  optional,
  type CharacterAcc,
} from "../chain/game";
import { aspectName, className, raceName } from "../chain/names";
import { Aspect, Class, Race } from "../chain/types";
import { batchCost } from "../chain/send";
import { useTx } from "../chain/useTx";
import { DATA_SOURCE } from "../config";
import { OldCharacterLabel } from "../raster/OldCharacter";
import { useRasterizer, useRenderImage } from "../raster/RasterizerProvider";
import { PREVIEW_SCALE, SpriteCanvas, usePreviewFit } from "../raster/SpriteCanvas";
import { useAppWallet } from "../wallet/AppWallet";

/** programs/character rules.rs: STAT_BASE and STAT_POINTS_START. */
const STAT_MIN = 5;
const STAT_POINTS = 10;

/** One user action: profile (first time), character, Core NFT and skill descriptions. */
const CREATE = "Create the character";

/** A creation built and priced, waiting for the player's yes; the same steps are then signed. */
type Pending = { name: string; character: Address; steps: Instruction[][]; lamports: bigint };

/** Character creation (the characters window): a new character and its Core NFT. */
export function CharacterCreate({ onCreated, initial }: { onCreated?: (address: Address) => void; initial?: Partial<CharacterDraft> }) {
  const [draft, setDraft] = useState<CharacterDraft>({
    name: "",
    race: Race.Human,
    class: Class.Warrior,
    predisposition: Aspect.Piercing,
    stats: [STAT_MIN, STAT_MIN, STAT_MIN, STAT_MIN],
    ...initial,
  });
  const [mockCreated, setMockCreated] = useState<CharacterDraft | null>(null);
  const raster = useRasterizer();
  // A new character wears its class starter kit.
  const chainPreview = DATA_SOURCE === "chain" ? renderRequest(dressed(characterAppearance(draft.race, draft.predisposition), starterEquipment(draft.class))) : null;
  const preview = useRenderImage(chainPreview ?? (raster.status === "ready" ? raster.sample : null));
  const previewFit = usePreviewFit();
  const wallet = useAppWallet();
  const { sendSteps, stalled, dismissStalled, busy } = useTx();
  const [pending, setPending] = useState<Pending | null>(null);
  const [priceError, setPriceError] = useState<string | null>(null);

  const allowed = RACE_PREDISPOSITIONS[draft.race];
  const onChange = (next: CharacterDraft) => {
    const ok = RACE_PREDISPOSITIONS[next.race];
    setDraft(ok.includes(next.predisposition) ? next : { ...next, predisposition: ok[0]! });
  };

  // Build the transactions and show what they take (rent of the new accounts and the Core NFT,
  // network fees) before anything is signed (docs/GAME_COMMISSIONS.md 6.2).
  const create = async (d: CharacterDraft) => {
    if (DATA_SOURCE === "mock") return setMockCreated(d);
    const owner = wallet.signer;
    if (!owner) return void (await sendSteps(CREATE, null, []));
    setPriceError(null);
    try {
      const c = await characterSteps(owner, {
        name: d.name,
        race: d.race,
        class: d.class,
        predisposition: d.predisposition,
        stats: [...d.stats],
        ...(d.skills ? { skills: d.skills } : {}),
      });
      setPending({ name: d.name, character: c.character, steps: c.steps, lamports: await batchCost(owner, c.steps) });
    } catch (e) {
      setPriceError(`Could not price the character: ${e instanceof Error ? e.message : String(e)}`);
    }
  };
  const confirm = async (p: Pending) => {
    // One wallet confirmation for every transaction, sent in order (a retry resumes).
    if (await sendSteps(CREATE, wallet.signer, p.steps)) created(p);
  };
  const created = (p: Pending) => {
    setPending(null);
    setDraft((x) => ({ ...x, name: "" }));
    void wallet.refreshBalance();
    onCreated?.(p.character);
  };

  return (
    <section>
      <CharacterForm
        value={draft}
        onChange={onChange}
        onSubmit={(d) => void create(d)}
        statPoints={STAT_POINTS}
        statMin={STAT_MIN}
        allowedPredispositions={allowed}
        busy={busy !== null || pending !== null}
        preview={<SpriteCanvas image={preview.image} scale={PREVIEW_SCALE} fit={previewFit} label="character preview" />}
      />
      {pending && !stalled && (
        <PrepayConfirm
          action={`Create ${pending.name}`}
          lamports={pending.lamports}
          detail={`as rent for the character, its skills, bag and Core NFT, and the network fees of ${pending.steps.length} transaction${pending.steps.length > 1 ? "s" : ""} (one wallet confirmation)`}
          busy={busy !== null}
          onConfirm={() => void confirm(pending)}
          onCancel={() => setPending(null)}
        />
      )}
      {stalled && pending && (
        <StalledAction
          what={stalled.what}
          step={stalled.step}
          total={stalled.total}
          busy={busy !== null}
          onRetry={() => void stalled.retry().then((r) => r && created(pending))}
          onDismiss={() => {
            dismissStalled();
            // Past step 1 the character exists (only descriptions are missing): show it.
            if (stalled.step > 1) created(pending);
            else setPending(null);
          }}
        />
      )}
      {busy && <p className="muted">{busy}...</p>}
      {priceError && <p className="error">{priceError}</p>}
      {mockCreated && (
        <p className="ok">
          Mock: {mockCreated.name}, {raceName(mockCreated.race)} {className(mockCreated.class)} with {aspectName(mockCreated.predisposition)} predisposition.
        </p>
      )}
    </section>
  );
}

export function CharacterCard({ c, active = false, onOpen }: { c: CharacterAcc; active?: boolean; onOpen: () => void }) {
  const dead = !c.core.alive;
  const current = isCurrentAppearance(c.core.appearance);
  const req = current ? characterRequest(c) : null;
  const img = useRenderImage(req);
  const race = c.core.race as Race;
  const cls = c.core.class as Class;
  const pred = c.core.predisposition as Aspect;
  // The whole card opens the character window and makes it active; explorer links live there.
  return (
    <button type="button" className={`char-card-link${active ? " active" : ""}`} aria-label={`Open ${bytesText(c.name)}`} aria-pressed={active} onClick={onOpen}>
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
          <span className="muted">{active ? "Active · details →" : "Details →"}</span>
        </figcaption>
      </figure>
    </button>
  );
}
