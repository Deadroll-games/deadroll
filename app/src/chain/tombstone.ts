// After CharacterDied the app sends character.freeze_tombstone (permissionless, split from
// mark_dead to keep the VRF callback at CPI depth 2). The owner's app sends at once; any other
// connected app waits and sends only if the NFT is still not frozen, so an offline owner does not
// leave a tombstone unfrozen.
import type { Address, TransactionSigner } from "@solana/kit";
import type { AccountSource } from "./accounts";
import { freezeTombstoneInstruction } from "./character";
import { decodeCharacter, type CharacterDied } from "./layouts";

/** How long a non-owner waits for the owner's app before freezing itself. */
export const NON_OWNER_DELAY_MS = 8_000;

export type FreezeDecision =
  | { kind: "send" }
  | { kind: "skip"; reason: "no-nft" | "frozen" | "alive" | "missing" };

/** Pure decision on the current character state. */
export function decideFreeze(
  ev: CharacterDied,
  character: { alive: boolean; nftFrozen: boolean; nft: Address | null } | null,
): FreezeDecision {
  // FALLBACK(nft): characters minted without a Core NFT have nothing to freeze.
  if (!ev.nft) return { kind: "skip", reason: "no-nft" };
  if (!character) return { kind: "skip", reason: "missing" };
  if (character.alive) return { kind: "skip", reason: "alive" };
  if (character.nftFrozen) return { kind: "skip", reason: "frozen" };
  return { kind: "send" };
}

export type TombstoneLog = {
  character: Address;
  status: "seen" | "sent" | "skipped" | "failed";
  detail: string;
};

export async function handleCharacterDied(opts: {
  ev: CharacterDied;
  me: Address | null;
  payer: TransactionSigner;
  accounts: AccountSource;
  send: (payer: TransactionSigner, ixs: ReturnType<typeof freezeTombstoneInstruction>[]) => Promise<string>;
  sleep?: (ms: number) => Promise<void>;
  signal?: AbortSignal;
}): Promise<TombstoneLog> {
  const { ev, me, payer, accounts, send } = opts;
  const sleep = opts.sleep ?? ((ms) => new Promise((r) => setTimeout(r, ms)));
  if (!ev.nft) return { character: ev.character, status: "skipped", detail: "FALLBACK(nft): no Core NFT" };
  if (me !== ev.owner) await sleep(NON_OWNER_DELAY_MS);
  if (opts.signal?.aborted) return { character: ev.character, status: "skipped", detail: "aborted" };
  const raw = await accounts.get(ev.character);
  const decision = decideFreeze(ev, raw ? decodeCharacter(raw) : null);
  if (decision.kind === "skip") return { character: ev.character, status: "skipped", detail: decision.reason };
  try {
    const sig = await send(payer, [freezeTombstoneInstruction({ character: ev.character, asset: ev.nft, payer })]);
    return { character: ev.character, status: "sent", detail: sig };
  } catch (e) {
    return { character: ev.character, status: "failed", detail: e instanceof Error ? e.message : String(e) };
  }
}
