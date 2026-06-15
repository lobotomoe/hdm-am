import { createContext, createElement, useContext, useMemo } from 'react';
import type { ReactNode } from 'react';
import { HdmBridgeClient } from '@hdm-am/client';
import type { HdmClientOptions } from '@hdm-am/client';

const HdmContext = createContext<HdmBridgeClient | null>(null);

/** Props for {@link HdmProvider}: pass a ready `client`, or `options` to construct one. */
export interface HdmProviderProps {
  /** A pre-built client. Takes precedence over `options`. */
  client?: HdmBridgeClient;
  /** Options to construct a client when `client` is not given. Memoize this to avoid rebuilds. */
  options?: HdmClientOptions;
  children: ReactNode;
}

/**
 * Provides an {@link HdmBridgeClient} to the hooks below. When constructing from `options`, the
 * client is rebuilt only when an option field changes — pass a stable `options` object (or a
 * pre-built `client`) to keep it referentially stable across renders.
 */
export function HdmProvider({ client, options, children }: HdmProviderProps): ReactNode {
  const resolved = useMemo(() => {
    if (client) {
      return client;
    }
    if (!options) {
      throw new Error('HdmProvider requires either a `client` or `options` prop');
    }
    return new HdmBridgeClient(options);
  }, [
    client,
    options?.baseUrl,
    options?.token,
    options?.connection,
    options?.fetch,
    options?.timeoutMs,
    options?.validation,
  ]);

  return createElement(HdmContext.Provider, { value: resolved }, children);
}

/** Access the client provided by the nearest {@link HdmProvider}. Throws if used outside one. */
export function useHdmClient(): HdmBridgeClient {
  const client = useContext(HdmContext);
  if (!client) {
    throw new Error('useHdmClient must be used within an <HdmProvider>');
  }
  return client;
}
