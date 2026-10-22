/** Points still to spend: budget minus how far each stat sits above the minimum. */
export function pointsLeft(stats: number[], statMin: number, statPoints: number): number {
  const spent = stats.reduce((sum, stat) => sum + (stat - statMin), 0);
  return statPoints - spent;
}

/** UTF-8 byte length, the same limit the chain uses for names. */
export function nameBytes(name: string): number {
  return new TextEncoder().encode(name).length;
}

/** Same rule as programs/character/src/rules.rs name_valid: 1..=32 printable ASCII, no leading space. */
export function nameValid(name: string): boolean {
  return /^[!-~][ -~]{0,31}$/.test(name);
}

/**
 * Lamports as SOL. Truncates to 4 decimal places and strips trailing zeros.
 * Bigint math only.
 */
export function formatSol(lamports: bigint): string {
  const whole = lamports / 1_000_000_000n;
  const frac = lamports % 1_000_000_000n;
  if (frac === 0n) return whole.toString();
  const digits = frac.toString().padStart(9, "0").slice(0, 4).replace(/0+$/, "");
  if (digits.length === 0) return whole.toString();
  return `${whole.toString()}.${digits}`;
}

/** Lamports as SOL to the last lamport (9 decimal places, trailing zeros stripped): what a
 * confirmation shows before the wallet signs. */
export function formatSolExact(lamports: bigint): string {
  const whole = lamports / 1_000_000_000n;
  const digits = (lamports % 1_000_000_000n).toString().padStart(9, "0").replace(/0+$/, "");
  return digits.length === 0 ? whole.toString() : `${whole.toString()}.${digits}`;
}

/** First 4 and last 4 characters, for wallet addresses in the lobby. */
export function shortAddress(a: string): string {
  return `${a.slice(0, 4)}..${a.slice(-4)}`;
}
