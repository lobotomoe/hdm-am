import { describe, expect, it } from 'vitest';
import { HdmValidationError } from '../src/index.js';
import {
  createValidatedClient,
  hdmZodRequestPaths,
  hdmZodRequestValidators,
  hdmZodResponsePaths,
  hdmZodResponseValidators,
} from '../src/zod.js';

interface Captured {
  url: string;
  init: RequestInit;
}

function ok(body: unknown): { fetch: typeof fetch; calls: Captured[] } {
  const calls: Captured[] = [];
  const fetchImpl = (input: string, init?: RequestInit): Promise<Response> => {
    calls.push({ url: input, init: init ?? {} });
    return Promise.resolve(new Response(JSON.stringify(body), { status: 200 }));
  };
  return { fetch: fetchImpl as unknown as typeof fetch, calls };
}

describe('@hdm-am/client/zod', () => {
  it('exports generated request and response validator maps', () => {
    expect(hdmZodRequestPaths).toContain('/v1/receipt');
    expect(hdmZodResponsePaths).toContain('/v1/receipt');
    expect(hdmZodRequestValidators['/v1/receipt']).toBeTypeOf('function');
    expect(hdmZodResponseValidators['/v1/receipt']).toBeTypeOf('function');
  });

  it('validates a request envelope before fetch', async () => {
    const { fetch, calls } = ok({});
    const client = createValidatedClient({ baseUrl: 'http://bridge.test', fetch });

    const err = await client
      .cashInOut({ amount: 'bad', isCashIn: true } as never)
      .catch((e: unknown) => e);

    expect(err).toBeInstanceOf(HdmValidationError);
    expect((err as HdmValidationError).direction).toBe('request');
    expect((err as HdmValidationError).path).toBe('/v1/cash');
    expect(calls).toHaveLength(0);
  });

  it('validates a successful response payload', async () => {
    const { fetch } = ok({ status: 200 });
    const client = createValidatedClient({ baseUrl: 'http://bridge.test', fetch });

    const err = await client.health().catch((e: unknown) => e);

    expect(err).toBeInstanceOf(HdmValidationError);
    expect((err as HdmValidationError).direction).toBe('response');
    expect((err as HdmValidationError).path).toBe('/v1/health');
  });

  it('can opt out of generated response validation', async () => {
    const { fetch } = ok({ status: 200 });
    const client = createValidatedClient(
      { baseUrl: 'http://bridge.test', fetch },
      { responses: false },
    );

    await expect(client.health()).resolves.toEqual({ status: 200 });
  });
});
