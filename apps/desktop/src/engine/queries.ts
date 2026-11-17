/**
 * Keyed HTTP results (martial-arts 10 §4). `queryCache` holds sessions, repetitions, progress and
 * athletes by URL; `contentCache` holds content and is dropped whole when `content_rev` changes.
 */
import { useEffect, useState, useSyncExternalStore } from "react";
import type { ApiResponse } from "@contracts";
import { request } from "./http";
import { createStore } from "./store";

export interface QueryCache {
  get<T>(path: string): Promise<ApiResponse<T>>;
  /** Drops entries whose path starts with any prefix; subscribers re-fetch. */
  invalidate(...prefixes: string[]): void;
  /** Increments on every invalidation. */
  readonly version: ReturnType<typeof createStore<number>>;
}

export function createQueryCache(
  load: (path: string) => Promise<ApiResponse<unknown>> = (p) =>
    request("GET", p),
): QueryCache {
  const entries = new Map<string, Promise<ApiResponse<unknown>>>();
  const version = createStore(0);
  return {
    version,
    get<T>(path: string) {
      let hit = entries.get(path);
      if (!hit) {
        hit = load(path).then((r) => {
          // Failures are not cached: the next read retries.
          if (!r.ok) entries.delete(path);
          return r;
        });
        entries.set(path, hit);
      }
      return hit as Promise<ApiResponse<T>>;
    },
    invalidate(...prefixes) {
      for (const key of [...entries.keys()])
        if (prefixes.some((p) => key.startsWith(p))) entries.delete(key);
      version.set(version.get() + 1);
    },
  };
}

export const queryCache = createQueryCache();
const content = createQueryCache();
let contentRev: number | null = null;
export const contentCache = Object.assign(content, {
  /** Clears everything when the engine reports a different `content_rev`. */
  syncRev(rev: number | null | undefined): void {
    if (rev === undefined || rev === null || rev === contentRev) return;
    const first = contentRev === null;
    contentRev = rev;
    if (!first) content.invalidate("");
  },
});

export interface QueryResult<T> {
  data: T | null;
  error: string | null;
  loading: boolean;
}

/** Reads `path` from `cache`; `null` path means nothing to load. Re-reads after invalidation. */
export function useQuery<T>(
  path: string | null,
  cache: QueryCache = queryCache,
): QueryResult<T> {
  const version = useSyncExternalStore(cache.version.subscribe, cache.version.get);
  const [result, setResult] = useState<QueryResult<T> & { path: string | null }>({
    data: null,
    error: null,
    loading: path !== null,
    path,
  });
  useEffect(() => {
    if (path === null) return;
    let live = true;
    setResult((r) => ({ ...r, loading: true, path }));
    void cache.get<T>(path).then((r) => {
      if (!live) return;
      setResult(
        r.ok
          ? { data: r.data, error: null, loading: false, path }
          : { data: null, error: r.error.message, loading: false, path },
      );
    });
    return () => {
      live = false;
    };
  }, [path, version, cache]);
  // A new path shows nothing from the previous one.
  if (result.path !== path)
    return { data: null, error: null, loading: path !== null };
  return result;
}
