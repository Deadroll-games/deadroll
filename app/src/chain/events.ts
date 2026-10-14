// Anchor events from transaction logs. `emit!` writes "Program data: <base64>" inside the
// emitting program's invocation, so we track the invoke stack to attribute each line.
import type { Address } from "@solana/kit";
import { fromBase64 } from "./bytes";

const INVOKE = /^Program (\w+) invoke \[\d+\]$/;
const DONE = /^Program (\w+) (success|failed)/;
const DATA = "Program data: ";

/** Raw event payloads (discriminator first) emitted directly by `program`. */
export function eventsFromLogs(logs: readonly string[], program: Address): Uint8Array[] {
  const stack: string[] = [];
  const out: Uint8Array[] = [];
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
    if (line.startsWith(DATA) && stack[stack.length - 1] === program) {
      try {
        out.push(fromBase64(line.slice(DATA.length).trim()));
      } catch {
        // not base64: ignore
      }
    }
  }
  return out;
}

export type ChainEvent = { program: "session" | "character" | "items" | "map"; name: string; data: any };

/** Every session, character, items and map event in a transaction's logs, in log order. */
export function decodeEvents(logs: readonly string[], coders: Record<ChainEvent["program"], { decodeEvent(p: Uint8Array): { name: string; data: any } | null }>, ids: Record<ChainEvent["program"], Address>): ChainEvent[] {
  const out: ChainEvent[] = [];
  const stack: string[] = [];
  const byId = new Map(Object.entries(ids).map(([k, v]) => [v as string, k as ChainEvent["program"]]));
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
    const program = byId.get(stack[stack.length - 1] ?? "");
    if (!program) continue;
    try {
      const ev = coders[program].decodeEvent(fromBase64(line.slice(DATA.length).trim()));
      if (ev) out.push({ program, ...ev });
    } catch {
      // not base64: ignore
    }
  }
  return out;
}
