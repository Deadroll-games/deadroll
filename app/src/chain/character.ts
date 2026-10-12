// Character program client (thin wrapper until sdk/ from L8).
import { AccountRole, type AccountMeta, type AccountSignerMeta, type Address, type Instruction, type TransactionSigner } from "@solana/kit";
import { MPL_CORE_PROGRAM, PROGRAMS, SYSTEM_PROGRAM } from "../config";
import { IX_DISC } from "./disc";

/**
 * `character.freeze_tombstone()`: permissionless, sets the permanent freeze on a dead
 * character's Core NFT. Accounts in the order of programs/character/src/instructions/freeze_tombstone.rs.
 */
export function freezeTombstoneInstruction(args: {
  character: Address;
  asset: Address;
  payer: TransactionSigner;
}): Instruction {
  const accounts: (AccountMeta | AccountSignerMeta)[] = [
    { address: args.character, role: AccountRole.WRITABLE },
    { address: args.asset, role: AccountRole.WRITABLE },
    { address: args.payer.address, role: AccountRole.WRITABLE_SIGNER, signer: args.payer },
    { address: MPL_CORE_PROGRAM, role: AccountRole.READONLY },
    { address: SYSTEM_PROGRAM, role: AccountRole.READONLY },
  ];
  return { programAddress: PROGRAMS.character, accounts, data: IX_DISC.freeze_tombstone };
}
