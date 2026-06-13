import { describe, expect, it } from 'vitest';
import { HdmBridgeClient, HdmBridgeError, HdmTransportError } from '../src/index.js';
import type { ErrorBody } from '../src/index.js';

interface Captured {
  url: string;
  init: RequestInit;
}

/** A stub `fetch` returning a JSON 200, capturing each call for assertions. */
function ok(body: unknown): { fetch: typeof fetch; calls: Captured[] } {
  const calls: Captured[] = [];
  const fetchImpl = (input: string, init?: RequestInit): Promise<Response> => {
    calls.push({ url: input, init: init ?? {} });
    return Promise.resolve(
      new Response(JSON.stringify(body), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    );
  };
  return { fetch: fetchImpl as unknown as typeof fetch, calls };
}

function headersOf(init: RequestInit): Record<string, string> {
  return (init.headers ?? {}) as Record<string, string>;
}

function bodyOf(init: RequestInit): unknown {
  return JSON.parse(init.body as string);
}

describe('HdmBridgeClient request shaping', () => {
  it('posts the typed envelope with bearer auth to the right path', async () => {
    const { fetch, calls } = ok({});
    const client = new HdmBridgeClient({ baseUrl: 'http://bridge.test/', token: 'secret', fetch });

    await client.cashInOut({ amount: 5000, isCashIn: true });

    const call = calls[0];
    expect(call).toBeDefined();
    expect(call!.init.method).toBe('POST');
    expect(call!.url).toBe('http://bridge.test/v1/cash');
    expect(headersOf(call!.init).authorization).toBe('Bearer secret');
    const sent = bodyOf(call!.init) as { params: { amount: number; isCashIn: boolean } };
    expect(sent.params).toEqual({ amount: 5000, isCashIn: true });
  });

  it('omits the connection key and auth header when unset', async () => {
    const { fetch, calls } = ok({});
    const client = new HdmBridgeClient({ baseUrl: 'http://bridge.test', fetch });

    await client.probe();

    const sent = bodyOf(calls[0]!.init) as Record<string, unknown>;
    expect(sent).not.toHaveProperty('connection');
    expect(headersOf(calls[0]!.init).authorization).toBeUndefined();
  });

  it('merges a per-call connection over the client default field-by-field', async () => {
    const { fetch, calls } = ok({ ok: true });
    const client = new HdmBridgeClient({
      baseUrl: 'http://bridge.test',
      connection: { host: '10.0.0.5', password: 'pw', cashier: 3, pin: '1234' },
      fetch,
    });

    await client.login({ connection: { host: '192.168.1.4', cashier: 7 } });

    const sent = bodyOf(calls[0]!.init) as { connection: Record<string, unknown> };
    expect(sent.connection).toEqual({
      host: '192.168.1.4', // override wins
      password: 'pw', // default kept
      cashier: 7, // override wins
      pin: '1234', // default kept
    });
  });
});

describe('HdmBridgeClient error handling', () => {
  it('maps a device error envelope to a typed HdmBridgeError', async () => {
    const envelope: ErrorBody = {
      error: {
        kind: 'device_error',
        code: 174,
        message: 'receipt-to-return does not exist',
        retryable: false,
        requires_relogin: false,
        requires_reconnect: false,
      },
    };
    const fetchImpl = (): Promise<Response> =>
      Promise.resolve(
        new Response(JSON.stringify(envelope), {
          status: 422,
          headers: { 'content-type': 'application/json' },
        }),
      );
    const client = new HdmBridgeClient({
      baseUrl: 'http://bridge.test',
      fetch: fetchImpl,
    });

    const err = await client
      .printReturn({ crn: '31008940', returnTicketId: 205 })
      .catch((e: unknown) => e);

    expect(err).toBeInstanceOf(HdmBridgeError);
    const bridgeErr = err as HdmBridgeError;
    expect(bridgeErr.status).toBe(422);
    expect(bridgeErr.kind).toBe('device_error');
    expect(bridgeErr.code).toBe(174);
    expect(bridgeErr.requiresRelogin).toBe(false);
    expect(bridgeErr.message).toContain('does not exist');
  });

  it('wraps a network failure in HdmTransportError', async () => {
    const fetchImpl = (): Promise<Response> => Promise.reject(new Error('connection refused'));
    const client = new HdmBridgeClient({
      baseUrl: 'http://bridge.test',
      fetch: fetchImpl,
    });

    const err = await client.health().catch((e: unknown) => e);
    expect(err).toBeInstanceOf(HdmTransportError);
    expect((err as HdmTransportError).cause).toBeInstanceOf(Error);
  });

  it('aborts via timeout and reports a transport error', async () => {
    const fetchImpl = (_input: string, init?: RequestInit): Promise<Response> =>
      new Promise<Response>((_resolve, reject) => {
        init?.signal?.addEventListener('abort', () => {
          reject(init.signal?.reason as Error);
        });
      });
    const client = new HdmBridgeClient({
      baseUrl: 'http://bridge.test',
      fetch: fetchImpl as unknown as typeof fetch,
      timeoutMs: 10,
    });

    const err = await client.probe().catch((e: unknown) => e);
    expect(err).toBeInstanceOf(HdmTransportError);
  });
});
