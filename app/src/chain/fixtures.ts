// Encoders for synthetic accounts, the inverse of layouts.ts, through the sdk's generated encoders.
// Used by mock mode and tests so they exercise the real decoders and hash checks.
import type { Address } from "@solana/kit";
import { getGameConfigEncoder } from "@deadroll/sdk/registry";
import { getPaletteEncoder, getRasterizerCodeEncoder, getSpriteBookEncoder, type BlobHeaderArgs } from "@deadroll/sdk/render";
import { concat } from "./bytes";
import { BLOB_KIND, type BlobKind } from "./layouts";

const DEFAULT_KEY = "11111111111111111111111111111111" as Address;
/** `GameConfig.sprite_books` slots. */
const SPRITE_BOOK_SLOTS = 4;

export function encodeGameConfig(c: {
  authority: Address;
  programs: Address[];
  rasterizerVersion: number;
  rasterizer: Address;
  rasterizerSha256: Uint8Array;
  palette: Address;
  uriBase?: string;
  /** Up to 4; the rest of the slots stay empty (`Pubkey::default()`). */
  spriteBooks?: Address[];
}): Uint8Array {
  const uriBase = new Uint8Array(64);
  uriBase.set(new TextEncoder().encode(c.uriBase ?? "").subarray(0, 64));
  const books = c.spriteBooks ?? [];
  return Uint8Array.from(
    getGameConfigEncoder().encode({
      core: {
        version: 1,
        authority: c.authority,
        programs: c.programs,
        rasterizerVersion: c.rasterizerVersion,
        rasterizer: c.rasterizer,
        rasterizerSha256: c.rasterizerSha256,
        palette: c.palette,
        uriBase,
        bump: 255,
      },
      spriteBooks: Array.from({ length: SPRITE_BOOK_SLOTS }, (_, i) => books[i] ?? DEFAULT_KEY),
    }),
  );
}

const BLOB_ENCODERS = {
  rasterizer: getRasterizerCodeEncoder,
  palette: getPaletteEncoder,
  sprite_book: getSpriteBookEncoder,
} as const;

export function encodeBlob(
  kind: BlobKind,
  id: number,
  body: Uint8Array,
  sha: Uint8Array,
  opts: { finalized?: boolean; authority?: Address } = {},
): Uint8Array {
  const finalized = opts.finalized !== false;
  const header: BlobHeaderArgs = {
    version: 1,
    kind: BLOB_KIND[kind],
    finalized: finalized ? 1 : 0,
    bump: 254,
    len: body.length,
    id,
    pad: new Uint8Array(2),
    authority: opts.authority ?? DEFAULT_KEY,
    expectedSha256: sha,
    sha256: finalized ? sha : new Uint8Array(32),
  };
  return concat(Uint8Array.from(BLOB_ENCODERS[kind]().encode({ header })), body);
}
