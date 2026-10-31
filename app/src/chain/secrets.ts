// Patch C: the DM's secrets on the website (docs/MAP_DESCRIPTION.md 8, 17.5). The tree, the
// encrypted copy and the keys are `@deadroll/sdk/secrets` (pinned against the Rust tree by its
// fixture); this file keeps a draft's secrets by cell, turns them into leaves, and holds the
// wallet's unlock signature in this tab's memory only: nothing is stored, any device with the
// wallet opens the secrets again.
import { Leaves, Secret, UNLOCK_MESSAGE, declaredOf, deriveKeys, type PackShape, type SecretKeys } from "@deadroll/sdk/secrets";
import type { DraftSecret, PackDraft } from "./mapcheckWasm";
import bestiary from "../../../assets/registry/bestiary.json";

/** The bestiary ids with the Boss role (the registry table is seeded from this file): a pack's
 * threat counts a boss's level 3 times. */
const BOSS_IDS = new Set((bestiary as { records: { id: number; role: string }[] }).records.filter((m) => m.role === "Boss").map((m) => m.id));
export const isBoss = (bestiaryId: number) => BOSS_IDS.has(bestiaryId);

export { Secret };

/** A secret as the builder places it: on the map (`pack: null`) or in pack `pack`'s arena. */
export type BoardSecret = { pack: number | null; x: number; y: number; kind: number; a: number; b: number };

export const SECRET_LABELS: Record<number, string> = {
  [Secret.Trap]: "Trap",
  [Secret.Ambush]: "Ambush",
  [Secret.Stash]: "Hidden stash",
  [Secret.Door]: "Secret door",
};

export const packShapes = (packs: (PackDraft | null)[]): PackShape[] => packs.map((p) => (p ? { width: p.width, height: p.height } : null));

/** The draft's secrets by leaf (a secret outside its board is left out: the builder moves it). */
export function secretLeaves(width: number, height: number, packs: (PackDraft | null)[], list: BoardSecret[]): DraftSecret[] {
  const leaves = new Leaves(width, height, packShapes(packs));
  const out: DraftSecret[] = [];
  for (const s of list) {
    const leaf = s.pack === null ? leaves.of({ kind: "map", x: s.x, y: s.y }) : leaves.of({ kind: "arena", pack: s.pack, x: s.x, y: s.y });
    if (leaf !== null) out.push({ leaf, kind: s.kind, a: s.a, b: s.b, c: 0 });
  }
  return out;
}

/** The declared counts of the draft's secrets (trap, ambush, stash, door). */
export const declaredOfDraft = (list: BoardSecret[]) => declaredOf(list.map((s) => ({ leaf: 0, secret: { kind: s.kind, a: s.a, b: s.b, c: 0 } as never })));

/** Signs `UNLOCK_MESSAGE` with the connected wallet (`signMessage` of the wallet plugin, or a dev
 * key pair's). */
export type MessageSigner = { address: string; signMessage: (message: Uint8Array) => Promise<Uint8Array> };

const signatures = new Map<string, Uint8Array>();
const CHECKED_KEY = "deadroll.unlockChecked.v1:";

/** The wallet's unlock signature, asked once per tab and kept in memory. The first time a wallet
 * unlocks in this browser it signs twice: an Ed25519 wallet gives the same signature, and a wallet
 * that does not could never open its secrets again. Only the "checked" flag is stored. */
export async function unlockSignature(signer: MessageSigner): Promise<Uint8Array> {
  const known = signatures.get(signer.address);
  if (known) return known;
  const message = new TextEncoder().encode(UNLOCK_MESSAGE);
  const sig = await signer.signMessage(message);
  const flag = CHECKED_KEY + signer.address;
  if (!readFlag(flag)) {
    const again = await signer.signMessage(message);
    if (again.length !== sig.length || again.some((b, i) => b !== sig[i])) {
      throw new Error("This wallet signs the same message differently each time: it cannot keep map secrets. Use another wallet.");
    }
    writeFlag(flag);
  }
  signatures.set(signer.address, sig);
  return sig;
}

/** The keys of map `map` (its address bytes) for this wallet. */
export async function mapKeys(signer: MessageSigner, map: Uint8Array): Promise<SecretKeys> {
  return deriveKeys(await unlockSignature(signer), map);
}

/** Forgets the signatures (tests, wallet change). */
export function forgetUnlock() {
  signatures.clear();
}

function readFlag(key: string): boolean {
  try {
    return globalThis.localStorage?.getItem(key) === "1";
  } catch {
    return false;
  }
}

function writeFlag(key: string) {
  try {
    globalThis.localStorage?.setItem(key, "1");
  } catch {
    // No storage: the double signature is asked again next time.
  }
}
