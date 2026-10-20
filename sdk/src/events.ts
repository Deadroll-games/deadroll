// Anchor events from transaction logs. `emit!` writes "Program data: <base64>" inside the
// emitting program's invocation, so the invoke stack attributes each line to a program.
import { getBase64Encoder, type Address, type ReadonlyUint8Array } from "@solana/kit";
import * as character from "./generated/character/index.js";
import * as map from "./generated/map/index.js";
import * as registry from "./generated/registry/index.js";
import * as render from "./generated/render/index.js";
import * as session from "./generated/session/index.js";
import { programNameOf } from "./errors.js";
import type { ProgramName } from "./programs.js";

type Decoder = { decode(bytes: ReadonlyUint8Array): object };
type EventTable = { names: Record<string, string>; identify(data: ReadonlyUint8Array): string | number; decoders: Record<string, Decoder> };

function table(mod: Record<string, unknown>, events: Record<string, string | number>, identify: (d: ReadonlyUint8Array) => string | number): EventTable {
  const names: Record<string, string> = {};
  const decoders: Record<string, Decoder> = {};
  for (const [name, value] of Object.entries(events)) {
    if (typeof value !== "number") continue;
    names[value] = name;
    decoders[name] = (mod[`get${name}EventDecoder`] as () => Decoder)();
  }
  return { names, identify, decoders };
}

const TABLES: Partial<Record<ProgramName, EventTable>> = {
  registry: table(registry, registry.DeadrollRegistryEvent, registry.identifyDeadrollRegistryEvent),
  render: table(render, render.DeadrollRenderProgramEvent, render.identifyDeadrollRenderProgramEvent),
  character: table(character, character.DeadrollCharacterEvent, character.identifyDeadrollCharacterEvent),
  map: table(map, map.DeadrollMapEvent, map.identifyDeadrollMapEvent),
  session: table(session, session.DeadrollSessionEvent, session.identifyDeadrollSessionEvent),
};

/** A decoded event: `name` is the IDL event name (`PlayerDied`), `data` its generated type. */
export type ChainEvent = { program: ProgramName; name: string; data: any };

/** Decodes one event payload (discriminator first) of `program`, or null if it is not one of its events. */
export function decodeEvent(program: ProgramName, payload: ReadonlyUint8Array): Omit<ChainEvent, "program"> | null {
  const t = TABLES[program];
  if (!t) return null;
  let kind: string | number;
  try {
    kind = t.identify(payload);
  } catch {
    return null;
  }
  const name = t.names[kind as number]!;
  return { name, data: t.decoders[name]!.decode(payload) };
}

const INVOKE = /^Program (\w+) invoke \[\d+\]$/;
const DONE = /^Program (\w+) (success|failed)/;
const DATA = "Program data: ";
const base64 = getBase64Encoder();

function eachData(logs: readonly string[], visit: (program: string, payload: ReadonlyUint8Array) => void): void {
  const stack: string[] = [];
  for (const line of logs) {
    const inv = INVOKE.exec(line);
    if (inv) {
      stack.push(inv[1]!);
      continue;
    }
    if (DONE.test(line)) {
      stack.pop();
      continue;
    }
    if (!line.startsWith(DATA)) continue;
    let payload: ReadonlyUint8Array;
    try {
      payload = base64.encode(line.slice(DATA.length).trim());
    } catch {
      continue; // not base64
    }
    visit(stack[stack.length - 1] ?? "", payload);
  }
}

/** Raw event payloads (discriminator first) emitted directly by `program`. */
export function eventsFromLogs(logs: readonly string[], program: Address): ReadonlyUint8Array[] {
  const out: ReadonlyUint8Array[] = [];
  eachData(logs, (p, payload) => {
    if (p === program) out.push(payload);
  });
  return out;
}

/** Every Deadroll event in a transaction's logs, in log order. */
export function decodeEvents(logs: readonly string[]): ChainEvent[] {
  const out: ChainEvent[] = [];
  eachData(logs, (p, payload) => {
    const program = programNameOf(p);
    const ev = program ? decodeEvent(program, payload) : null;
    if (program && ev) out.push({ program, ...ev });
  });
  return out;
}
