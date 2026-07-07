import { describe, expect, it } from 'vitest';
import {
  HdmBridgeClient,
  HdmBridgeError,
  HdmTransportError,
  HdmValidationError,
  isErrorBody,
} from '../src/index.js';
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
    expect(headersOf(call!.init).accept).toBe('application/json');
    expect(headersOf(call!.init).authorization).toBe('Bearer secret');
    const sent = bodyOf(call!.init) as { params: { amount: number; isCashIn: boolean } };
    expect(sent.params).toEqual({ amount: 5000, isCashIn: true });
  });

  it('uses GET without a body for public metadata endpoints', async () => {
    const { fetch, calls } = ok({ ok: true });
    const client = new HdmBridgeClient({ baseUrl: ' http://bridge.test/ ', fetch });

    await client.health();

    expect(calls[0]!.url).toBe('http://bridge.test/v1/health');
    expect(calls[0]!.init.method).toBe('GET');
    expect(calls[0]!.init.body).toBeUndefined();
    expect(headersOf(calls[0]!.init)['content-type']).toBeUndefined();
  });

  it('omits the connection key and auth header when unset', async () => {
    const { fetch, calls } = ok({});
    const client = new HdmBridgeClient({ baseUrl: 'http://bridge.test', fetch });

    await client.operators();

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

  it('rejects invalid configuration before issuing a request', async () => {
    expect(() => new HdmBridgeClient({ baseUrl: '   ', fetch: ok({}).fetch })).toThrow(TypeError);

    const client = new HdmBridgeClient({ baseUrl: 'http://bridge.test', fetch: ok({}).fetch });
    await expect(client.operators({ timeoutMs: Number.NaN })).rejects.toBeInstanceOf(RangeError);
    await expect(client.operators({ timeoutMs: -1 })).rejects.toBeInstanceOf(RangeError);
  });
});

describe('HdmBridgeClient error handling', () => {
  it('accepts a schema-valid error envelope with a null device code', () => {
    expect(
      isErrorBody({
        error: {
          code: null,
          kind: 'bad_request',
          message: 'missing field',
          retryable: false,
          requires_relogin: false,
          requires_reconnect: false,
        },
      }),
    ).toBe(true);
  });

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

  it('synthesizes a bridge error for non-conforming error JSON', async () => {
    const fetchImpl = (): Promise<Response> =>
      Promise.resolve(
        new Response(JSON.stringify({ message: 'plain failure' }), {
          status: 500,
          statusText: 'Internal Server Error',
        }),
      );
    const client = new HdmBridgeClient({
      baseUrl: 'http://bridge.test',
      fetch: fetchImpl,
    });

    const err = await client.health().catch((e: unknown) => e);

    expect(err).toBeInstanceOf(HdmBridgeError);
    expect((err as HdmBridgeError).kind).toBe('bad_request');
    expect((err as HdmBridgeError).message).toContain('plain failure');
  });

  it('synthesizes a bridge error for invalid error JSON', async () => {
    const fetchImpl = (): Promise<Response> =>
      Promise.resolve(new Response('not json', { status: 502, statusText: 'Bad Gateway' }));
    const client = new HdmBridgeClient({
      baseUrl: 'http://bridge.test',
      fetch: fetchImpl,
    });

    const err = await client.health().catch((e: unknown) => e);

    expect(err).toBeInstanceOf(HdmBridgeError);
    expect((err as HdmBridgeError).status).toBe(502);
    expect((err as HdmBridgeError).message).toBe('not json');
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

  it('wraps invalid success JSON in HdmTransportError', async () => {
    const fetchImpl = (): Promise<Response> => Promise.resolve(new Response('{nope', { status: 200 }));
    const client = new HdmBridgeClient({
      baseUrl: 'http://bridge.test',
      fetch: fetchImpl,
    });

    const err = await client.health().catch((e: unknown) => e);
    expect(err).toBeInstanceOf(HdmTransportError);
    expect((err as HdmTransportError).message).toContain('invalid JSON');
  });

  it('treats an empty success body as an empty object', async () => {
    const fetchImpl = (): Promise<Response> => Promise.resolve(new Response(null, { status: 204 }));
    const client = new HdmBridgeClient({
      baseUrl: 'http://bridge.test',
      fetch: fetchImpl,
    });

    await expect(client.printLastReceipt()).resolves.toEqual({});
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

    const err = await client.operators().catch((e: unknown) => e);
    expect(err).toBeInstanceOf(HdmTransportError);
  });

  it('forwards an external abort signal', async () => {
    const controller = new AbortController();
    const fetchImpl = (_input: string, init?: RequestInit): Promise<Response> => {
      expect(init?.signal).toBe(controller.signal);
      controller.abort(new Error('cancelled'));
      return Promise.reject(new Error('cancelled'));
    };
    const client = new HdmBridgeClient({
      baseUrl: 'http://bridge.test',
      fetch: fetchImpl as unknown as typeof fetch,
    });

    const err = await client.operators({ signal: controller.signal }).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(HdmTransportError);
  });
});

describe('HdmBridgeClient validation hooks', () => {
  it('validates a request envelope before sending', async () => {
    const { fetch, calls } = ok({});
    const client = new HdmBridgeClient({
      baseUrl: 'http://bridge.test',
      fetch,
      validation: {
        requestValidators: {
          '/v1/cash': () => {
            throw new Error('bad request shape');
          },
        },
      },
    });

    const err = await client.cashInOut({ amount: 1, isCashIn: true }).catch((e: unknown) => e);

    expect(err).toBeInstanceOf(HdmValidationError);
    expect((err as HdmValidationError).direction).toBe('request');
    expect((err as HdmValidationError).path).toBe('/v1/cash');
    expect(calls).toHaveLength(0);
  });

  it('validates a successful response after JSON parsing', async () => {
    const { fetch } = ok({ status: 'wrong' });
    const client = new HdmBridgeClient({
      baseUrl: 'http://bridge.test',
      fetch,
      validation: {
        responseValidators: {
          '/v1/health': (value: unknown) => {
            if (
              typeof value !== 'object' ||
              value === null ||
              !('status' in value) ||
              value.status !== 'ok'
            ) {
              throw new Error('bad response shape');
            }
          },
        },
      },
    });

    const err = await client.health().catch((e: unknown) => e);

    expect(err).toBeInstanceOf(HdmValidationError);
    expect((err as HdmValidationError).direction).toBe('response');
    expect((err as HdmValidationError).status).toBe(200);
  });

  it('can disable response validation while keeping validators configured', async () => {
    const { fetch } = ok({ status: 'wrong' });
    const client = new HdmBridgeClient({
      baseUrl: 'http://bridge.test',
      fetch,
      validation: {
        responses: false,
        responseValidators: {
          '/v1/health': () => {
            throw new Error('should not run');
          },
        },
      },
    });

    await expect(client.health()).resolves.toEqual({ status: 'wrong' });
  });
});
