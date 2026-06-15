import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { act } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { HdmBridgeClient } from '@hdm-am/client';
import { HdmProvider, useHdmClient, useHdmInfo, useLogin } from '../src/index.js';

afterEach(() => {
  vi.restoreAllMocks();
  cleanup();
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

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void; reject: (error: unknown) => void } {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('HdmProvider', () => {
  it('rebuilds a constructed client when validation options change', () => {
    const seen: HdmBridgeClient[] = [];

    function Probe(): ReactNode {
      seen.push(useHdmClient());
      return createElement('span', null, 'ready');
    }

    const firstValidation = { responses: false };
    const secondValidation = { responses: true };
    const { rerender } = render(
      createElement(HdmProvider, {
        options: { baseUrl: 'http://bridge.test', validation: firstValidation },
        children: createElement(Probe, null),
      }),
    );
    const firstClient = seen.at(-1);

    rerender(
      createElement(HdmProvider, {
        options: { baseUrl: 'http://bridge.test', validation: secondValidation },
        children: createElement(Probe, null),
      }),
    );

    expect(seen.at(-1)).toBeInstanceOf(HdmBridgeClient);
    expect(seen.at(-1)).not.toBe(firstClient);
  });
});

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

  it('does not run while disabled', () => {
    const client = new HdmBridgeClient({ baseUrl: 'http://bridge.test' });
    const info = vi.spyOn(client, 'info').mockResolvedValue({
      default_device_configured: false,
      name: 'hdm-bridge',
      operations: [],
      spec_version: '0.7.3',
      version: '0.2.0',
    });

    function Probe(): ReactNode {
      const result = useHdmInfo({ enabled: false });
      return createElement('span', null, result.loading ? 'loading' : 'idle');
    }

    render(createElement(wrapper(client), null, createElement(Probe, null)));

    expect(screen.getByText('idle')).toBeDefined();
    expect(info).not.toHaveBeenCalled();
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

  it('keeps the latest mutation result when concurrent calls settle out of order', async () => {
    const first = deferred<{ ok: boolean; seq: number }>();
    const second = deferred<{ ok: boolean; seq: number }>();
    const client = new HdmBridgeClient({ baseUrl: 'http://bridge.test' });
    vi.spyOn(client, 'login')
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    let captured: ReturnType<typeof useLogin> | undefined;

    function Probe(): ReactNode {
      captured = useLogin();
      return createElement('span', null, captured.loading ? 'running' : 'idle');
    }

    render(createElement(wrapper(client), null, createElement(Probe, null)));

    let firstRun!: Promise<unknown>;
    let secondRun!: Promise<unknown>;
    act(() => {
      firstRun = captured!.mutate();
      secondRun = captured!.mutate();
    });
    expect(screen.getByText('running')).toBeDefined();

    await act(async () => {
      second.resolve({ ok: true, seq: 2 });
      await secondRun;
    });
    expect(captured!.data).toEqual({ ok: true, seq: 2 });
    expect(screen.getByText('running')).toBeDefined();

    await act(async () => {
      first.resolve({ ok: true, seq: 1 });
      await firstRun;
    });
    expect(captured!.data).toEqual({ ok: true, seq: 2 });
    expect(screen.getByText('idle')).toBeDefined();
  });

  it('reset clears state and ignores an older in-flight mutation result', async () => {
    const pending = deferred<{ ok: boolean }>();
    const client = new HdmBridgeClient({ baseUrl: 'http://bridge.test' });
    vi.spyOn(client, 'login').mockReturnValueOnce(pending.promise);
    let captured: ReturnType<typeof useLogin> | undefined;

    function Probe(): ReactNode {
      captured = useLogin();
      return createElement('span', null, captured.loading ? 'running' : 'idle');
    }

    render(createElement(wrapper(client), null, createElement(Probe, null)));

    let run!: Promise<unknown>;
    act(() => {
      run = captured!.mutate();
    });
    expect(screen.getByText('running')).toBeDefined();

    act(() => {
      captured!.reset();
    });
    expect(screen.getByText('idle')).toBeDefined();

    await act(async () => {
      pending.resolve({ ok: true });
      await run;
    });
    expect(captured!.data).toBeUndefined();
    expect(captured!.error).toBeUndefined();
    expect(screen.getByText('idle')).toBeDefined();
  });
});
