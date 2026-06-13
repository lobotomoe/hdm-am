# @hdm-am/client

Isomorphic TypeScript client for the [HDM bridge](https://github.com/lobotomoe/hdm-am) — the local
HTTP gateway to an Armenian fiscal cash register (HDM). One typed method per protocol operation,
typed errors, and zero runtime dependencies. Runs in the browser and in Node 18+ (or any runtime
with `fetch`, or with a `fetch` injected).

The types are generated from the bridge's OpenAPI document, so they cannot drift from the device
protocol.

## Install

```bash
npm install @hdm-am/client
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
over the bridge's configured default. Pass `{ signal }` or `{ timeoutMs }` per call to cancel.

## Errors

- **`HdmBridgeError`** — a non-2xx response carrying the bridge's error envelope (`kind`, `code`,
  `message`, `retryable`, `requiresRelogin`, `requiresReconnect`).
- **`HdmTransportError`** — a network failure, CORS rejection, abort, or timeout (no HTTP response).

## License

MIT OR Apache-2.0
