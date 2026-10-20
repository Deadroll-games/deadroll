// Account and event reads in the app's shapes, over the sdk's generated decoders
// (GameConfig, the render blob header, Character, CharacterDied).
import type { Address, ReadonlyUint8Array } from "@solana/kit";
import { decodeEvent } from "@deadroll/sdk";
import { CHARACTER_DISCRIMINATOR, getCharacterDecoder, type CharacterDiedEvent } from "@deadroll/sdk/character";
import { GAME_CONFIG_DISCRIMINATOR, getGameConfigDecoder } from "@deadroll/sdk/registry";
import {
  PALETTE_DISCRIMINATOR,
  RASTERIZER_CODE_DISCRIMINATOR,
  SPRITE_BOOK_DISCRIMINATOR,
  getBlobHeaderDecoder,
} from "@deadroll/sdk/render";
import { bytesText, optional } from "./game";
import type { Aspect, Class, LayerRef, Race } from "./types";

function expectDisc(data: ReadonlyUint8Array, disc: ReadonlyUint8Array, name: string): void {
  if (data.length < 8 || disc.some((b, i) => data[i] !== b)) throw new Error(`not a ${name} account`);
}

// ---- GameConfig (registry ["config"]) ----

export type GameConfig = {
  version: number;
  authority: Address;
  /** Indexed by ProgramIdx: registry, render, character, items, map, session, city. */
  programs: Address[];
  rasterizerVersion: number;
  rasterizer: Address;
  rasterizerSha256: Uint8Array;
  palette: Address;
  uriBase: string;
  bump: number;
  /** Patch 7 registry field: the sprite books to draw from, in lookup order (empty slots dropped). */
  spriteBooks: Address[];
};

/** `GameConfig.sprite_books` slots (programs/registry `SPRITE_BOOKS_MAX`); `Pubkey::default()` = empty. */
export const SPRITE_BOOKS_MAX = 4;
const EMPTY_ADDRESS = "11111111111111111111111111111111";

export function decodeGameConfig(data: ReadonlyUint8Array): GameConfig {
  expectDisc(data, GAME_CONFIG_DISCRIMINATOR, "GameConfig");
  const { core, spriteBooks } = getGameConfigDecoder().decode(data);
  return {
    version: core.version,
    authority: core.authority,
    programs: [...core.programs],
    rasterizerVersion: core.rasterizerVersion,
    rasterizer: core.rasterizer,
    rasterizerSha256: Uint8Array.from(core.rasterizerSha256),
    palette: core.palette,
    uriBase: bytesText(core.uriBase),
    bump: core.bump,
    spriteBooks: spriteBooks.filter((a) => a !== EMPTY_ADDRESS),
  };
}

// ---- Render blobs (RasterizerCode, Palette, SpriteBook) ----

export const BLOB_BODY_OFFSET = 116;
export const BLOB_KIND = { rasterizer: 1, palette: 2, sprite_book: 3 } as const;
export type BlobKind = keyof typeof BLOB_KIND;

const BLOB_DISC: Record<BlobKind, ReadonlyUint8Array> = {
  rasterizer: RASTERIZER_CODE_DISCRIMINATOR,
  palette: PALETTE_DISCRIMINATOR,
  sprite_book: SPRITE_BOOK_DISCRIMINATOR,
};

export type BlobHeader = {
  version: number;
  kind: number;
  finalized: boolean;
  bump: number;
  len: number;
  id: number;
  authority: Address;
  expectedSha256: Uint8Array;
  sha256: Uint8Array;
};

/** The header after the discriminator (programs/render BlobHeader), then the body at `BLOB_BODY_OFFSET`. */
export function decodeBlobHeader(data: ReadonlyUint8Array): BlobHeader {
  if (data.length < BLOB_BODY_OFFSET) {
    throw new Error(`account data is ${data.length} bytes, header needs ${BLOB_BODY_OFFSET}`);
  }
  const h = getBlobHeaderDecoder().decode(data, 8);
  return {
    version: h.version,
    kind: h.kind,
    finalized: h.finalized !== 0,
    bump: h.bump,
    len: h.len,
    id: h.id,
    authority: h.authority,
    expectedSha256: Uint8Array.from(h.expectedSha256),
    sha256: Uint8Array.from(h.sha256),
  };
}

/** Checks discriminator, kind, id and finalized; returns the header and the exact body bytes. */
export function decodeBlob(data: Uint8Array, kind: BlobKind, id: number): { header: BlobHeader; body: Uint8Array } {
  expectDisc(data, BLOB_DISC[kind], kind);
  const header = decodeBlobHeader(data);
  if (header.version !== 1 || header.kind !== BLOB_KIND[kind]) throw new Error(`${kind}: bad blob header`);
  if (header.id !== id) throw new Error(`${kind}: id ${header.id}, expected ${id}`);
  if (!header.finalized) throw new Error(`${kind} ${id} is not finalized`);
  if (data.length !== BLOB_BODY_OFFSET + header.len) {
    throw new Error(`${kind}: account is ${data.length} bytes, header says ${BLOB_BODY_OFFSET + header.len}`);
  }
  return { header, body: data.subarray(BLOB_BODY_OFFSET) };
}

// ---- Character ----

export type Character = {
  version: number;
  owner: Address;
  index: number;
  nft: Address | null;
  race: Race;
  class: Class;
  level: number;
  xp: number;
  stats: [number, number, number, number];
  predisposition: Aspect;
  predispositionTier: number;
  hpMax: number;
  alive: boolean;
  inSession: Address | null;
  appearance: LayerRef[];
  stateVersion: number;
  bump: number;
  // lane fields (programs/character/src/state/character.rs)
  name: string;
  unspentPoints: number;
  nftFrozen: boolean;
};

export function decodeCharacter(data: ReadonlyUint8Array): Character {
  expectDisc(data, CHARACTER_DISCRIMINATOR, "Character");
  const { core, name, unspentPoints, nftFrozen } = getCharacterDecoder().decode(data);
  const [s0 = 0, s1 = 0, s2 = 0, s3 = 0] = core.stats;
  return {
    version: core.version,
    owner: core.owner,
    index: core.index,
    nft: optional(core.nft),
    race: core.race,
    class: core.class,
    level: core.level,
    xp: core.xp,
    stats: [s0, s1, s2, s3],
    predisposition: core.predisposition,
    predispositionTier: core.predispositionTier,
    hpMax: core.hpMax,
    alive: core.alive,
    inSession: optional(core.inSession),
    appearance: core.appearance,
    stateVersion: core.stateVersion,
    bump: core.bump,
    name: bytesText(name),
    unspentPoints,
    nftFrozen,
  };
}

// ---- Events ----

export type CharacterDied = {
  character: Address;
  owner: Address;
  session: Address;
  nft: Address | null;
  stateVersion: number;
};

/** `data` is the raw event bytes from a "Program data:" log line (discriminator first). */
export function decodeCharacterDied(data: ReadonlyUint8Array): CharacterDied | null {
  const ev = decodeEvent("character", data);
  if (ev?.name !== "CharacterDied") return null;
  const d = ev.data as CharacterDiedEvent;
  return { character: d.character, owner: d.owner, session: d.session, nft: optional(d.nft), stateVersion: d.stateVersion };
}
