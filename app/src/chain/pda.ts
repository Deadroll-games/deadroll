// PDAs from agents/CONTRACTS.md, section 2.
import { getAddressEncoder, getProgramDerivedAddress, type Address } from "@solana/kit";
import { PROGRAMS } from "../config";
import { u16le, u32le } from "./bytes";

const enc = getAddressEncoder();
const text = (s: string) => new TextEncoder().encode(s);

async function pda(program: Address, seeds: (Uint8Array | string)[]): Promise<Address> {
  const [addr] = await getProgramDerivedAddress({
    programAddress: program,
    seeds: seeds.map((s) => (typeof s === "string" ? text(s) : s)),
  });
  return addr;
}

export const gameConfigPda = () => pda(PROGRAMS.registry, ["config"]);
export const rasterizerPda = (version: number) => pda(PROGRAMS.render, ["rasterizer", u16le(version)]);
export const palettePda = (version: number) => pda(PROGRAMS.render, ["palette", u16le(version)]);
export const spriteBookPda = (bookId: number) => pda(PROGRAMS.render, ["sprites", u16le(bookId)]);
export const characterPda = (owner: Address, index: number) =>
  pda(PROGRAMS.character, ["character", enc.encode(owner) as Uint8Array, new Uint8Array([index])]);
export const profilePda = (owner: Address) => pda(PROGRAMS.character, ["profile", enc.encode(owner) as Uint8Array]);
export const skillSetPda = (character: Address) => pda(PROGRAMS.character, ["skills", enc.encode(character) as Uint8Array]);
export const bagPda = (character: Address) => pda(PROGRAMS.character, ["bag", enc.encode(character) as Uint8Array]);
export const loadoutPda = (character: Address) => pda(PROGRAMS.items, ["loadout", enc.encode(character) as Uint8Array]);

export const bestiaryPda = () => pda(PROGRAMS.registry, ["bestiary"]);
export const tilesetPda = () => pda(PROGRAMS.registry, ["tileset"]);

export const mapPda = (dm: Address, index: number) => pda(PROGRAMS.map, ["map", enc.encode(dm) as Uint8Array, u32le(index)]);
export const layoutPda = (map: Address) => pda(PROGRAMS.map, ["layout", enc.encode(map) as Uint8Array]);

export const sessionPda = (map: Address) => pda(PROGRAMS.session, ["session", enc.encode(map) as Uint8Array]);
export const monstersPda = (session: Address) => pda(PROGRAMS.session, ["monsters", enc.encode(session) as Uint8Array]);
export const escrowPda = (session: Address) => pda(PROGRAMS.session, ["escrow", enc.encode(session) as Uint8Array]);
export const participationPda = (session: Address, character: Address) =>
  pda(PROGRAMS.session, ["participation", enc.encode(session) as Uint8Array, enc.encode(character) as Uint8Array]);
export const stashPda = (session: Address, character: Address) =>
  pda(PROGRAMS.session, ["stash", enc.encode(session) as Uint8Array, enc.encode(character) as Uint8Array]);
export const sessionAuthorityPda = () => pda(PROGRAMS.session, ["authority"]);
/** The session program's `["identity"]` PDA that signs VRF requests. */
export const programIdentityPda = () => pda(PROGRAMS.session, ["identity"]);

/** MagicBlock session-keys `SessionTokenV2` (crate session-keys 3.1.1, `SEED_PREFIX = "session_token_v2"`). */
export const SESSION_KEYS_PROGRAM = "KeyspM2ssCJbqUhQ4k7sveSiY4WjnYsrXkC8oDbwde5" as Address;
export const sessionTokenPda = (targetProgram: Address, sessionSigner: Address, authority: Address) =>
  pda(SESSION_KEYS_PROGRAM, [
    "session_token_v2",
    enc.encode(targetProgram) as Uint8Array,
    enc.encode(sessionSigner) as Uint8Array,
    enc.encode(authority) as Uint8Array,
  ]);
