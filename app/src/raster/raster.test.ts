// @vitest-environment node
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it, vi } from "vitest";
import type { Address } from "@solana/kit";
import { mapAccountSource } from "../chain/accounts";
import { sha256, toHex } from "../chain/bytes";
import { encodeBlob, encodeGameConfig } from "../chain/fixtures";
import { decodeGameConfig } from "../chain/layouts";
import { gameConfigPda, palettePda, rasterizerPda, spriteBookPda } from "../chain/pda";
import { PROGRAMS } from "../config";
import { idbWasmCache, noCache, type WasmCache } from "./idb";
import { loadRasterizerWasm, loadRender } from "./loader";
import { OutMode, RENDER_REQUEST_LEN, RenderError, decodeRenderRequest, encodeRenderRequest, paletteVersionOf } from "./rasterizer";

const root = resolve(import.meta.dirname, "../../..");
const file = (rel: string) => resolve(root, rel);
const WASM = file("target/render-wasm/deadroll_render.wasm");
const GOLDEN = file("target/render-golden");
const haveBuild = existsSync(WASM) && existsSync(resolve(GOLDEN, "state.bin"));
if (!haveBuild) console.warn("skipping rasterizer parity tests: run crates/render/wasm/build.sh and cargo test -p deadroll-render --test golden");

async function fixture(body: Uint8Array, opts: { configSha?: Uint8Array; headerSha?: Uint8Array; finalized?: boolean; rasterizer?: Address } = {}) {
  const version = 3;
  const rasterizer = await rasterizerPda(version);
  const real = await sha256(body);
  const configData = encodeGameConfig({
    authority: PROGRAMS.registry,
    programs: Object.values(PROGRAMS),
    rasterizerVersion: version,
    rasterizer: opts.rasterizer ?? rasterizer,
    rasterizerSha256: opts.configSha ?? real,
    palette: await palettePda(1),
  });
  const accounts = new Map<string, Uint8Array>([
    [await gameConfigPda(), configData],
    [rasterizer, encodeBlob("rasterizer", version, body, opts.headerSha ?? real, { finalized: opts.finalized })],
  ]);
  return { src: mapAccountSource(accounts), config: decodeGameConfig(configData) };
}

function memoryCache(): WasmCache & { map: Map<string, Uint8Array> } {
  const map = new Map<string, Uint8Array>();
  return { map, get: async (k) => map.get(k) ?? null, put: async (k, v) => void map.set(k, v) };
}

describe("loadRasterizerWasm", () => {
  const body = new Uint8Array([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0, 42]);

  it("verifies the body against GameConfig and caches it by hash", async () => {
    const { src, config } = await fixture(body);
    const cache = memoryCache();
    const first = await loadRasterizerWasm(src, config, cache);
    expect(first).toEqual({ wasm: body, fromCache: false });
    expect([...cache.map.keys()]).toEqual([toHex(await sha256(body))]);
    const get = vi.spyOn(src, "get");
    expect(await loadRasterizerWasm(src, config, cache)).toEqual({ wasm: body, fromCache: true });
    expect(get).not.toHaveBeenCalled();
  });

  it("rejects a body whose hash differs from GameConfig", async () => {
    const { src, config } = await fixture(body, { configSha: new Uint8Array(32).fill(1) });
    await expect(loadRasterizerWasm(src, config, noCache)).rejects.toThrow(/hash mismatch/);
  });

  it("rejects a header hash that differs from the body", async () => {
    const { src, config } = await fixture(body, { headerSha: new Uint8Array(32).fill(2) });
    await expect(loadRasterizerWasm(src, config, noCache)).rejects.toThrow(/finalized header/);
  });

  it("rejects a rasterizer that is not finalized", async () => {
    const { src, config } = await fixture(body, { finalized: false });
    await expect(loadRasterizerWasm(src, config, noCache)).rejects.toThrow(/not finalized/);
  });

  it("rejects a GameConfig.rasterizer that is not the version PDA", async () => {
    const { src, config } = await fixture(body, { rasterizer: PROGRAMS.map });
    await expect(loadRasterizerWasm(src, config, noCache)).rejects.toThrow(/not the PDA/);
  });

  it("treats a tampered cache entry as a miss and refetches", async () => {
    const { src, config } = await fixture(body);
    const cache = memoryCache();
    cache.map.set(toHex(config.rasterizerSha256), new Uint8Array([6, 6, 6]));
    expect(await loadRasterizerWasm(src, config, cache)).toEqual({ wasm: body, fromCache: false });
    expect(cache.map.get(toHex(config.rasterizerSha256))).toEqual(body);
  });

  it("IndexedDB cache round-trips", async () => {
    await idbWasmCache.put("ab", new Uint8Array([1, 2]));
    expect(await idbWasmCache.get("ab")).toEqual(new Uint8Array([1, 2]));
    expect(await idbWasmCache.get("missing")).toBeNull();
  });
});

describe("RenderRequest", () => {
  it("round-trips 106 bytes", () => {
    const bytes = new Uint8Array(RENDER_REQUEST_LEN).map((_, i) => (i * 7) & 0xff);
    expect(encodeRenderRequest(decodeRenderRequest(bytes))).toEqual(bytes);
  });
});

describe.skipIf(!haveBuild)("rasterizer from accounts (golden parity)", () => {
  async function loaded(opts: { books?: Address[]; extra?: [Address, Uint8Array][] } = {}) {
    const wasm = new Uint8Array(readFileSync(WASM));
    const palette = new Uint8Array(readFileSync(resolve(GOLDEN, "palette.bin")));
    const book = new Uint8Array(readFileSync(resolve(GOLDEN, "book.bin")));
    const paletteVersion = palette[1]! | (palette[2]! << 8);
    const bookId = book[1]! | (book[2]! << 8);
    const rasterizer = await rasterizerPda(2);
    const wasmSha = await sha256(wasm);
    const bookAddr = await spriteBookPda(bookId);
    const accounts = new Map<string, Uint8Array>([
      [
        await gameConfigPda(),
        encodeGameConfig({
          authority: PROGRAMS.registry,
          programs: Object.values(PROGRAMS),
          rasterizerVersion: 2,
          rasterizer,
          rasterizerSha256: wasmSha,
          palette: await palettePda(paletteVersion),
          spriteBooks: opts.books ?? [bookAddr],
        }),
      ],
      [rasterizer, encodeBlob("rasterizer", 2, wasm, wasmSha)],
      [await palettePda(paletteVersion), encodeBlob("palette", paletteVersion, palette, await sha256(palette))],
      [bookAddr, encodeBlob("sprite_book", bookId, book, await sha256(book))],
      ...(opts.extra ?? []),
    ]);
    return loadRender(mapAccountSource(accounts), { cache: noCache });
  }

  const golden = () => decodeRenderRequest(new Uint8Array(readFileSync(resolve(GOLDEN, "state.bin"))));

  it("renders the reference warrior byte-identical to native (PNG up to 512 px)", async () => {
    const r = await loaded();
    expect(r.books.map((b) => b.id)).toEqual([new Uint8Array(readFileSync(resolve(GOLDEN, "book.bin")))[1]]);
    const req = golden();
    const png = r.rasterizer.render(req, r.palette, r.books.map((b) => b.body), OutMode.PNG);
    const expected = readFileSync(resolve(GOLDEN, "expected.sha256"), "utf8").trim();
    expect(createHash("sha256").update(png.bytes).digest("hex")).toBe(expected);
    // Indexed PNG: color type 3 in IHDR.
    expect(png.bytes[25]).toBe(3);
    const rgba = r.rasterizer.render(req, r.palette, r.books.map((b) => b.body), OutMode.RGBA);
    expect(rgba.bytes.length).toBe(rgba.width * rgba.height * 4);
    // The canvas enlarged by the integer scale that fits 512 px: 64 -> x8, 96 -> x5.
    const k = Math.floor(512 / Math.max(rgba.width, rgba.height));
    expect([png.width, png.height]).toEqual([rgba.width * k, rgba.height * k]);
  });

  it("draws with the palette version of the loaded palette, whatever the request says", async () => {
    const r = await loaded();
    const req = golden();
    const books = r.books.map((b) => b.body);
    const want = r.rasterizer.render(req, r.palette, books).bytes;
    expect(r.rasterizer.render({ ...req, paletteVersion: 0 }, r.palette, books).bytes).toEqual(want);
    expect(paletteVersionOf(r.palette)).toBe(r.paletteVersion);
  });

  it("looks sprites up in every GameConfig book, in order", async () => {
    // An empty book first (format 1, id 3, no sprites): every sprite comes from the second one.
    const empty = new Uint8Array([1, 3, 0, 0, 0]);
    const emptyAddr = await spriteBookPda(3);
    const golden1 = await loaded();
    const goldenAddr = golden1.books[0]!.address;
    const r = await loaded({
      books: [emptyAddr, goldenAddr],
      extra: [[emptyAddr, encodeBlob("sprite_book", 3, empty, await sha256(empty))]],
    });
    expect(r.books.map((b) => b.id)).toEqual([3, golden1.books[0]!.id]);
    const req = golden();
    const one = golden1.rasterizer.render(req, golden1.palette, golden1.books.map((b) => b.body));
    const two = r.rasterizer.render(req, r.palette, r.books.map((b) => b.body));
    expect(two.bytes).toEqual(one.bytes);
  });

  it("takes the books from GameConfig and rejects a missing or foreign one", async () => {
    await expect(loaded({ books: [] })).rejects.toThrow(/no sprite books/);
    await expect(loaded({ books: [await palettePda(1)] })).rejects.toThrow();
  });

  it("surfaces FORMAT.md error codes", async () => {
    const r = await loaded();
    const req = golden();
    expect(() => r.rasterizer.render(req, r.palette, r.books.map((b) => b.body), 2 as OutMode)).toThrow(RenderError);
    try {
      r.rasterizer.render(req, r.palette, []);
      expect.unreachable();
    } catch (e) {
      expect((e as RenderError).code).toBe(9);
    }
    try {
      r.rasterizer.render({ ...req, layers: req.layers.map((l, i) => (i === 8 ? { ...l, spriteId: 65000 } : l)) }, r.palette, r.books.map((b) => b.body));
      expect.unreachable();
    } catch (e) {
      expect((e as RenderError).code).toBe(7);
    }
  });
});
