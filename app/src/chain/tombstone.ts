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
/** Extra random wait per non-owner, so several watching apps rarely send at once. */
export const NON_OWNER_JITTER_MS = 6_000;
/** The death and the log notification can run ahead of the RPC's account view: re-read this often. */
const STALE_RETRIES = 8;
const STALE_RETRY_MS = 750;

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
  /** Non-owner extra wait; random in [0, NON_OWNER_JITTER_MS) by default. */
  jitterMs?: number;
}): Promise<TombstoneLog> {
  const { ev, me, payer, accounts, send } = opts;
  const sleep = opts.sleep ?? ((ms) => new Promise((r) => setTimeout(r, ms)));
  if (!ev.nft) return { character: ev.character, status: "skipped", detail: "FALLBACK(nft): no Core NFT" };
  if (me !== ev.owner) {
    await sleep(NON_OWNER_DELAY_MS);
    const jitter = opts.jitterMs ?? Math.floor(Math.random() * NON_OWNER_JITTER_MS);
    if (jitter > 0) await sleep(jitter);
  }
  let decision: FreezeDecision = { kind: "skip", reason: "missing" };
  for (let i = 0; i < STALE_RETRIES; i++) {
    if (opts.signal?.aborted) return { character: ev.character, status: "skipped", detail: "aborted" };
    const raw = await accounts.get(ev.character);
    decision = decideFreeze(ev, raw ? decodeCharacter(raw) : null);
    // "alive" right after CharacterDied is a stale read, not a resurrection: read again.
    if (decision.kind !== "skip" || decision.reason !== "alive") break;
    await sleep(STALE_RETRY_MS);
  }
  if (decision.kind === "skip") return { character: ev.character, status: "skipped", detail: decision.reason };
  try {
    const sig = await send(payer, [freezeTombstoneInstruction({ character: ev.character, asset: ev.nft, payer })]);
    return { character: ev.character, status: "sent", detail: sig };
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    // Another app froze it between our read and our send.
    if (/AlreadyFrozen|#6016/.test(msg)) return { character: ev.character, status: "skipped", detail: "frozen" };
    return { character: ev.character, status: "failed", detail: msg };
  }
}
