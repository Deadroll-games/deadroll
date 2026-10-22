// System program `transfer`: the session key's prepayment top-up and its refund (docs/GAME_COMMISSIONS.md 6.2).
import { AccountRole, type AccountMeta, type AccountSignerMeta, type Address, type Instruction, type TransactionSigner } from "@solana/kit";
import { SYSTEM_PROGRAM } from "../config";

/** `SystemInstruction::Transfer` (index 2, u32 LE) and the lamports (u64 LE). */
export function transferData(lamports: bigint): Uint8Array {
  const out = new Uint8Array(12);
  const view = new DataView(out.buffer);
  view.setUint32(0, 2, true);
  view.setBigUint64(4, lamports, true);
  return out;
}

export function transferSolIx(from: TransactionSigner, to: Address, lamports: bigint): Instruction {
  const accounts: (AccountMeta | AccountSignerMeta)[] = [
    { address: from.address, role: AccountRole.WRITABLE_SIGNER, signer: from },
    { address: to, role: AccountRole.WRITABLE },
  ];
  return { programAddress: SYSTEM_PROGRAM, accounts, data: transferData(lamports) };
}
