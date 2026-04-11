import { useCallback, useEffect, useRef, useState } from "react";

import { getQuota, type Quota } from "../lib/tauri";

export interface UseQuotaResult {
  /** Latest known quota, or null if never fetched or fetch failed. */
  quota: Quota | null;
  /** True while a fetch is in progress. */
  loading: boolean;
  /** Last fetch error message, or null on success. */
  error: string | null;
  /** Re-fetch the quota from the server. Safe to call repeatedly. */
  refresh: () => Promise<void>;
}

/**
 * Hook that fetches and exposes the authenticated user's generation quota.
 *
 * Fetches once on mount and then whenever `refresh()` is called manually
 * (for example after a generation completes). The hook is tolerant of
 * offline / auth failures: on error, `quota` stays at the last known good
 * value and `error` surfaces the reason.
 *
 * The `enabled` flag lets callers skip fetching until the user is logged in;
 * while `enabled` is false the hook stays inert and returns null quota.
 */
export function useQuota(enabled: boolean = true): UseQuotaResult {
  const [quota, setQuota] = useState<Quota | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const refresh = useCallback(async () => {
    if (!enabled) return;
    setLoading(true);
    try {
      const result = await getQuota();
      if (!mountedRef.current) return;
      setQuota(result);
      setError(null);
    } catch (e) {
      if (!mountedRef.current) return;
      setError(String(e));
      // Keep the last known good quota; do not wipe it on transient errors.
    } finally {
      if (mountedRef.current) setLoading(false);
    }
  }, [enabled]);

  useEffect(() => {
    mountedRef.current = true;
    if (enabled) {
      void refresh();
    }
    return () => {
      mountedRef.current = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled]);

  return { quota, loading, error, refresh };
}
