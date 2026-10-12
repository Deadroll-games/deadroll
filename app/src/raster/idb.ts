// Tiny IndexedDB cache for verified WASM bodies, keyed by SHA-256 hex. Callers re-verify the hash
// on read, so a tampered entry is only a cache miss. Any IndexedDB failure is a miss too.
const DB = "deadroll";
const STORE = "wasm";

export type WasmCache = {
  get(hashHex: string): Promise<Uint8Array | null>;
  put(hashHex: string, bytes: Uint8Array): Promise<void>;
};

function open(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function tx<T>(mode: IDBTransactionMode, run: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  return open().then(
    (db) =>
      new Promise<T>((resolve, reject) => {
        const t = db.transaction(STORE, mode);
        const req = run(t.objectStore(STORE));
        t.oncomplete = () => {
          db.close();
          resolve(req.result);
        };
        t.onerror = () => {
          db.close();
          reject(t.error);
        };
      }),
  );
}

export const idbWasmCache: WasmCache = {
  async get(hashHex) {
    try {
      const v = await tx("readonly", (s) => s.get(hashHex) as IDBRequest<unknown>);
      return v instanceof Uint8Array ? v : v instanceof ArrayBuffer ? new Uint8Array(v) : null;
    } catch {
      return null;
    }
  },
  async put(hashHex, bytes) {
    try {
      await tx("readwrite", (s) => s.put(bytes, hashHex));
    } catch {
      // cache is optional
    }
  },
};

export const noCache: WasmCache = { get: async () => null, put: async () => {} };
