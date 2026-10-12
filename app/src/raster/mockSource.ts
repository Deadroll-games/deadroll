// Mock mode: builds GameConfig + render blob accounts from local build outputs served by the dev
// server (/dev-assets, see vite.config.ts), then the loader runs its normal verify path on them.
import type { Address } from "@solana/kit";
import { mapAccountSource, type AccountSource } from "../chain/accounts";
import { sha256 } from "../chain/bytes";
import { encodeBlob, encodeGameConfig } from "../chain/fixtures";
import { gameConfigPda, palettePda, rasterizerPda, spriteBookPda } from "../chain/pda";
import { PROGRAMS } from "../config";
import { RENDER_REQUEST_LEN, decodeRenderRequest } from "./rasterizer";
import type { RenderRequest } from "../chain/types";

export const MOCK_RASTERIZER_VERSION = 1;

async function fetchBytes(path: string): Promise<Uint8Array> {
  const res = await fetch(path);
  if (!res.ok) throw new Error(`${path}: ${res.status} ${await res.text()}`);
  return new Uint8Array(await res.arrayBuffer());
}

export async function mockRenderSource(base = "/dev-assets"): Promise<{
  source: AccountSource;
  bookIds: number[];
  sample: RenderRequest;
}> {
  const [wasm, palette, book, golden] = (await Promise.all(
    ["rasterizer.wasm", "palette.bin", "book.bin", "golden-input.bin"].map((f) => fetchBytes(`${base}/${f}`)),
  )) as [Uint8Array, Uint8Array, Uint8Array, Uint8Array];
  const paletteVersion = palette[1]! | (palette[2]! << 8);
  const bookId = book[1]! | (book[2]! << 8);
  const rasterizer = await rasterizerPda(MOCK_RASTERIZER_VERSION);
  const paletteAddr = await palettePda(paletteVersion);
  const wasmSha = await sha256(wasm);
  const accounts = new Map<string, Uint8Array>([
    [
      await gameConfigPda(),
      encodeGameConfig({
        authority: PROGRAMS.registry,
        programs: Object.values(PROGRAMS) as Address[],
        rasterizerVersion: MOCK_RASTERIZER_VERSION,
        rasterizer,
        rasterizerSha256: wasmSha,
        palette: paletteAddr,
      }),
    ],
    [rasterizer, encodeBlob("rasterizer", MOCK_RASTERIZER_VERSION, wasm, wasmSha)],
    [paletteAddr, encodeBlob("palette", paletteVersion, palette, await sha256(palette))],
    [await spriteBookPda(bookId), encodeBlob("sprite_book", bookId, book, await sha256(book))],
  ]);
  // golden-input.bin starts with u32 len + the reference warrior RenderRequest.
  const sample = decodeRenderRequest(golden.subarray(4, 4 + RENDER_REQUEST_LEN));
  return { source: mapAccountSource(accounts), bookIds: [bookId], sample };
}
