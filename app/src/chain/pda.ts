// PDAs from agents/CONTRACTS.md, section 2: the sdk's generated finders, as addresses.
import { getAddressEncoder, getProgramDerivedAddress, type Address, type ProgramDerivedAddress } from "@solana/kit";
import { PROGRAMS, SESSION_KEYS_PROGRAM } from "@deadroll/sdk";
import { findBagPda, findProfilePda, findSkillExtrasPda, findSkillSetPda } from "@deadroll/sdk/character";
import { findLayoutPda, findMapPda } from "@deadroll/sdk/map";
import { findConfigPda, findTablePda } from "@deadroll/sdk/registry";
import { findPalettePda, findRasterizerPda, findSpriteBookPda } from "@deadroll/sdk/render";
import {
  findEscrowPda,
  findMonstersPda,
  findParticipationPda,
  findProgramIdentityPda,
  findSessionAuthorityPda,
  findSessionPda,
  findRewardCachePda,
} from "@deadroll/sdk/session";

export { SESSION_KEYS_PROGRAM };

const enc = getAddressEncoder();
const at = async (pda: Promise<ProgramDerivedAddress>): Promise<Address> => (await pda)[0];

export const gameConfigPda = () => at(findConfigPda());
export const rasterizerPda = (version: number) => at(findRasterizerPda({ version }));
export const palettePda = (version: number) => at(findPalettePda({ version }));
export const spriteBookPda = (bookId: number) => at(findSpriteBookPda({ bookId }));
/** `["character", owner, index: u8]`: the index comes from the profile, so the IDL has no seeds for it. */
export const characterPda = async (owner: Address, index: number): Promise<Address> =>
  (
    await getProgramDerivedAddress({
      programAddress: PROGRAMS.character,
      seeds: ["character", enc.encode(owner), new Uint8Array([index])],
    })
  )[0];
export const profilePda = (owner: Address) => at(findProfilePda({ owner }));
export const skillSetPda = (character: Address) => at(findSkillSetPda({ character }));
export const bagPda = (character: Address) => at(findBagPda({ character }));
/** Patch 5: range, timing, HP threshold, AP cost and description of every skill. */
export const skillExtrasPda = (character: Address) => at(findSkillExtrasPda({ character }));

export const bestiaryPda = () => at(findTablePda({ seed: "bestiary" }));
export const tilesetPda = () => at(findTablePda({ seed: "tileset" }));
/** Registry resonance tables, copied into the session at open_session (patch 4). */
export const aspectsPda = () => at(findTablePda({ seed: "aspects" }));
export const traitsPda = () => at(findTablePda({ seed: "traits" }));
export const resonancesPda = () => at(findTablePda({ seed: "resonances" }));
/** Patch 6: the skills of the bestiary pools. */
export const monsterSkillsPda = () => at(findTablePda({ seed: "monster_skills" }));
/** Patch 11: reward tables (chests, boss, bodies), read by finalize_map and the builder. */
export const rewardsPda = () => at(findTablePda({ seed: "rewards" }));
export const triplesPda = () => at(findTablePda({ seed: "triples" }));

export const mapPda = (dm: Address, index: number) => at(findMapPda({ dm, index }));
export const layoutPda = (map: Address) => at(findLayoutPda({ map }));

export const sessionPda = (map: Address) => at(findSessionPda({ map }));
export const monstersPda = (session: Address) => at(findMonstersPda({ session }));
export const escrowPda = (session: Address) => at(findEscrowPda({ session }));
export const participationPda = (session: Address, character: Address) => at(findParticipationPda({ session, character }));
/** Patch 11: the character's reward cache after a party, until "Done" (`close_cache`). */
export const rewardCachePda = (character: Address) => at(findRewardCachePda({ character }));
export const sessionAuthorityPda = () => at(findSessionAuthorityPda());
/** The session program's `["identity"]` PDA that signs VRF requests. */
export const programIdentityPda = () => at(findProgramIdentityPda());

/** MagicBlock session-keys `SessionTokenV2` (crate session-keys 3.1.1, `SEED_PREFIX = "session_token_v2"`). */
export const sessionTokenPda = async (targetProgram: Address, sessionSigner: Address, authority: Address): Promise<Address> =>
  (
    await getProgramDerivedAddress({
      programAddress: SESSION_KEYS_PROGRAM,
      seeds: ["session_token_v2", enc.encode(targetProgram), enc.encode(sessionSigner), enc.encode(authority)],
    })
  )[0];
