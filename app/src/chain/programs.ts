// Instruction builders for character, items, map and session, from the IDL snapshot. Account names and
// order are the IDL's (camelCase, as `accountsStrict` in tests/demo.test.ts); every account must
// be given, optional ones as null (Anchor passes the program id for None).
import { AccountRole, type AccountMeta, type AccountSignerMeta, type Address, type Instruction, type TransactionSigner } from "@solana/kit";
import { PROGRAMS } from "../config";
import { Coder, camel, type Idl } from "./coder";
import characterIdl from "./idl/deadroll_character.json";
import itemsIdl from "./idl/deadroll_items.json";
import mapIdl from "./idl/deadroll_map.json";
import sessionIdl from "./idl/deadroll_session.json";

export const coders = {
  character: new Coder(characterIdl as unknown as Idl),
  items: new Coder(itemsIdl as unknown as Idl),
  map: new Coder(mapIdl as unknown as Idl),
  session: new Coder(sessionIdl as unknown as Idl),
} as const;
export type ProgramName = keyof typeof coders;

/** An account value: an address, or a signer for signer accounts; null for an absent optional. */
export type AccountInput = Address | TransactionSigner | null;

const isSigner = (v: AccountInput): v is TransactionSigner => typeof v === "object" && v !== null && "address" in v;

export function buildInstruction(
  program: ProgramName,
  name: string,
  args: Record<string, unknown>,
  accounts: Record<string, AccountInput>,
): Instruction {
  const coder = coders[program];
  const programAddress = PROGRAMS[program];
  const metas: (AccountMeta | AccountSignerMeta)[] = coder.instructionAccounts(name).map((a) => {
    const key = camel(a.name);
    if (!(key in accounts)) throw new Error(`${program}.${name}: account ${key} missing`);
    const v = accounts[key] ?? null;
    if (v === null) {
      if (!a.optional) throw new Error(`${program}.${name}: account ${key} is required`);
      return { address: programAddress, role: AccountRole.READONLY };
    }
    if (a.signer) {
      if (!isSigner(v)) throw new Error(`${program}.${name}: account ${key} must be a signer`);
      return { address: v.address, role: a.writable ? AccountRole.WRITABLE_SIGNER : AccountRole.READONLY_SIGNER, signer: v };
    }
    const address = isSigner(v) ? v.address : v;
    return { address, role: a.writable ? AccountRole.WRITABLE : AccountRole.READONLY };
  });
  for (const key of Object.keys(accounts)) {
    if (!coder.instructionAccounts(name).some((a) => camel(a.name) === key)) throw new Error(`${program}.${name}: unknown account ${key}`);
  }
  return { programAddress, accounts: metas, data: coder.instructionData(name, args) };
}

