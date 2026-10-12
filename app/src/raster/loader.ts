// GameConfig -> RasterizerCode -> SHA-256 check -> IndexedDB cache -> WebAssembly, plus the
// palette and sprite books the rasterizer draws from (docs/STORAGE_AND_DELIVERY.md, rasterizer skill).
import type { Address } from "@solana/kit";
import type { AccountSource } from "../chain/accounts";
import { equalBytes, sha256, toHex } from "../chain/bytes";
import { decodeBlob, decodeBlobHeader, decodeGameConfig, type GameConfig } from "../chain/layouts";
import { gameConfigPda, palettePda, rasterizerPda, spriteBookPda } from "../chain/pda";
import type { WasmCache } from "./idb";
import { Rasterizer } from "./rasterizer";

export type LoadedRender = {
  rasterizer: Rasterizer;
  config: GameConfig;
  sha256Hex: string;
  source: AccountSource["label"];
  fromCache: boolean;
  palette: Uint8Array;
  paletteVersion: number;
  books: { id: number; body: Uint8Array }[];
};

async function must(src: AccountSource, address: Address, what: string): Promise<Uint8Array> {
  const data = await src.get(address);
  if (!data) throw new Error(`${what} account ${address} not found`);
  return data;
}

/** Fetches and verifies the rasterizer WASM body. Throws on any mismatch, never falls back. */
export async function loadRasterizerWasm(
  src: AccountSource,
  config: GameConfig,
  cache: WasmCache,
): Promise<{ wasm: Uint8Array; fromCache: boolean }> {
  const expected = await rasterizerPda(config.rasterizerVersion);
  if (config.rasterizer !== expected) {
    throw new Error(`GameConfig.rasterizer ${config.rasterizer} is not the PDA for version ${config.rasterizerVersion}`);
  }
  const hex = toHex(config.rasterizerSha256);
  const cached = await cache.get(hex);
  if (cached && equalBytes(await sha256(cached), config.rasterizerSha256)) return { wasm: cached, fromCache: true };

  const { header, body } = decodeBlob(await must(src, config.rasterizer, "RasterizerCode"), "rasterizer", config.rasterizerVersion);
  const actual = await sha256(body);
  if (!equalBytes(actual, config.rasterizerSha256)) {
    throw new Error(`rasterizer hash mismatch: chain body ${toHex(actual)}, GameConfig ${hex}`);
  }
  if (!equalBytes(actual, header.sha256)) throw new Error("rasterizer hash differs from its finalized header");
  const wasm = body.slice();
  await cache.put(hex, wasm);
  return { wasm, fromCache: false };
}

export async function loadRender(
  src: AccountSource,
  opts: { cache: WasmCache; bookIds: number[]; configAddress?: Address },
): Promise<LoadedRender> {
  const config = decodeGameConfig(await must(src, opts.configAddress ?? (await gameConfigPda()), "GameConfig"));
  const { wasm, fromCache } = await loadRasterizerWasm(src, config, opts.cache);
  const rasterizer = await Rasterizer.instantiate(wasm);

  const palData = await must(src, config.palette, "Palette");
  const paletteVersion = decodeBlobHeader(palData).id;
  if ((await palettePda(paletteVersion)) !== config.palette) throw new Error("GameConfig.palette is not a palette PDA");
  const palette = decodeBlob(palData, "palette", paletteVersion).body;

  const books = await Promise.all(
    opts.bookIds.map(async (id) => ({
      id,
      body: decodeBlob(await must(src, await spriteBookPda(id), `SpriteBook ${id}`), "sprite_book", id).body,
    })),
  );
  return { rasterizer, config, sha256Hex: toHex(config.rasterizerSha256), source: src.label, fromCache, palette, paletteVersion, books };
}
