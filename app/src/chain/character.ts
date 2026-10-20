// Character program client: the sdk's generated builders.
import type { Address, Instruction, TransactionSigner } from "@solana/kit";
import { getFreezeTombstoneInstruction } from "@deadroll/sdk/character";
import { MPL_CORE_PROGRAM, SYSTEM_PROGRAM } from "../config";

/**
 * `character.freeze_tombstone()`: permissionless, sets the permanent freeze on a dead
 * character's Core NFT.
 */
export function freezeTombstoneInstruction(args: { character: Address; asset: Address; payer: TransactionSigner }): Instruction {
  return getFreezeTombstoneInstruction({ ...args, mplCoreProgram: MPL_CORE_PROGRAM, systemProgram: SYSTEM_PROGRAM });
}
