import { useCallback, useEffect, useState } from "react";
import type {
  ApiEndpointInventoryItem,
  ApiEndpointInventoryPage,
  ApiEndpointInventoryRequest,
  ApiInventoryCounts,
} from "./model";

export type ApiCatalogPageLoader = (
  request: ApiEndpointInventoryRequest,
) => Promise<ApiEndpointInventoryPage>;

interface ApiCatalogSnapshot {
  endpoints: ApiEndpointInventoryItem[];
  total: number;
  indexedTotal: number;
  counts: ApiInventoryCounts;
}

export interface ApiCatalogLoadState extends ApiCatalogSnapshot {
  loading: boolean;
  error: string | null;
  retry: () => void;
}

const completedCache = new Map<string, ApiCatalogSnapshot>();
const PAGE_SIZE = 200;
const MAX_ENDPOINTS = 10_000;
const MAX_PAGES = Math.ceil(MAX_ENDPOINTS / PAGE_SIZE);
const EMPTY_COUNTS: ApiInventoryCounts = {
  producer: 0, consumerOnly: 0, clientCovered: 0, unknown: 0,
};

function cacheCompleted(key: string, snapshot: ApiCatalogSnapshot): void {
  completedCache.delete(key);
  completedCache.set(key, snapshot);
  while (completedCache.size > 4) {
    const oldest = completedCache.keys().next().value;
    if (typeof oldest !== "string") break;
    completedCache.delete(oldest);
  }
}

function failureMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Unable to load the API inventory";
}

export function useApiCatalog(
  workspaceId: string | undefined,
  workspaceGeneration: number,
  query: string,
  loadPage: ApiCatalogPageLoader,
): ApiCatalogLoadState {
  const normalizedQuery = query.trim().slice(0, 128);
  const [debouncedQuery, setDebouncedQuery] = useState(normalizedQuery);
  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedQuery(normalizedQuery), 180);
    return () => window.clearTimeout(timer);
  }, [normalizedQuery]);
  const cacheKey = workspaceId
    ? `${workspaceId}:${workspaceGeneration}:${encodeURIComponent(debouncedQuery)}`
    : "";
  const cached = cacheKey ? completedCache.get(cacheKey) : undefined;
  const [endpoints, setEndpoints] = useState<ApiEndpointInventoryItem[]>(cached?.endpoints ?? []);
  const [total, setTotal] = useState(cached?.total ?? 0);
  const [indexedTotal, setIndexedTotal] = useState(cached?.indexedTotal ?? 0);
  const [counts, setCounts] = useState<ApiInventoryCounts>(cached?.counts ?? EMPTY_COUNTS);
  const [loading, setLoading] = useState(Boolean(workspaceId && !cached));
  const [error, setError] = useState<string | null>(null);
  const [snapshotKey, setSnapshotKey] = useState(cached ? cacheKey : "");
  const [attempt, setAttempt] = useState(0);
  const retry = useCallback(() => {
    setLoading(true);
    setError(null);
    setAttempt((current) => current + 1);
  }, []);

  useEffect(() => {
    let cancelled = false;
    if (!workspaceId) {
      setEndpoints([]);
      setTotal(0);
      setIndexedTotal(0);
      setCounts(EMPTY_COUNTS);
      setLoading(false);
      setError(null);
      setSnapshotKey("");
      return () => { cancelled = true; };
    }
    const ready = completedCache.get(cacheKey);
    if (ready) {
      setEndpoints(ready.endpoints);
      setTotal(ready.total);
      setIndexedTotal(ready.indexedTotal);
      setCounts(ready.counts);
      setLoading(false);
      setError(null);
      setSnapshotKey(cacheKey);
      return () => { cancelled = true; };
    }

    setSnapshotKey(cacheKey);
    setEndpoints([]);
    setTotal(0);
    setIndexedTotal(0);
    setCounts(EMPTY_COUNTS);
    setLoading(true);
    setError(null);
    void (async () => {
      const byId = new Map<string, ApiEndpointInventoryItem>();
      const visitedCursors = new Set<string>();
      let cursor: string | undefined;
      let expectedTotal = 0;
      for (let pageIndex = 0; pageIndex < MAX_PAGES; pageIndex += 1) {
        const page = await loadPage({
          workspaceId,
          query: debouncedQuery || undefined,
          cursor,
          limit: PAGE_SIZE,
        });
        if (cancelled) return;
        if (page.workspaceId !== workspaceId) {
          throw new Error("The API inventory response belongs to another workspace");
        }
        expectedTotal = page.total;
        if (expectedTotal > MAX_ENDPOINTS) {
          throw new Error(`API inventory has ${expectedTotal} endpoints, above the ${MAX_ENDPOINTS} endpoint renderer limit`);
        }
        for (const endpoint of page.endpoints) byId.set(endpoint.id, endpoint);
        const loaded = [...byId.values()];
        if (page.returned !== page.endpoints.length) {
          throw new Error("API inventory returned an inconsistent page count");
        }
        setEndpoints(loaded);
        setTotal(expectedTotal);
        setIndexedTotal(page.indexedTotal);
        setCounts(page.counts);

        const next = page.nextCursor;
        if (!next) {
          if (page.truncated || loaded.length < expectedTotal) {
            throw new Error(`API inventory stopped after ${loaded.length} of ${expectedTotal} endpoints`);
          }
          const snapshot = {
            endpoints: loaded,
            total: expectedTotal,
            indexedTotal: page.indexedTotal,
            counts: page.counts,
          };
          cacheCompleted(cacheKey, snapshot);
          setLoading(false);
          return;
        }
        if (visitedCursors.has(next)) throw new Error("API inventory returned a repeated page cursor");
        visitedCursors.add(next);
        cursor = next;
      }
      throw new Error("API inventory exceeded the safe page limit");
    })().catch((reason: unknown) => {
      if (!cancelled) {
        setError(failureMessage(reason));
        setLoading(false);
      }
    });
    return () => { cancelled = true; };
  }, [attempt, cacheKey, debouncedQuery, loadPage, workspaceId]);

  const currentSnapshot = snapshotKey === cacheKey;
  return {
    endpoints: currentSnapshot ? endpoints : [],
    total: currentSnapshot ? total : 0,
    indexedTotal: currentSnapshot ? indexedTotal : 0,
    counts: currentSnapshot ? counts : EMPTY_COUNTS,
    loading: loading || normalizedQuery !== debouncedQuery || !currentSnapshot,
    error,
    retry,
  };
}

export function clearApiCatalogCache(): void {
  completedCache.clear();
}
