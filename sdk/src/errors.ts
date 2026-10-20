import type { Address } from "@solana/kit";
import { PROGRAM_ERRORS } from "./generated/errors.js";
import { PROGRAMS, type ProgramName } from "./programs.js";

export { PROGRAM_ERRORS };

const BY_ADDRESS = new Map<string, ProgramName>(Object.entries(PROGRAMS).map(([k, v]) => [v, k as ProgramName]));

/** The Deadroll program at `address`, or null for any other program. */
export function programNameOf(address: Address | string): ProgramName | null {
  return BY_ADDRESS.get(address) ?? null;
}

/** Anchor error name (`AlreadyActed`) of a custom error code of `program`, or null. */
export function errorName(program: ProgramName, code: number): string | null {
  const table = PROGRAM_ERRORS[program] as Record<number, readonly [string, string]>;
  return table[code]?.[0] ?? null;
}

/** The `#[msg]` text of a custom error code of `program`, or null. */
export function errorMessage(program: ProgramName, code: number): string | null {
  const table = PROGRAM_ERRORS[program] as Record<number, readonly [string, string]>;
  return table[code]?.[1] ?? null;
}
