// Encoders for synthetic accounts, the inverse of layouts.ts. Used by mock mode and tests so they
// exercise the real decoders and hash checks.
import { getAddressEncoder, type Address } from "@solana/kit";
import { concat, u16le, u32le } from "./bytes";
import { ACCOUNT_DISC } from "./disc";
import { BLOB_KIND, type BlobKind } from "./layouts";

const enc = getAddressEncoder();
const addr = (a: Address) => enc.encode(a) as Uint8Array;
const BLOB_DISC: Record<BlobKind, Uint8Array> = {
  rasterizer: ACCOUNT_DISC.RasterizerCode,
  palette: ACCOUNT_DISC.Palette,
  sprite_book: ACCOUNT_DISC.SpriteBook,
};

export function encodeGameConfig(c: {
  authority: Address;
  programs: Address[];
  rasterizerVersion: number;
  rasterizer: Address;
  rasterizerSha256: Uint8Array;
  palette: Address;
  uriBase?: string;
  /** Up to 4; the rest of the slots stay empty (zeros). */
  spriteBooks?: Address[];
}): Uint8Array {
  const uri = new Uint8Array(64);
  uri.set(new TextEncoder().encode(c.uriBase ?? "").subarray(0, 64));
  const books = new Uint8Array(4 * 32);
  (c.spriteBooks ?? []).forEach((b, i) => books.set(addr(b), i * 32));
  return concat(
    ACCOUNT_DISC.GameConfig,
    new Uint8Array([1]),
    addr(c.authority),
    ...c.programs.map(addr),
    u16le(c.rasterizerVersion),
    addr(c.rasterizer),
    c.rasterizerSha256,
    addr(c.palette),
    uri,
    new Uint8Array([255]),
    books,
  );
}

export function encodeBlob(
  kind: BlobKind,
  id: number,
  body: Uint8Array,
  sha: Uint8Array,
  opts: { finalized?: boolean; authority?: Address } = {},
): Uint8Array {
  return concat(
    BLOB_DISC[kind],
    new Uint8Array([1, BLOB_KIND[kind], opts.finalized === false ? 0 : 1, 254]),
    u32le(body.length),
    u16le(id),
    new Uint8Array(2),
    opts.authority ? addr(opts.authority) : new Uint8Array(32),
    sha,
    opts.finalized === false ? new Uint8Array(32) : sha,
    body,
  );
}
