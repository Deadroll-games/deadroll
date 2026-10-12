// Little-endian readers and small byte helpers. All on-chain integers are LE (FORMAT.md, borsh).
import { getAddressDecoder, type Address } from "@solana/kit";

const addressDecoder = getAddressDecoder();

export class Reader {
  private pos: number;
  constructor(
    private readonly data: Uint8Array,
    start = 0,
  ) {
    this.pos = start;
  }
  get offset(): number {
    return this.pos;
  }
  private need(n: number): void {
    if (this.pos + n > this.data.length) {
      throw new Error(`account data too short: need ${this.pos + n} bytes, have ${this.data.length}`);
    }
  }
  u8(): number {
    this.need(1);
    return this.data[this.pos++]!;
  }
  bool(): boolean {
    const v = this.u8();
    if (v > 1) throw new Error(`invalid bool ${v}`);
    return v === 1;
  }
  u16(): number {
    this.need(2);
    const v = this.data[this.pos]! | (this.data[this.pos + 1]! << 8);
    this.pos += 2;
    return v;
  }
  u32(): number {
    this.need(4);
    const d = this.data;
    const p = this.pos;
    this.pos += 4;
    return (d[p]! | (d[p + 1]! << 8) | (d[p + 2]! << 16)) + d[p + 3]! * 0x1000000;
  }
  u64(): bigint {
    const lo = BigInt(this.u32());
    const hi = BigInt(this.u32());
    return (hi << 32n) | lo;
  }
  i64(): bigint {
    return BigInt.asIntN(64, this.u64());
  }
  bytes(n: number): Uint8Array {
    this.need(n);
    const out = this.data.slice(this.pos, this.pos + n);
    this.pos += n;
    return out;
  }
  address(): Address {
    return addressDecoder.decode(this.bytes(32));
  }
  option<T>(read: () => T): T | null {
    const tag = this.u8();
    if (tag === 0) return null;
    if (tag !== 1) throw new Error(`invalid option tag ${tag}`);
    return read();
  }
}

export function u16le(n: number): Uint8Array {
  return new Uint8Array([n & 0xff, (n >>> 8) & 0xff]);
}

export function u32le(n: number): Uint8Array {
  return new Uint8Array([n & 0xff, (n >>> 8) & 0xff, (n >>> 16) & 0xff, (n >>> 24) & 0xff]);
}

export function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

export function equalBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

export function startsWith(data: Uint8Array, prefix: Uint8Array): boolean {
  return data.length >= prefix.length && equalBytes(data.subarray(0, prefix.length), prefix);
}

export function toHex(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += b.toString(16).padStart(2, "0");
  return s;
}

export function fromHex(hex: string): Uint8Array {
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  return out;
}

export async function sha256(bytes: Uint8Array): Promise<Uint8Array> {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", bytes as BufferSource));
}

export function fromBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}
