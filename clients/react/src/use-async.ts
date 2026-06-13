import { useCallback, useEffect, useRef, useState } from 'react';
import type { DependencyList } from 'react';

/** State returned by {@link useQuery}. */
export interface QueryResult<T> {
  /** The latest successful value, or `undefined` before the first success. */
  data: T | undefined;
  /** The latest error, or `undefined` if the last run succeeded. */
  error: unknown;
  /** Whether a run is in flight. */
  loading: boolean;
  /** Re-run the query. */
  refetch: () => void;
}

/** Options for {@link useQuery}. */
export interface QueryOptions {
  /** When `false`, the query does not run until re-enabled. Defaults to `true`. */
  enabled?: boolean;
}

/**
 * Run an async function on mount and whenever `deps` change. Stale runs are cancelled via an
 * `AbortSignal`, and state is never set after unmount. Returns the latest data/error/loading plus a
 * `refetch`.
 */
export function useQuery<T>(
  fn: (signal: AbortSignal) => Promise<T>,
  deps: DependencyList,
  options: QueryOptions = {},
): QueryResult<T> {
  const enabled = options.enabled ?? true;
  const [data, setData] = useState<T | undefined>(undefined);
  const [error, setError] = useState<unknown>(undefined);
  const [loading, setLoading] = useState<boolean>(enabled);
  const [nonce, setNonce] = useState(0);

  const fnRef = useRef(fn);
  fnRef.current = fn;

  useEffect(() => {
    if (!enabled) {
      setLoading(false);
      return;
    }
    const controller = new AbortController();
    let active = true;
    setLoading(true);
    setError(undefined);
    fnRef.current(controller.signal).then(
      (value) => {
        if (active) {
          setData(value);
          setLoading(false);
        }
      },
      (err: unknown) => {
        if (active && !controller.signal.aborted) {
          setError(err);
          setLoading(false);
        }
      },
    );
    return () => {
      active = false;
      controller.abort();
    };
  }, [...deps, enabled, nonce]);

  const refetch = useCallback(() => {
    setNonce((n) => n + 1);
  }, []);

  return { data, error, loading, refetch };
}

/** State returned by {@link useMutation}. */
export interface MutationResult<TArgs extends unknown[], TData> {
  /** Run the mutation. Resolves with the result and also stores it; rejects on error. */
  mutate: (...args: TArgs) => Promise<TData>;
  /** The latest successful result. */
  data: TData | undefined;
  /** The latest error. */
  error: unknown;
  /** Whether a run is in flight. */
  loading: boolean;
  /** Clear data, error, and loading. */
  reset: () => void;
}

/**
 * Wrap an async action as a manually-triggered mutation. `mutate` returns the action's promise (so
 * callers can `await` it and handle errors) while also tracking data/error/loading for rendering.
 * State is never set after unmount.
 */
export function useMutation<TArgs extends unknown[], TData>(
  fn: (...args: TArgs) => Promise<TData>,
): MutationResult<TArgs, TData> {
  const [data, setData] = useState<TData | undefined>(undefined);
  const [error, setError] = useState<unknown>(undefined);
  const [loading, setLoading] = useState(false);

  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const fnRef = useRef(fn);
  fnRef.current = fn;

  const mutate = useCallback(async (...args: TArgs): Promise<TData> => {
    setLoading(true);
    setError(undefined);
    try {
      const result = await fnRef.current(...args);
      if (mountedRef.current) {
        setData(result);
      }
      return result;
    } catch (err) {
      if (mountedRef.current) {
        setError(err);
      }
      throw err;
    } finally {
      if (mountedRef.current) {
        setLoading(false);
      }
    }
  }, []);

  const reset = useCallback(() => {
    setData(undefined);
    setError(undefined);
    setLoading(false);
  }, []);

  return { mutate, data, error, loading, reset };
}
