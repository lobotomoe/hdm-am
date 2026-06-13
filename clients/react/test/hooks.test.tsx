import { render, screen, waitFor } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { act } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { HdmBridgeClient } from '@hdm-am/client';
import { HdmProvider, useHdmInfo, useLogin } from '../src/index.js';

afterEach(() => {
  vi.restoreAllMocks();
});

/** Build a client whose every method is a stub resolving to `result`. */
function stubClient(result: unknown): HdmBridgeClient {
  const client = new HdmBridgeClient({ baseUrl: 'http://bridge.test' });
  return new Proxy(client, {
    get(target, prop, receiver) {
      if (prop === 'info' || prop === 'login') {
        return () => Promise.resolve(result);
      }
      return Reflect.get(target, prop, receiver) as unknown;
    },
  });
}

function wrapper(client: HdmBridgeClient) {
  return function Wrapper({ children }: { children: ReactNode }): ReactNode {
    return createElement(HdmProvider, { client, children });
  };
}

describe('useHdmInfo', () => {
  it('loads then exposes data', async () => {
    const info = { name: 'hdm-bridge', version: '0.2.0' };
    const client = stubClient(info);

    function Probe(): ReactNode {
      const { data, loading } = useHdmInfo();
      if (loading) return createElement('span', null, 'loading');
      return createElement('span', null, (data as { name: string } | undefined)?.name ?? 'none');
    }

    render(createElement(wrapper(client), null, createElement(Probe, null)));
    expect(screen.getByText('loading')).toBeDefined();
    await waitFor(() => {
      expect(screen.getByText('hdm-bridge')).toBeDefined();
    });
  });
});

describe('useLogin', () => {
  it('runs the mutation on demand and tracks its result', async () => {
    const client = stubClient({ ok: true });
    let captured: ReturnType<typeof useLogin> | undefined;

    function Probe(): ReactNode {
      captured = useLogin();
      return createElement('span', null, captured.loading ? 'running' : 'idle');
    }

    render(createElement(wrapper(client), null, createElement(Probe, null)));
    expect(screen.getByText('idle')).toBeDefined();

    await act(async () => {
      await captured!.mutate();
    });

    expect(captured!.data).toEqual({ ok: true });
    expect(captured!.error).toBeUndefined();
  });
});
