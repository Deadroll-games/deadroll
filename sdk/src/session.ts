export * from "./generated/session/index.js";

// Patch D: the `SessionBoard` (`["board", session]`) holds what a party uncovered, sized for the
// largest map; the generated decoder reads its header only. Offsets: `board_off` in
// programs/session/src/state/session_board.rs.

/** `SECRET_LEAVES_CAP`, `SECRETS_CAP`, `MONSTERS_CAP` (crates/types). */
export const SECRET_LEAVES_CAP = 40 + 143 * 143 + 40 * 144;
export const SECRETS_CAP = 96;
export const MONSTERS_CAP = 80;
const REVEALED = 8 + 1 + 32 + 2;
const REVEALED_BYTES = Math.ceil(SECRET_LEAVES_CAP / 8);
const FOUND = REVEALED + REVEALED_BYTES;
const SPAWNS = FOUND + SECRETS_CAP * 6;

export type BoardSecret = { leaf: number; secret: { kind: number; a: number; b: number; c: number } };
export type BoardSpawn = { bestiaryId: number; x: number; y: number; level: number; skills: number; flags: number; aspects: number; pack: number };

/** A `SessionBoard`, read whole: the revealed leaves, the secrets found, the revealed spawns. */
export type SessionBoardV1 = {
  revealed: (leaf: number) => boolean;
  found: BoardSecret[];
  spawns: BoardSpawn[];
};

export function decodeSessionBoard(raw: Uint8Array): SessionBoardV1 {
  const foundCount = Math.min(raw[41]!, SECRETS_CAP);
  const spawnCount = Math.min(raw[42]!, MONSTERS_CAP);
  return {
    revealed: (leaf) => leaf >= 0 && leaf < SECRET_LEAVES_CAP && (raw[REVEALED + (leaf >> 3)]! & (1 << (leaf & 7))) !== 0,
    found: Array.from({ length: foundCount }, (_, i) => {
      const at = FOUND + i * 6;
      return { leaf: raw[at]! | (raw[at + 1]! << 8), secret: { kind: raw[at + 2]!, a: raw[at + 3]!, b: raw[at + 4]!, c: raw[at + 5]! } };
    }),
    spawns: Array.from({ length: spawnCount }, (_, i) => {
      const at = SPAWNS + i * 9;
      return {
        bestiaryId: raw[at]!,
        x: raw[at + 1]!,
        y: raw[at + 2]!,
        level: raw[at + 3]!,
        skills: raw[at + 4]! | (raw[at + 5]! << 8),
        flags: raw[at + 6]!,
        aspects: raw[at + 7]!,
        pack: raw[at + 8]!,
      };
    }),
  };
}
