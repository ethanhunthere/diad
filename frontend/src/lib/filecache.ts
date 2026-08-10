// ============================================================================
// IndexedDB file cache (spec 6.6) — LRU, capped at 50 MB
// ============================================================================
// Persists open editor buffers locally so a refresh (or an idle-stage PTY
// teardown) never loses work. Entries are evicted least-recently-used when
// the store exceeds the cap, and the whole store is dropped when the GUI
// reconnects after the daemon went deep-idle (stage 2), which is the
// browser-side mirror of the daemon's cache eviction.

const DB_NAME = "diad-file-cache";
const STORE = "files";
const CAP_BYTES = 50 * 1024 * 1024;

interface Entry {
  path: string;
  content: string;
  savedAt: number;
  size: number;
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => {
      req.result.createObjectStore(STORE, { keyPath: "path" });
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function withStore<T>(
  mode: IDBTransactionMode,
  fn: (store: IDBObjectStore) => IDBRequest<T>,
): Promise<T> {
  const db = await openDb();
  return new Promise<T>((resolve, reject) => {
    const tx = db.transaction(STORE, mode);
    const req = fn(tx.objectStore(STORE));
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
    tx.oncomplete = () => db.close();
  });
}

/** Store one file's content (debounced by the caller). */
export async function cacheFile(path: string, content: string): Promise<void> {
  const entry: Entry = { path, content, savedAt: Date.now(), size: content.length * 2 };
  try {
    await withStore("readwrite", (s) => s.put(entry));
    void evictOverCap();
  } catch {
    /* IndexedDB unavailable (private mode etc.) — cache is best-effort */
  }
}

/** Read a cached buffer back (returns null when absent). */
export async function readCache(path: string): Promise<string | null> {
  try {
    const entry = await withStore<Entry | undefined>("readonly", (s) => s.get(path));
    return entry?.content ?? null;
  } catch {
    return null;
  }
}

/** Remove one entry (e.g. after a successful save to the daemon). */
export async function dropCache(path: string): Promise<void> {
  try {
    await withStore("readwrite", (s) => s.delete(path));
  } catch {
    /* best-effort */
  }
}

/** Drop the entire cache (idle stage 2 mirror / fresh start). */
export async function clearCache(): Promise<void> {
  try {
    await withStore("readwrite", (s) => s.clear());
  } catch {
    /* best-effort */
  }
}

/** LRU eviction until the store fits under CAP_BYTES. */
async function evictOverCap(): Promise<void> {
  const all = await withStore<Entry[]>("readonly", (s) => s.getAll());
  let total = all.reduce((acc, e) => acc + e.size, 0);
  if (total <= CAP_BYTES) return;
  const byAge = [...all].sort((a, b) => a.savedAt - b.savedAt);
  for (const entry of byAge) {
    if (total <= CAP_BYTES) break;
    await withStore("readwrite", (s) => s.delete(entry.path)).catch(() => undefined);
    total -= entry.size;
  }
}
