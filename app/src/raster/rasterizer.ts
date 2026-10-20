// The on-chain rasterizer behind its C ABI (crates/render/FORMAT.md, section 7). Same module as
// in tools and the wallet worker, so the bytes match everywhere.
import { concat, u32le } from "../chain/bytes";
import { LAYERS_MAX, type RenderRequest } from "../chain/types";

export const RENDER_REQUEST_LEN = 106;

/** FORMAT.md section 5. */
export const RENDER_ERRORS: Record<number, string> = {
  1: "truncated input or trailing bytes",
  2: "unsupported format version",
  3: "bad canvas size, layer type or flags",
  4: "unknown opcode",
  5: "bad slot",
  6: "argument out of range",
  7: "sprite id not found or bad book index",
  8: "bad palette",
  9: "bad render request, output mode or no sprite book",
  10: "more than 256 colors for an indexed PNG",
};

export class RenderError extends Error {
  constructor(readonly code: number) {
    super(`rasterizer error ${code}: ${RENDER_ERRORS[code] ?? "unknown"}`);
  }
}

export const OutMode = { RGBA: 0, PNG: 1 } as const;
export type OutMode = (typeof OutMode)[keyof typeof OutMode];

export type RenderOutput = { bytes: Uint8Array; width: number; height: number };

type Exports = {
  memory: WebAssembly.Memory;
  alloc(len: number): number;
  dealloc(ptr: number, len: number): void;
  render(
    spritesPtr: number,
    spritesLen: number,
    palettePtr: number,
    paletteLen: number,
    statePtr: number,
    stateLen: number,
    assetPtr: number,
    mode: number,
  ): number;
  result_len(): number;
  result_width(): number;
  result_height(): number;
  last_error(): number;
};

export function encodeRenderRequest(req: RenderRequest): Uint8Array {
  if (req.layers.length !== LAYERS_MAX) throw new Error(`RenderRequest needs ${LAYERS_MAX} layers`);
  const out = new Uint8Array(RENDER_REQUEST_LEN);
  const dv = new DataView(out.buffer);
  dv.setUint16(0, req.paletteVersion, true);
  req.layers.forEach((l, i) => {
    const o = 2 + i * 5;
    dv.setUint16(o, l.spriteId, true);
    out[o + 2] = l.materialRamp;
    out[o + 3] = l.accentRamp;
    out[o + 4] = l.elementRamp;
  });
  dv.setUint32(102, req.flags >>> 0, true);
  return out;
}

export function decodeRenderRequest(bytes: Uint8Array): RenderRequest {
  if (bytes.length !== RENDER_REQUEST_LEN) throw new Error("RenderRequest is 106 bytes");
  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  return {
    paletteVersion: dv.getUint16(0, true),
    layers: Array.from({ length: LAYERS_MAX }, (_, i) => {
      const o = 2 + i * 5;
      return { spriteId: dv.getUint16(o, true), materialRamp: bytes[o + 2]!, accentRamp: bytes[o + 3]!, elementRamp: bytes[o + 4]! };
    }),
    flags: dv.getUint32(102, true),
  };
}

/** The `sprites` argument (FORMAT.md section 7): each sprite book as `u32 len + body`, in lookup order. */
export function encodeBooks(books: Uint8Array[]): Uint8Array {
  return concat(...books.flatMap((b) => [u32le(b.length), b]));
}

/** Asset address bytes; unused by format 2 (zeros). */
export const ASSET_LEN = 32;

export class Rasterizer {
  private constructor(private readonly x: Exports) {}

  static async instantiate(wasm: Uint8Array): Promise<Rasterizer> {
    const { instance } = await WebAssembly.instantiate(wasm as BufferSource, {});
    const x = instance.exports as unknown as Exports;
    for (const name of ["memory", "alloc", "dealloc", "render", "result_len", "result_width", "result_height", "last_error"]) {
      if (!(name in x)) throw new Error(`rasterizer wasm is missing export ${name}`);
    }
    return new Rasterizer(x);
  }

  /**
   * `render(sprites, palette, state, asset, mode)` of ART_DESCRIPTION.md 2.2 on raw bytes. The four
   * arguments go into one WASM allocation. Output is copied out of WASM memory.
   */
  renderRaw(sprites: Uint8Array, palette: Uint8Array, state: Uint8Array, mode: OutMode, asset = new Uint8Array(ASSET_LEN)): RenderOutput {
    const { x } = this;
    if (asset.length !== ASSET_LEN) throw new Error(`asset must be ${ASSET_LEN} bytes`);
    const parts = [sprites, palette, state, asset];
    const total = parts.reduce((n, p) => n + p.length, 0);
    const base = x.alloc(total);
    if (base === 0) throw new Error("rasterizer alloc failed");
    const ptrs: number[] = [];
    let at = base;
    for (const p of parts) {
      new Uint8Array(x.memory.buffer, at, p.length).set(p);
      ptrs.push(at);
      at += p.length;
    }
    const out = x.render(ptrs[0]!, sprites.length, ptrs[1]!, palette.length, ptrs[2]!, state.length, ptrs[3]!, mode);
    x.dealloc(base, total);
    if (out === 0) throw new RenderError(x.last_error());
    return {
      bytes: new Uint8Array(x.memory.buffer, out, x.result_len()).slice(),
      width: x.result_width(),
      height: x.result_height(),
    };
  }

  /** RGBA: the canvas 1:1 (64 px characters). PNG: enlarged to 512 px on the longer side. */
  render(req: RenderRequest, palette: Uint8Array, books: Uint8Array[], mode: OutMode = OutMode.RGBA): RenderOutput {
    return this.renderRaw(encodeBooks(books), palette, encodeRenderRequest(req), mode);
  }
}
