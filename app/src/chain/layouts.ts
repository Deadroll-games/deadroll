// Account and event decoders. Layouts: agents/CONTRACTS.md 4 (CharacterData, GameConfigData in
// crates/types/src/accounts.rs) and programs/render/src/state/blob.rs (BlobHeader offsets).
import type { Address } from "@solana/kit";
import { Reader, startsWith } from "./bytes";
import { ACCOUNT_DISC, EVENT_DISC } from "./disc";
import { LAYERS_MAX, type Aspect, type Class, type LayerRef, type Race } from "./types";

function expectDisc(data: Uint8Array, disc: Uint8Array, name: string): void {
  if (!startsWith(data, disc)) throw new Error(`not a ${name} account`);
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

/** Byte offsets after the discriminator are fixed: rasterizer_version at 265, sha256 at 299. */
export function decodeGameConfig(data: Uint8Array): GameConfig {
  expectDisc(data, ACCOUNT_DISC.GameConfig, "GameConfig");
  const r = new Reader(data, 8);
  const version = r.u8();
  const authority = r.address();
  const programs = Array.from({ length: 7 }, () => r.address());
  const rasterizerVersion = r.u16();
  const rasterizer = r.address();
  const rasterizerSha256 = r.bytes(32);
  const palette = r.address();
  const uriRaw = r.bytes(64);
  const end = uriRaw.indexOf(0);
  const uriBase = new TextDecoder().decode(end < 0 ? uriRaw : uriRaw.subarray(0, end));
  const bump = r.u8();
  const slots = Array.from({ length: SPRITE_BOOKS_MAX }, () => r.address());
  const spriteBooks = slots.filter((a) => a !== EMPTY_ADDRESS);
  return { version, authority, programs, rasterizerVersion, rasterizer, rasterizerSha256, palette, uriBase, bump, spriteBooks };
}

// ---- Render blobs (RasterizerCode, Palette, SpriteBook) ----

export const BLOB_BODY_OFFSET = 116;
export const BLOB_KIND = { rasterizer: 1, palette: 2, sprite_book: 3 } as const;
export type BlobKind = keyof typeof BLOB_KIND;

const BLOB_DISC: Record<BlobKind, Uint8Array> = {
  rasterizer: ACCOUNT_DISC.RasterizerCode,
  palette: ACCOUNT_DISC.Palette,
  sprite_book: ACCOUNT_DISC.SpriteBook,
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

/** Offsets: version 8, kind 9, finalized 10, bump 11, len 12..16, id 16..18, pad, authority 20..52, expected 52..84, sha 84..116. */
export function decodeBlobHeader(data: Uint8Array): BlobHeader {
  if (data.length < BLOB_BODY_OFFSET) {
    throw new Error(`account data is ${data.length} bytes, header needs ${BLOB_BODY_OFFSET}`);
  }
  const r = new Reader(data, 8);
  const version = r.u8();
  const kind = r.u8();
  const finalized = r.u8() !== 0;
  const bump = r.u8();
  const len = r.u32();
  const id = r.u16();
  r.bytes(2);
  const authority = r.address();
  const expectedSha256 = r.bytes(32);
  const sha256 = r.bytes(32);
  return { version, kind, finalized, bump, len, id, authority, expectedSha256, sha256 };
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

export function decodeCharacter(data: Uint8Array): Character {
  expectDisc(data, ACCOUNT_DISC.Character, "Character");
  const r = new Reader(data, 8);
  const version = r.u8();
  const owner = r.address();
  const index = r.u8();
  const nft = r.option(() => r.address());
  const race = r.u8() as Race;
  const cls = r.u8() as Class;
  const level = r.u16();
  const xp = r.u32();
  const stats: [number, number, number, number] = [r.u16(), r.u16(), r.u16(), r.u16()];
  const predisposition = r.u8() as Aspect;
  const predispositionTier = r.u8();
  const hpMax = r.u32();
  const alive = r.bool();
  const inSession = r.option(() => r.address());
  const appearance: LayerRef[] = Array.from({ length: LAYERS_MAX }, () => ({
    spriteId: r.u16(),
    materialRamp: r.u8(),
    accentRamp: r.u8(),
    elementRamp: r.u8(),
  }));
  const stateVersion = r.u32();
  const bump = r.u8();
  const nameRaw = r.bytes(32);
  const end = nameRaw.indexOf(0);
  const name = new TextDecoder().decode(end < 0 ? nameRaw : nameRaw.subarray(0, end));
  const unspentPoints = r.u8();
  const nftFrozen = r.bool();
  return {
    version, owner, index, nft, race, class: cls, level, xp, stats, predisposition, predispositionTier,
    hpMax, alive, inSession, appearance, stateVersion, bump, name, unspentPoints, nftFrozen,
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
export function decodeCharacterDied(data: Uint8Array): CharacterDied | null {
  if (!startsWith(data, EVENT_DISC.CharacterDied)) return null;
  const r = new Reader(data, 8);
  return {
    character: r.address(),
    owner: r.address(),
    session: r.address(),
    nft: r.option(() => r.address()),
    stateVersion: r.u32(),
  };
}
