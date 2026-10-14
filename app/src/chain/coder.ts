// Borsh coder driven by the Anchor IDLs in ./idl (snapshot of target/idl, the same files
// tests/demo.test.ts loads). Replaced by the Codama client once sdk/ (L8.1) exists.
//
// JS shapes follow Anchor's TS coder so code reads like the tests: field names in camelCase,
// u8..u32 / u16 as number, u64 / i64 as bigint, pubkey as Address, Option as value | null, enums as
// `{ variantName: {...fields} }` (e.g. `s.core.status.active`). Encoding also accepts a bare
// variant name string ("human") or a variant index for unit enums.
import { getAddressDecoder, getAddressEncoder, type Address } from "@solana/kit";
import { concat } from "./bytes";

export type IdlType =
  | string
  | { array: [IdlType, number] }
  | { option: IdlType }
  | { vec: IdlType }
  | { defined: { name: string } };
type IdlField = { name: string; type: IdlType };
type IdlTypeDef = {
  name: string;
  type:
    | { kind: "struct"; fields?: IdlField[] }
    | { kind: "enum"; variants: { name: string; fields?: (IdlField | IdlType)[] }[] };
};
type IdlAccountItem = { name: string; writable?: boolean; signer?: boolean; optional?: boolean };
export type Idl = {
  address: string;
  instructions: { name: string; discriminator: number[]; accounts: IdlAccountItem[]; args: IdlField[] }[];
  accounts?: { name: string; discriminator: number[] }[];
  events?: { name: string; discriminator: number[] }[];
  errors?: { code: number; name: string; msg?: string }[];
  /** Absent when a program declares no types (the items program for now). */
  types?: IdlTypeDef[];
};

export const camel = (s: string): string => s.replace(/_([a-z0-9])/g, (_, c: string) => c.toUpperCase());
const lowerFirst = (s: string): string => s.charAt(0).toLowerCase() + s.slice(1);

const addrEnc = getAddressEncoder();
const addrDec = getAddressDecoder();

class Reader {
  o = 0;
  constructor(private readonly b: Uint8Array) {}
  private view = (n: number) => {
    if (this.o + n > this.b.length) throw new Error(`borsh: read past end (${this.o}+${n} > ${this.b.length})`);
    const v = new DataView(this.b.buffer, this.b.byteOffset + this.o, n);
    this.o += n;
    return v;
  };
  u8 = () => this.view(1).getUint8(0);
  u16 = () => this.view(2).getUint16(0, true);
  u32 = () => this.view(4).getUint32(0, true);
  u64 = () => this.view(8).getBigUint64(0, true);
  i64 = () => this.view(8).getBigInt64(0, true);
  bytes = (n: number) => {
    this.view(n);
    return this.b.slice(this.o - n, this.o);
  };
}

export class Coder {
  private readonly types = new Map<string, IdlTypeDef>();
  constructor(readonly idl: Idl) {
    for (const t of idl.types ?? []) this.types.set(t.name, t);
  }

  private def(name: string): IdlTypeDef {
    const t = this.types.get(name);
    if (!t) throw new Error(`idl ${this.idl.address}: unknown type ${name}`);
    return t;
  }

  // ---- encode

  encode(type: IdlType, value: unknown, out: Uint8Array[]): void {
    if (typeof type === "string") return this.encodePrim(type, value, out);
    if ("array" in type) {
      const [inner, len] = type.array;
      if (inner === "u8" && value instanceof Uint8Array) {
        if (value.length !== len) throw new Error(`borsh: [u8; ${len}] got ${value.length} bytes`);
        out.push(value);
        return;
      }
      const arr = value as unknown[];
      if (!Array.isArray(arr) || arr.length !== len) throw new Error(`borsh: array of ${len} expected, got ${arr?.length}`);
      for (const v of arr) this.encode(inner, v, out);
      return;
    }
    if ("option" in type) {
      if (value === null || value === undefined) out.push(Uint8Array.of(0));
      else {
        out.push(Uint8Array.of(1));
        this.encode(type.option, value, out);
      }
      return;
    }
    if ("vec" in type) {
      const arr = value as unknown[];
      this.encodePrim("u32", arr.length, out);
      for (const v of arr) this.encode(type.vec, v, out);
      return;
    }
    const t = this.def(type.defined.name);
    if (t.type.kind === "struct") {
      const obj = value as Record<string, unknown>;
      for (const f of t.type.fields ?? []) {
        const v = obj[camel(f.name)];
        if (v === undefined) throw new Error(`borsh: ${t.name}.${camel(f.name)} missing`);
        this.encode(f.type, v, out);
      }
      return;
    }
    const variants = t.type.variants;
    let index: number;
    let fields: Record<string, unknown> = {};
    if (typeof value === "number") index = value;
    else if (typeof value === "string") index = variants.findIndex((v) => lowerFirst(v.name) === lowerFirst(value));
    else {
      const key = Object.keys(value as object)[0] ?? "";
      index = variants.findIndex((v) => lowerFirst(v.name) === lowerFirst(key));
      fields = ((value as Record<string, unknown>)[key] as Record<string, unknown>) ?? {};
    }
    const variant = variants[index];
    if (!variant) throw new Error(`borsh: ${t.name} has no variant ${JSON.stringify(value)}`);
    out.push(Uint8Array.of(index));
    (variant.fields ?? []).forEach((f, i) => {
      if (typeof f === "object" && "name" in f) this.encode(f.type, fields[camel(f.name)], out);
      else this.encode(f as IdlType, (fields as unknown as unknown[])[i], out);
    });
  }

  private encodePrim(type: string, value: unknown, out: Uint8Array[]): void {
    const num = (n: number) => {
      const b = new Uint8Array(n);
      return [b, new DataView(b.buffer)] as const;
    };
    switch (type) {
      case "bool":
        out.push(Uint8Array.of(value ? 1 : 0));
        return;
      case "u8": {
        const [b, v] = num(1);
        v.setUint8(0, Number(value));
        out.push(b);
        return;
      }
      case "u16": {
        const [b, v] = num(2);
        v.setUint16(0, Number(value), true);
        out.push(b);
        return;
      }
      case "u32": {
        const [b, v] = num(4);
        v.setUint32(0, Number(value), true);
        out.push(b);
        return;
      }
      case "u64": {
        const [b, v] = num(8);
        v.setBigUint64(0, BigInt(value as bigint | number), true);
        out.push(b);
        return;
      }
      case "i64": {
        const [b, v] = num(8);
        v.setBigInt64(0, BigInt(value as bigint | number), true);
        out.push(b);
        return;
      }
      case "pubkey":
        out.push(addrEnc.encode(value as Address) as Uint8Array);
        return;
      case "bytes": {
        const bytes = value as Uint8Array;
        this.encodePrim("u32", bytes.length, out);
        out.push(bytes);
        return;
      }
      default:
        throw new Error(`borsh: unsupported type ${type}`);
    }
  }

  // ---- decode

  decode(type: IdlType, r: Reader): unknown {
    if (typeof type === "string") {
      switch (type) {
        case "bool":
          return r.u8() !== 0;
        case "u8":
          return r.u8();
        case "u16":
          return r.u16();
        case "u32":
          return r.u32();
        case "u64":
          return r.u64();
        case "i64":
          return r.i64();
        case "pubkey":
          return addrDec.decode(r.bytes(32));
        case "bytes":
          return r.bytes(r.u32());
        default:
          throw new Error(`borsh: unsupported type ${type}`);
      }
    }
    if ("array" in type) {
      const [inner, len] = type.array;
      if (inner === "u8") return r.bytes(len);
      return Array.from({ length: len }, () => this.decode(inner, r));
    }
    if ("option" in type) return r.u8() === 0 ? null : this.decode(type.option, r);
    if ("vec" in type) {
      const n = r.u32();
      return Array.from({ length: n }, () => this.decode(type.vec, r));
    }
    const t = this.def(type.defined.name);
    if (t.type.kind === "struct") {
      const obj: Record<string, unknown> = {};
      for (const f of t.type.fields ?? []) obj[camel(f.name)] = this.decode(f.type, r);
      return obj;
    }
    const index = r.u8();
    const variant = t.type.variants[index];
    if (!variant) throw new Error(`borsh: ${t.name} variant ${index} out of range`);
    const fields: Record<string, unknown> = {};
    (variant.fields ?? []).forEach((f, i) => {
      if (typeof f === "object" && "name" in f) fields[camel(f.name)] = this.decode(f.type, r);
      else fields[String(i)] = this.decode(f as IdlType, r);
    });
    return { [lowerFirst(variant.name)]: fields };
  }

  // ---- instructions, accounts, events

  instructionData(name: string, args: Record<string, unknown>): Uint8Array {
    const ix = this.idl.instructions.find((i) => i.name === name);
    if (!ix) throw new Error(`idl ${this.idl.address}: no instruction ${name}`);
    const out: Uint8Array[] = [Uint8Array.from(ix.discriminator)];
    for (const a of ix.args) {
      const v = args[camel(a.name)];
      if (v === undefined) throw new Error(`${name}: arg ${camel(a.name)} missing`);
      this.encode(a.type, v, out);
    }
    return concat(...out);
  }

  instructionAccounts(name: string): IdlAccountItem[] {
    const ix = this.idl.instructions.find((i) => i.name === name);
    if (!ix) throw new Error(`idl ${this.idl.address}: no instruction ${name}`);
    return ix.accounts;
  }

  accountDiscriminator(name: string): Uint8Array {
    const a = this.idl.accounts?.find((x) => x.name === name);
    if (!a) throw new Error(`idl ${this.idl.address}: no account ${name}`);
    return Uint8Array.from(a.discriminator);
  }

  /** Decodes an account by type name; throws on a wrong discriminator. */
  decodeAccount<T = any>(name: string, data: Uint8Array): T {
    const disc = this.accountDiscriminator(name);
    if (data.length < 8 || disc.some((b, i) => data[i] !== b)) throw new Error(`not a ${name} account`);
    return this.decode({ defined: { name } }, new Reader(data.subarray(8))) as T;
  }

  /** An event payload (discriminator first) → `{ name, data }`, or null if not an event of this IDL. */
  decodeEvent(payload: Uint8Array): { name: string; data: any } | null {
    for (const ev of this.idl.events ?? []) {
      if (payload.length >= 8 && ev.discriminator.every((b, i) => payload[i] === b)) {
        try {
          return { name: ev.name, data: this.decode({ defined: { name: ev.name } }, new Reader(payload.subarray(8))) };
        } catch {
          return null;
        }
      }
    }
    return null;
  }

  errorName(code: number): string | null {
    const e = this.idl.errors?.find((x) => x.code === code);
    return e ? `${e.name}${e.msg ? `: ${e.msg}` : ""}` : null;
  }
}

/** Index of a unit enum value decoded as `{ variant: {} }`. */
export function variantIndex(coder: Coder, typeName: string, value: unknown): number {
  const t = coder.idl.types?.find((x) => x.name === typeName);
  if (!t || t.type.kind !== "enum") throw new Error(`${typeName} is not an enum`);
  const key = typeof value === "string" ? value : Object.keys(value as object)[0] ?? "";
  return t.type.variants.findIndex((v) => lowerFirst(v.name) === lowerFirst(key));
}

export function variantName(value: unknown): string {
  return Object.keys(value as object)[0] ?? "";
}
