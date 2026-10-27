// The game screen: the city, a building's interior or the dungeon on the main area, the side panel
// to its right and the windows over the main area. There are no separate pages: everything opens
// from the city, the Tavern and the panel. The main area is in the URL (`?view=`), so a reload
// keeps the dungeon.
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useSearchParams } from "react-router-dom";
import { getAddressEncoder, type Address } from "@solana/kit";
import { bytesText, findAccounts, status } from "../chain/game";
import type { QuestView } from "../chain/city";
import type { NamedSkill } from "../chain/skillcheck";
import type { Class } from "../chain/types";
import { DATA_SOURCE } from "../config";
import { PartyProvider } from "../routes/PartyChain";
import { useAppWallet } from "../wallet/AppWallet";
import { partyOf, useActiveCharacter } from "./ActiveCharacter";
import { CityView } from "./city/CityView";
import { InteriorView } from "./city/InteriorView";
import { isOpen, isTavern, loadCity, type CityBuilding, type CityModel } from "./city/model";
import { INTERIORS } from "./city/prefabs";
import { Dungeon } from "./Dungeon";
import { SidePanel, type PanelAction } from "./SidePanel";
import { BoardWindow } from "./tavern/BoardWindow";
import { DmTableWindow } from "./tavern/DmTableWindow";
import { QuestWindow } from "./tavern/QuestWindow";
import { GameWindow } from "./Window";
import { BagWindow, CharactersWindow, CharacterWindow, SettingsWindow, SkillsWindow } from "./windows";
import "./game.css";

type Win =
  | { kind: "bag" }
  | { kind: "skills" }
  | { kind: "characters"; creating: boolean; initial?: { class: Class; skills: NamedSkill[] } }
  | { kind: "character"; address: Address }
  | { kind: "settings" }
  | { kind: "board"; board: Address }
  | { kind: "quest"; quest: QuestView }
  | { kind: "dm"; building: Address; board: Address };

/** `GameSessionData.dm`: after the discriminator, `version` and `map`. */
const SESSION_DM_OFFSET = 8 + 1 + 32;
/** How often the panel looks for a party the wallet runs as DM. */
const DM_PARTY_POLL_MS = 30_000;

export function GameShell() {
  const [params, setParams] = useSearchParams();
  const view = params.get("view") ?? "city";
  const buildingId = params.get("b");
  const session = params.get("s") as Address | null;
  const biomeHint = params.get("biome");
  const go = useCallback((next: Record<string, string>) => setParams(next), [setParams]);

  const wallet = useAppWallet();
  const { active } = useActiveCharacter();
  const [city, setCity] = useState<CityModel | null | "missing">(null);
  const [win, setWin] = useState<Win | null>(null);
  const [dmParty, setDmParty] = useState<Address | null>(null);

  useEffect(() => {
    let live = true;
    loadCity(DATA_SOURCE !== "chain").then(
      (c) => live && setCity(c ?? "missing"),
      () => live && setCity("missing"),
    );
    return () => {
      live = false;
    };
  }, []);

  // A party this wallet runs as DM (lobby or in progress): the dungeon button's target when the
  // active character is in no party.
  useEffect(() => {
    if (DATA_SOURCE !== "chain" || !wallet.address) return setDmParty(null);
    const me = wallet.address;
    let live = true;
    const look = () =>
      findAccounts("GameSession", [{ offset: SESSION_DM_OFFSET, bytes: addressBytes(me) }]).then(
        (all) => live && setDmParty(all.find((s) => ["lobby", "active"].includes(status(s.data)))?.address ?? null),
        () => undefined,
      );
    void look();
    const t = setInterval(() => void look(), DM_PARTY_POLL_MS);
    return () => {
      live = false;
      clearInterval(t);
    };
  }, [wallet.address]);

  const currentParty = partyOf(active) ?? dmParty;
  const partyAddress = view === "dungeon" ? session : currentParty;
  const building = city && city !== "missing" && buildingId !== null ? (city.buildings.find((b) => String(b.id) === buildingId) ?? null) : null;

  const enterDungeon = useCallback(
    (s: Address, biome?: number) => {
      setWin(null);
      go({ view: "dungeon", s, ...(biome !== undefined ? { biome: String(biome) } : {}) });
    },
    [go],
  );

  const onPanel = (a: PanelAction) => {
    if (a === "city") return setWin(null), go({});
    if (a === "dungeon") return currentParty ? enterDungeon(currentParty) : undefined;
    if (a === "characters") return setWin({ kind: "characters", creating: false });
    setWin(win?.kind === a ? null : ({ kind: a } as Win));
  };

  const enterBuilding = (b: CityBuilding) => {
    if (!isOpen(b)) return;
    setWin(null);
    go({ view: "inside", b: String(b.id) });
  };

  let main: ReactNode;
  if (view === "dungeon") {
    main = <Dungeon session={session} biome={biomeHint !== null ? Number(biomeHint) : null} />;
  } else if (city === null) {
    main = <p className="muted">Walking to the city…</p>;
  } else if (city === "missing") {
    main = <p className="error">No city on this cluster yet: the city program has not been seeded (tools/scripts seed-city).</p>;
  } else if (view === "inside" && building && isOpen(building) && INTERIORS[building.interiorId]) {
    main = (
      <InteriorView
        interior={INTERIORS[building.interiorId]!}
        onAction={(action) => {
          if (action === "exit") return go({});
          if (!isTavern(building)) return;
          if (action === "board") setWin({ kind: "board", board: building.target });
          if (action === "dm") setWin({ kind: "dm", building: building.address, board: building.target });
        }}
      />
    );
  } else {
    main = <CityView city={city} onEnter={enterBuilding} />;
  }

  const panelCurrent: PanelAction | null =
    win?.kind === "bag" || win?.kind === "skills" || win?.kind === "settings" ? win.kind : win?.kind === "characters" || win?.kind === "character" ? "characters" : view === "dungeon" ? "dungeon" : view === "city" ? "city" : null;

  const body = (
    <div className="game">
      <main className="game-main">
        {main}
        {win && <WindowFor win={win} setWin={setWin} enterDungeon={enterDungeon} />}
      </main>
      <SidePanel
        current={panelCurrent}
        disabled={currentParty ? {} : { dungeon: "Not in a party: take a quest in the Tavern" }}
        activeName={active ? bytesText(active.data.name) : null}
        onAction={onPanel}
      />
    </div>
  );
  return partyAddress && DATA_SOURCE === "chain" ? <PartyProvider address={partyAddress}>{body}</PartyProvider> : body;
}

function WindowFor({ win, setWin, enterDungeon }: { win: Win; setWin: (w: Win | null) => void; enterDungeon: (s: Address, biome?: number) => void }) {
  const close = () => setWin(null);
  switch (win.kind) {
    case "bag":
      return (
        <GameWindow title="Bag" onClose={close} wide>
          <BagWindow />
        </GameWindow>
      );
    case "skills":
      return (
        <GameWindow title="Skills" onClose={close} wide>
          <SkillsWindow onCreateWith={(cls, skills) => setWin({ kind: "characters", creating: true, initial: { class: cls, skills } })} />
        </GameWindow>
      );
    case "characters":
      return (
        <GameWindow title={win.creating ? "New character" : "Characters"} onClose={close} wide>
          <CharactersWindow
            creating={win.creating}
            initial={win.initial}
            onCreate={(creating) => setWin({ kind: "characters", creating })}
            onOpen={(address) => setWin({ kind: "character", address })}
          />
        </GameWindow>
      );
    case "character":
      return (
        <GameWindow title="Character" onClose={close} wide>
          <CharacterWindow address={win.address} onOpenParty={(s) => enterDungeon(s)} />
        </GameWindow>
      );
    case "settings":
      return (
        <GameWindow title="Settings" onClose={close}>
          <SettingsWindow />
        </GameWindow>
      );
    case "board":
      return (
        <GameWindow title="Guild board" onClose={close} wide>
          <BoardWindow board={win.board} onOpen={(quest) => setWin({ kind: "quest", quest })} />
        </GameWindow>
      );
    case "quest":
      return (
        <GameWindow title="Quest" onClose={() => setWin({ kind: "board", board: win.quest.quest.core.board })} wide>
          <QuestWindow
            quest={win.quest}
            onEnter={(s) => enterDungeon(s, win.quest.quest.core.biome)}
            onClosed={() => setWin({ kind: "board", board: win.quest.quest.core.board })}
          />
        </GameWindow>
      );
    case "dm":
      return (
        <GameWindow title="DM's table" onClose={close} wide>
          <DmTableWindow building={win.building} onPosted={() => setWin({ kind: "board", board: win.board })} />
        </GameWindow>
      );
  }
}

const enc = getAddressEncoder();
const addressBytes = (a: Address): Uint8Array => Uint8Array.from(enc.encode(a));
