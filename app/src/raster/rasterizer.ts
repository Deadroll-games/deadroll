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
  9: "bad render request, output mode or scale",
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
  render(ptr: number, len: number, mode: number, scale: number): number;
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

/** Input layout of FORMAT.md section 7. The 32-byte asset address is zeros in format 1. */
export function buildRenderInput(req: RenderRequest, palette: Uint8Array, books: Uint8Array[]): Uint8Array {
  return concat(
    u32le(RENDER_REQUEST_LEN),
    encodeRenderRequest(req),
    new Uint8Array(32),
    u32le(palette.length),
    palette,
    ...books.flatMap((b) => [u32le(b.length), b]),
  );
}

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

  /** Raw call; `input` from buildRenderInput. Output is copied out of WASM memory. */
  renderRaw(input: Uint8Array, mode: OutMode, scale: number): RenderOutput {
    const { x } = this;
    const ptr = x.alloc(input.length);
    if (ptr === 0 && input.length > 0) throw new Error("rasterizer alloc failed");
    new Uint8Array(x.memory.buffer, ptr, input.length).set(input);
    const out = x.render(ptr, input.length, mode, scale);
    x.dealloc(ptr, input.length);
    if (out === 0) throw new RenderError(x.last_error());
    return {
      bytes: new Uint8Array(x.memory.buffer, out, x.result_len()).slice(),
      width: x.result_width(),
      height: x.result_height(),
    };
  }

  render(req: RenderRequest, palette: Uint8Array, books: Uint8Array[], mode: OutMode = OutMode.RGBA, scale = 1): RenderOutput {
    return this.renderRaw(buildRenderInput(req, palette, books), mode, scale);
  }
}
