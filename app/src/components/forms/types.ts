// Props contracts for the character form, map builder palette and party lobby (L9.5).
// Pure UI: values in, callbacks out. No chain calls inside the components.
import type { ReactNode } from "react";
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
  onReady: (ready: boolean) => void;
  /** DM only; enabled when every player is ready and there is at least one. */
  onStart: () => void;
  busy?: boolean;
  error?: string | null;
  /** Map preview slot (the app passes a small <Grid>). */
  mapPreview?: ReactNode;
};

/** Unused by the components; exported for callers that place spawns from clicks. */
export type SpawnPlacement = Cell & { bestiaryId: number };
