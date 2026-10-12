// PDAs from agents/CONTRACTS.md, section 2.
import { getAddressEncoder, getProgramDerivedAddress, type Address } from "@solana/kit";
import { PROGRAMS } from "../config";
import { u16le } from "./bytes";

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

/** MagicBlock session-keys `SessionTokenV2` (crate session-keys 3.1.1). */
export const SESSION_KEYS_PROGRAM = "KeyspM2ssCJbqUhQ4k7sveSiY4WjnYsrXkC8oDbwde5" as Address;
export const sessionTokenPda = (targetProgram: Address, sessionSigner: Address, authority: Address) =>
  pda(SESSION_KEYS_PROGRAM, [
    "session_token",
    enc.encode(targetProgram) as Uint8Array,
    enc.encode(sessionSigner) as Uint8Array,
    enc.encode(authority) as Uint8Array,
  ]);
