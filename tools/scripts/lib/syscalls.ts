// Syscalls whose runtime feature is not active on every cluster. A program that calls one passes
// on the local validator (every feature on) and fails on devnet with "unsupported BPF instruction";
// a VRF callback failing that way is dropped by the oracle and the session waits forever (U12).
// The deploy scripts look for these calls in every .so before sending anything.

/** The syscalls and the feature that enables each (`solana feature status`), inactive on devnet as of U12. */
export const GATED_SYSCALLS = [
  { name: "sol_remaining_compute_units", feature: "5TuppMutoyzhUSfuYdhgzD47F92GL1g89KpCZQKqedxP" },
  { name: "sol_blake3", feature: "HTW2pSyErTj4BV6KBM9NZ9VBUJVxt7sacNWcf76wtzb3" },
  { name: "sol_big_mod_exp", feature: "EBq48m8irRKuE7ZnMTLvLg2UuGSqhe8s8oMqnmja1fJw" },
] as const;

export type GatedSyscall = (typeof GATED_SYSCALLS)[number];

/** Murmur3 x86 32-bit, seed 0: the id the SBPF loader gives a syscall name. */
export function syscallHash(name: string): number {
  const b = Buffer.from(name);
  const c1 = 0xcc9e2d51;
  const c2 = 0x1b873593;
  const mix = (k: number) => Math.imul(((Math.imul(k, c1) << 15) | (Math.imul(k, c1) >>> 17)), c2);
  let h = 0;
  let i = 0;
  for (; i + 4 <= b.length; i += 4) {
    h ^= mix(b.readUInt32LE(i));
    h = (h << 13) | (h >>> 19);
    h = (Math.imul(h, 5) + 0xe6546b64) >>> 0;
  }
  let k = 0;
  const rest = b.length & 3;
  if (rest === 3) k ^= b[i + 2] << 16;
  if (rest >= 2) k ^= b[i + 1] << 8;
  if (rest >= 1) h ^= mix(k ^ b[i]);
  h ^= b.length;
  h ^= h >>> 16;
  h = Math.imul(h, 0x85ebca6b);
  h ^= h >>> 13;
  h = Math.imul(h, 0xc2b2ae35);
  h ^= h >>> 16;
  return h >>> 0;
}

const SYSCALL_V3 = 0x95; // SBPF v3 `syscall imm`
const CALL_IMM = 0x85; // SBPF v0-v2 `call imm`

/** The gated syscalls `so` calls: an instruction whose immediate is the name's hash (SBPF v3), or the
 *  name in the ELF string tables (older formats resolve syscalls by symbol). */
export function gatedSyscallsIn(so: Uint8Array): GatedSyscall[] {
  const buf = Buffer.from(so.buffer, so.byteOffset, so.byteLength);
  const hashes = new Map<number, GatedSyscall>(GATED_SYSCALLS.map((s) => [syscallHash(s.name), s]));
  const found = new Set<GatedSyscall>();
  for (let o = 0; o + 8 <= buf.length; o += 8) {
    if (buf[o] !== SYSCALL_V3 && buf[o] !== CALL_IMM) continue;
    const hit = hashes.get(buf.readUInt32LE(o + 4));
    if (hit) found.add(hit);
  }
  const text = buf.toString("latin1");
  for (const s of GATED_SYSCALLS) if (text.includes(`${s.name}\0`)) found.add(s);
  return GATED_SYSCALLS.filter((s) => found.has(s));
}

/** A feature account exists and holds `Some(activated_at)` (bincode: tag 1) once the feature is active. */
export function featureActive(data: Uint8Array | null): boolean {
  return data !== null && data.length >= 9 && data[0] === 1;
}

/** One line per program that calls a syscall `inactive` lists; empty when nothing is wrong. */
export function syscallProblems(programs: { name: string; so: Uint8Array }[], inactive: readonly GatedSyscall[]): string[] {
  const off = new Set(inactive.map((s) => s.name));
  return programs.flatMap(({ name, so }) => {
    const bad = gatedSyscallsIn(so).filter((s) => off.has(s.name));
    return bad.length ? [`${name}.so calls ${bad.map((s) => `${s.name} (feature ${s.feature})`).join(", ")}`] : [];
  });
}
