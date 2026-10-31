// Patch C: a map's secrets as the website, the scripts and the tests build them
// (docs/MAP_DESCRIPTION.md 8, 17.2, 17.5). The Merkle tree mirrors `deadroll_mapcheck::secrets`
// (test/secrets.test.ts replays its fixture); the leaf contents are the programs' own codecs.
//
// Keys: the DM's wallet signs UNLOCK_MESSAGE once (Ed25519 signatures are deterministic, RFC 8032),
// HKDF-SHA256 of that signature with the map address gives an AES-256-GCM key for the encrypted copy
// on chain and an HMAC-SHA256 key that derives each leaf's 16-byte salt. Nothing is stored: any
// device with the wallet opens the secrets again. WebCrypto only (browsers, Node 20+).
import { getCellSecretEncoder, getSpawnEncoder, type CellSecretArgs, type SpawnArgs } from "./generated/session/index.js";

export const PACKS_MAX = 8;
export const PACK_LEAVES = 8;
export const ARENA_MONSTERS_MAX = 8;
export const SECRET_TYPES = 4;
export const SECRETS_MAX = 16;
export const SECRETS_BLOB_MAX = 800;
export const SALT_LEN = 16;
export const DEPTH_MAX = 12;
/** `PackLayout.visible` of an ambush pack. */
export const NO_SPAWN = 0xff;
export const SPAWN_ELITE = 1;
/** `SecretKind` (crates/types). */
export const Secret = { None: 0, Trap: 1, Ambush: 2, Stash: 3, Door: 4 } as const;

const TAG_CELL = 0;
const TAG_NODE = 1;
const TAG_PACK = 2;

/** What the wallet signs to unlock a DM's secrets (the same on every map and device). */
export const UNLOCK_MESSAGE = "Deadroll: unlock my map secrets. This signature never moves funds.";

export type Roster = { count: number; spawns: SpawnArgs[] };
export type CellEntry = { leaf: number; secret: CellSecretArgs };
/** A pack's arena size by pack number (`null`: unused). */
export type PackShape = { width: number; height: number } | null;
export type Board = { kind: "pack"; pack: number } | { kind: "map"; x: number; y: number } | { kind: "arena"; pack: number; x: number; y: number };

const subtle = () => globalThis.crypto.subtle;
const concat = (...parts: Uint8Array[]) => {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
};
const u16 = (v: number) => new Uint8Array([v & 0xff, (v >> 8) & 0xff]);
const buf = (b: Uint8Array) => b as Uint8Array<ArrayBuffer>;
async function sha256(...parts: Uint8Array[]): Promise<Uint8Array> {
  return new Uint8Array(await subtle().digest("SHA-256", buf(concat(...parts))));
}

/** The leaf numbering (`deadroll_mapcheck::secrets::Leaves`): packs, the map row by row, each used arena. */
export class Leaves {
  readonly arenaAt: number[] = new Array(PACKS_MAX).fill(0);
  readonly count: number;
  constructor(
    readonly width: number,
    readonly height: number,
    readonly packs: PackShape[],
  ) {
    let at = PACK_LEAVES + width * height;
    for (let n = 0; n < PACKS_MAX; n++) {
      const p = packs[n];
      if (!p) continue;
      this.arenaAt[n] = at;
      at += p.width * p.height;
    }
    this.count = at;
  }
  of(b: Board): number | null {
    if (b.kind === "pack") return b.pack < PACK_LEAVES ? b.pack : null;
    if (b.kind === "map") return b.x < this.width && b.y < this.height ? PACK_LEAVES + b.y * this.width + b.x : null;
    const p = this.packs[b.pack];
    return p && b.x < p.width && b.y < p.height ? this.arenaAt[b.pack] + b.y * p.width + b.x : null;
  }
  board(leaf: number): Board | null {
    if (leaf < 0 || leaf >= this.count) return null;
    if (leaf < PACK_LEAVES) return { kind: "pack", pack: leaf };
    const m = leaf - PACK_LEAVES;
    if (m < this.width * this.height) return { kind: "map", x: m % this.width, y: Math.floor(m / this.width) };
    for (let n = 0; n < PACKS_MAX; n++) {
      const p = this.packs[n];
      if (!p) continue;
      const i = leaf - this.arenaAt[n];
      if (i >= 0 && i < p.width * p.height) return { kind: "arena", pack: n, x: i % p.width, y: Math.floor(i / p.width) };
    }
    return null;
  }
}

/** Hashes in a proof of a tree of `count` leaves. */
export const depth = (count: number) => {
  let d = 0;
  while (1 << d < count) d++;
  return d;
};

export const secretBytes = (s: CellSecretArgs) => new Uint8Array(getCellSecretEncoder().encode(s));
export const emptyRoster = (): Roster => ({ count: 0, spawns: Array.from({ length: ARENA_MONSTERS_MAX }, () => ({ bestiaryId: 0, x: 0, y: 0, level: 0, skills: 0, flags: 0, aspects: 0, pack: 0 })) });
/** `PackRoster::bytes`: the count, then all `ARENA_MONSTERS_MAX` spawns. */
export function rosterBytes(r: Roster): Uint8Array {
  const enc = getSpawnEncoder();
  const spawns = emptyRoster().spawns.map((z, i) => r.spawns[i] ?? z);
  return concat(new Uint8Array([r.count]), ...spawns.map((s) => new Uint8Array(enc.encode(s))));
}

export const cellLeaf = (map: Uint8Array, leaf: number, s: CellSecretArgs, salt: Uint8Array) => sha256(new Uint8Array([TAG_CELL]), map, u16(leaf), secretBytes(s), salt);
export const packLeaf = (map: Uint8Array, leaf: number, r: Roster, salt: Uint8Array) => sha256(new Uint8Array([TAG_PACK]), map, u16(leaf), rosterBytes(r), salt);
export const node = (l: Uint8Array, r: Uint8Array) => sha256(new Uint8Array([TAG_NODE]), l, r);

/** The tree over `leaves` (padded with zero hashes to a power of two): its root and each leaf's proof. */
export async function buildTree(leaves: Uint8Array[]): Promise<{ root: Uint8Array; proof: (index: number) => Uint8Array[] }> {
  const d = depth(leaves.length);
  let level = [...leaves];
  while (level.length < 1 << d) level.push(new Uint8Array(32));
  const levels = [level];
  while (level.length > 1) {
    const next: Uint8Array[] = [];
    for (let i = 0; i < level.length; i += 2) next.push(await node(level[i], level[i + 1]));
    levels.push(next);
    level = next;
  }
  return {
    root: level[0],
    proof: (index) => levels.slice(0, d).map((l, k) => l[(index >> k) ^ 1]),
  };
}

/** A pack's threat (Ivan's decision): its monsters' levels, an elite's counted `elite` times, a boss's `boss` times. */
export function threat(spawns: SpawnArgs[], isBoss: (bestiaryId: number) => boolean, weights = { elite: 2, boss: 3 }): number {
  return spawns.reduce((t, s) => t + s.level * (isBoss(s.bestiaryId) ? weights.boss : s.flags & SPAWN_ELITE ? weights.elite : 1), 0);
}

export type PackRecord = { visible: number; x: number; y: number; facing: number; width: number; height: number };

/** `deadroll_mapcheck::design::split`: the public spawns (the visible ones, `visible` renumbered) and each pack's roster, visible monster first. */
export function splitDesign(spawns: SpawnArgs[], packs: (PackRecord | null)[]): { spawns: SpawnArgs[]; packs: (PackRecord | null)[]; rosters: Roster[] } {
  const pub: SpawnArgs[] = [];
  const outPacks = packs.map((p) => (p ? { ...p } : null));
  const rosters = Array.from({ length: PACKS_MAX }, emptyRoster);
  for (let n = 0; n < PACKS_MAX; n++) {
    const p = packs[n];
    if (!p) continue;
    const list: SpawnArgs[] = [];
    if (p.visible !== NO_SPAWN) {
      const v = spawns[p.visible];
      if (!v || v.pack !== n) throw new Error(`pack ${n}: its visible monster is not in the pack`);
      list.push(v);
      outPacks[n]!.visible = pub.length;
      pub.push(v);
    }
    spawns.forEach((s, i) => {
      if (s.pack === n && i !== p.visible) list.push(s);
    });
    if (list.length > ARENA_MONSTERS_MAX) throw new Error(`pack ${n}: more than ${ARENA_MONSTERS_MAX} monsters`);
    list.forEach((s, i) => (rosters[n].spawns[i] = s));
    rosters[n].count = list.length;
  }
  return { spawns: pub, packs: outPacks, rosters };
}

/** The declared counts of a list of secrets (`SecretKind::counted`). */
export function declaredOf(cells: CellEntry[]): number[] {
  const out = [0, 0, 0, 0];
  for (const c of cells) if (c.secret.kind !== Secret.None) out[(c.secret.kind as number) - 1]++;
  return out;
}

export type SecretKeys = { enc: CryptoKey; salt: CryptoKey };

/** The keys of one map from the wallet's signature of `UNLOCK_MESSAGE`. */
export async function deriveKeys(signature: Uint8Array, map: Uint8Array): Promise<SecretKeys> {
  const ikm = await subtle().importKey("raw", buf(signature), "HKDF", false, ["deriveBits"]);
  const bits = new Uint8Array(
    await subtle().deriveBits({ name: "HKDF", hash: "SHA-256", salt: buf(new TextEncoder().encode("deadroll/secrets/v1")), info: buf(map) }, ikm, 512),
  );
  const enc = await subtle().importKey("raw", bits.slice(0, 32), "AES-GCM", false, ["encrypt", "decrypt"]);
  const salt = await subtle().importKey("raw", bits.slice(32), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  return { enc, salt };
}

/** Leaf `leaf`'s salt: the first 16 bytes of HMAC-SHA256(key, leaf u16 LE). */
export async function saltOf(keys: SecretKeys, leaf: number): Promise<Uint8Array> {
  return new Uint8Array(await subtle().sign("HMAC", keys.salt, buf(u16(leaf)))).slice(0, SALT_LEN);
}

/** The DM's secrets: every pack's roster and every cell that hides something. */
export type Secrets = { rosters: Roster[]; cells: CellEntry[] };

const BLOB_VERSION = 1;
/** The plain bytes of the encrypted copy: version, 8 rosters, the cells (`leaf` u16 LE + 4 bytes). */
export function encodeSecrets(s: Secrets): Uint8Array {
  if (s.cells.length > SECRETS_MAX) throw new Error(`at most ${SECRETS_MAX} secrets`);
  const rosters = Array.from({ length: PACKS_MAX }, (_, n) => rosterBytes(s.rosters[n] ?? emptyRoster()));
  return concat(new Uint8Array([BLOB_VERSION]), ...rosters, new Uint8Array([s.cells.length]), ...s.cells.map((c) => concat(u16(c.leaf), secretBytes(c.secret))));
}

export function decodeSecrets(bytes: Uint8Array): Secrets {
  if (bytes[0] !== BLOB_VERSION) throw new Error(`unknown secrets version ${bytes[0]}`);
  const rosterLen = 1 + ARENA_MONSTERS_MAX * 9;
  const rosters: Roster[] = [];
  let at = 1;
  for (let n = 0; n < PACKS_MAX; n++, at += rosterLen) {
    const r = bytes.subarray(at, at + rosterLen);
    const spawns = Array.from({ length: ARENA_MONSTERS_MAX }, (_, i) => {
      const s = r.subarray(1 + i * 9, 10 + i * 9);
      return { bestiaryId: s[0], x: s[1], y: s[2], level: s[3], skills: s[4] | (s[5] << 8), flags: s[6], aspects: s[7], pack: s[8] };
    });
    rosters.push({ count: r[0], spawns });
  }
  const cells: CellEntry[] = [];
  const n = bytes[at++];
  for (let i = 0; i < n; i++, at += 6) {
    cells.push({ leaf: bytes[at] | (bytes[at + 1] << 8), secret: { kind: bytes[at + 2], a: bytes[at + 3], b: bytes[at + 4], c: bytes[at + 5] } as CellSecretArgs });
  }
  return { rosters, cells };
}

/** The encrypted copy: a random 12-byte nonce, then AES-GCM of `encodeSecrets` (the map address as AAD). */
export async function sealSecrets(keys: SecretKeys, map: Uint8Array, s: Secrets): Promise<Uint8Array> {
  const iv = globalThis.crypto.getRandomValues(new Uint8Array(12));
  const ct = new Uint8Array(await subtle().encrypt({ name: "AES-GCM", iv, additionalData: buf(map) }, keys.enc, buf(encodeSecrets(s))));
  const out = concat(iv, ct);
  if (out.length > SECRETS_BLOB_MAX) throw new Error(`encrypted secrets take ${out.length} bytes, at most ${SECRETS_BLOB_MAX}`);
  return out;
}

/** Opens the encrypted copy; throws if the key (the wallet's signature) does not fit. */
export async function openSecrets(keys: SecretKeys, map: Uint8Array, blob: Uint8Array): Promise<Secrets> {
  const plain = await subtle().decrypt({ name: "AES-GCM", iv: buf(blob.slice(0, 12)), additionalData: buf(map) }, keys.enc, buf(blob.slice(12)));
  return decodeSecrets(new Uint8Array(plain));
}

/** The whole commitment: every leaf hashed with its salt, the root, and what a reveal sends. */
export type Commitment = {
  leaves: Leaves;
  root: Uint8Array;
  /** The reveal of a cell leaf: its contents, salt and proof. */
  cell: (leaf: number) => Promise<{ secret: CellSecretArgs; salt: Uint8Array; proof: Uint8Array[] }>;
  /** The reveal of pack `n`'s roster. */
  pack: (n: number) => Promise<{ roster: Roster; salt: Uint8Array; proof: Uint8Array[] }>;
};

const NONE: CellSecretArgs = { kind: Secret.None, a: 0, b: 0, c: 0 } as CellSecretArgs;

export async function commit(map: Uint8Array, keys: SecretKeys, width: number, height: number, packs: PackShape[], s: Secrets): Promise<Commitment> {
  const leaves = new Leaves(width, height, packs);
  const byLeaf = new Map(s.cells.map((c) => [c.leaf, c.secret]));
  const salts = await Promise.all(Array.from({ length: leaves.count }, (_, i) => saltOf(keys, i)));
  const hashes = await Promise.all(
    salts.map((salt, i) => (i < PACK_LEAVES ? packLeaf(map, i, s.rosters[i] ?? emptyRoster(), salt) : cellLeaf(map, i, byLeaf.get(i) ?? NONE, salt))),
  );
  const tree = await buildTree(hashes);
  return {
    leaves,
    root: tree.root,
    cell: async (leaf) => ({ secret: byLeaf.get(leaf) ?? NONE, salt: salts[leaf], proof: tree.proof(leaf) }),
    pack: async (n) => ({ roster: s.rosters[n] ?? emptyRoster(), salt: salts[n], proof: tree.proof(n) }),
  };
}
