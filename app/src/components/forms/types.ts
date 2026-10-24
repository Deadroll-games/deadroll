// Props contracts for the character form, map builder palette and party lobby (L9.5).
// Pure UI: values in, callbacks out. No chain calls inside the components.
import type { ReactNode } from "react";
import type { Category, NamedSkill } from "../../chain/skillcheck";
import type { Aspect, Cell, Class, Race, Spawn, Tile } from "../../chain/types";

// ---- Character creation ----

export type CharacterDraft = {
  /** 1..=32 printable ASCII, no leading space (programs/character rules.rs name_valid). */
  name: string;
  race: Race;
  class: Class;
  predisposition: Aspect;
  /** Strength, Dexterity, Intelligence, Endurance. */
  stats: [number, number, number, number];
  /** Starting skills (at most 6). Unset = the class presets, following the class. */
  skills?: NamedSkill[];
};

export type CharacterFormProps = {
  value: CharacterDraft;
  onChange: (next: CharacterDraft) => void;
  onSubmit: (draft: CharacterDraft) => void;
  /** Points to distribute over the base stats; the form shows the remaining points. */
  statPoints: number;
  /** Minimum per stat. */
  statMin: number;
  /** Aspects allowed as predisposition for the selected race/class (others disabled). */
  allowedPredispositions: Aspect[];
  busy?: boolean;
  error?: string | null;
  /** Live sprite preview slot, rendered next to the form (the app passes a <SpriteCanvas>). */
  preview?: ReactNode;
};

// ---- Map builder ----

export type BuilderTool =
  | { kind: "tile"; tile: Tile }
  | { kind: "spawn"; bestiaryId: number }
  /** Patch 11: place a visible chest (docs/MAP_DESCRIPTION.md 7.1); Erase removes it. */
  | { kind: "chest" }
  /** Patch 13: move the entrance or the exit point (one of each). */
  | { kind: "entrance" }
  | { kind: "exit" }
  | { kind: "erase" };

export type BestiaryEntry = { id: number; name: string };

export type MapPreset = { name: string; width: number; height: number };

export type MapPaletteProps = {
  tool: BuilderTool;
  onToolChange: (tool: BuilderTool) => void;
  bestiary: BestiaryEntry[];
  presets: MapPreset[];
  onLoadPreset: (name: string) => void;
  /** Current spawns, listed with a remove button each. */
  spawns: Spawn[];
  onRemoveSpawn: (index: number) => void;
  /** Optional editor under a spawn (level, elite, skills). */
  renderSpawn?: (spawn: Spawn, index: number) => ReactNode;
  /** Patch 11: optional loot panel (chest tables, boss table, minimum players, budget). */
  loot?: ReactNode;
  /** Validation errors from crates/mapcheck (shown as a list); empty = valid. */
  errors: string[];
  onValidate: () => void;
  /** Enabled only when validated with no errors. */
  canFinalize: boolean;
  onFinalize: () => void;
  busy?: boolean;
};

// ---- Party lobby ----

export type LobbyPlayer = {
  slot: number;
  /** Wallet address, shown shortened. */
  player: string;
  characterName: string;
  race: Race;
  class: Class;
  ready: boolean;
  /** Lamports paid into escrow. */
  paid: bigint;
};

export type PartyLobbyProps = {
  mapName: string;
  dm: string;
  priceLamports: bigint;
  /** Lamports collected in escrow so far. */
  collected: bigint;
  players: LobbyPlayer[];
  maxPlayers: number;
  /** "dm" sees Start; "player" sees Join/Ready; "guest" sees Join only. */
  role: "dm" | "player" | "guest";
  /** For role "player": own slot. */
  mySlot?: number | null;
  /** Characters the viewer can join with (role guest). */
  myCharacters: { address: string; name: string }[];
  onJoin: (characterAddress: string) => void;
  /** What Join pays (the player's share); defaults to the full price. */
  joinLamports?: bigint;
  /** Patch 11: the rent of the accounts Join creates (participation, reward cache), shown with the
   * share before anything is signed. */
  joinRentLamports?: bigint;
  /** Ready with the skills taken onto the map (SkillSet indexes, 4 + 1 + 1, NO_SKILL = empty), or unready. */
  onReady: (ready: boolean, selection: number[]) => void;
  /** For role "player": the joined character's skills, by SkillSet index, for the map selection. */
  mySkills?: LobbySkill[];
  /** DM only; enabled when every player is ready and there is at least one. */
  onStart: () => void;
  busy?: boolean;
  error?: string | null;
  /** Map preview slot (the app passes a small <Grid>). */
  mapPreview?: ReactNode;
  /** What Ready moves from the wallet to a new session key (the fee prepayment). When set, Ready
   * asks for an explicit confirmation of the amount first; unset or 0 when the party's key is reused. */
  readyPrepayLamports?: bigint;
};

/** Unused by the components; exported for callers that place spawns from clicks. */
export type SpawnPlacement = Cell & { bestiaryId: number };

/** One skill of the joined character, as the lobby's map selection lists it. */
export type LobbySkill = { index: number; name: string; category: Category };
