# @hdm-am/client

Isomorphic TypeScript client for the [HDM bridge](https://github.com/lobotomoe/hdm-am) — the local
HTTP gateway to an Armenian fiscal cash register (HDM). One typed method per protocol operation,
typed errors, and a zero-dependency default entrypoint. Runs in the browser and in Node 18+ (or any
runtime with `fetch`, or with a `fetch` injected).

The types are generated from the bridge's OpenAPI document, so they cannot drift from the device
protocol.

## Install

```bash
npm install @hdm-am/client
```

Runtime Zod validation is optional:

```bash
npm install @hdm-am/client zod
```

## Usage

```ts
import { HdmBridgeClient, HdmBridgeError } from '@hdm-am/client';

const client = new HdmBridgeClient({
  baseUrl: 'http://127.0.0.1:8077',
  token: process.env.HDM_BRIDGE_TOKEN,
  // Default device connection; override per call if needed.
  connection: { host: '192.168.1.5', password: '...', cashier: 3, pin: '...' },
});

await client.login();

try {
  const receipt = await client.printReceipt({
    mode: 1, // simple
    paidAmount: 10,
    paidAmountCard: 0,
    partialAmount: 0,
    prePaymentAmount: 0,
    useExtPOS: false,
    dep: 1,
  });
  console.log('fiscal number:', receipt.fiscal);
} catch (err) {
  if (err instanceof HdmBridgeError) {
    // Stable machine tag + device/spec code, plus recovery hints.
    console.error(err.kind, err.code, err.message, {
      retryable: err.retryable,
      requiresRelogin: err.requiresRelogin,
      requiresReconnect: err.requiresReconnect,
    });
  }
}
```

A per-call `connection` is merged field-by-field over the client default, which is in turn merged
over the bridge's configured default. Pass `{ signal }` or `{ timeoutMs }` per call to cancel; the
client also accepts a default `timeoutMs` in the constructor.

## Runtime validation

The generated TypeScript types protect authored code. To validate data at the transport boundary,
use the generated Zod schemas from the optional subpath:

```ts
import { HdmValidationError } from '@hdm-am/client';
import { createValidatedClient } from '@hdm-am/client/zod';

const client = createValidatedClient({
  baseUrl: 'http://127.0.0.1:8077',
  connection: { host: '192.168.1.5', password: '...', cashier: 3, pin: '...' },
});

try {
  await client.health();
} catch (err) {
  if (err instanceof HdmValidationError) {
    console.error(err.direction, err.path, err.cause);
  }
}
```

`@hdm-am/client/zod` validates request envelopes before `fetch` and successful response payloads
after JSON parsing. It does not apply Zod defaults or mutate outgoing objects. Pass
`{ requests: false }` or `{ responses: false }` as the second argument to opt out per direction.

## Errors

- **`HdmBridgeError`** — a non-2xx response carrying the bridge's error envelope (`kind`, `code`,
  `message`, `retryable`, `requiresRelogin`, `requiresReconnect`).
- **`HdmTransportError`** — a network failure, CORS rejection, abort, or timeout (no HTTP response).
- **`HdmValidationError`** — an optional local request/response contract failure raised by
  configured validation hooks such as `@hdm-am/client/zod`.

Successful bridge responses are parsed as JSON. A malformed success response is treated as
`HdmTransportError`; malformed error responses are normalized into `HdmBridgeError` so callers can
still branch on one error class for HTTP failures.

## Package

The npm package ships ESM, CJS, `.d.ts`, `.d.cts`, source maps, README, and the MIT/Apache-2.0
license texts. The default entrypoint has no runtime dependencies; `@hdm-am/client/zod` uses `zod`
as an optional peer dependency. The publish gate runs typecheck, lint, tests, build, and a
`pnpm pack` smoke check.

## License

MIT OR Apache-2.0
