# HDM bridge — JavaScript/TypeScript clients

A pnpm workspace of browser/Node packages for the [HDM bridge](../bridge/), all generated from the
bridge's OpenAPI document so they cannot drift from the Rust implementation.

```
hdm_am types (serde + schemars)  ─┐
bridge types (PartialConn, …)    ─┤─ dump-openapi → docs/openapi.json (OpenAPI 3.1, CI-checked)
canonical OPERATIONS table       ─┘        │
                                           ├─ served at GET /v1/openapi.json  (+ Scalar UI at /docs)
                                           │
                                  openapi-typescript → @hdm-am/client generated types (gen:check)
                                           │
                                  HdmBridgeClient (isomorphic, zero deps)
                                           │
                                  @hdm-am/react (provider + hooks, react peer dep)
                                           │
                                  demo (Vite + React + shadcn/ui)
```

Each layer is generated from the one above and verified in CI: the Rust `check` job keeps
`docs/openapi.json` in sync with the types, and the `js` job's `gen:check` keeps the generated TS
types in sync with that document.

## Packages

| Package | What it is |
|---|---|
| [`@hdm-am/client`](typescript/) | Isomorphic TS client — one typed method per operation, typed errors, `fetch`-based (browser / Node 18+). Zero runtime deps. |
| [`@hdm-am/react`](react/) | React provider and typed hooks over the client. `react` is the only peer dependency. |
| [`demo`](demo/) | Private Vite + React + shadcn/ui app that drives a real device from the browser. |

## Develop

```bash
cd clients
pnpm install
pnpm -r gen:check   # generated types match docs/openapi.json
pnpm -r typecheck
pnpm -r lint
pnpm -r test
pnpm -r build
```

After changing a Rust wire type, regenerate both layers:

```bash
# from the repo root — refresh the OpenAPI document
cargo run -p hdm-am-bridge --example dump-openapi --features schema
# then refresh the TS types
pnpm --filter @hdm-am/client gen
```

## Run the demo against a device

1. Start the bridge with an allow-origin for the dev server (loopback dev — no token):

   ```bash
   HDM_HOST=192.168.1.5 cargo run -p hdm-am-bridge -- \
     --insecure-no-auth --allow-origin http://localhost:5173
   ```

   (Or omit `HDM_HOST` and enter the device connection in the demo's Connection form.)

2. Start the demo and open <http://localhost:5173>:

   ```bash
   pnpm --filter @hdm-am/demo dev
   ```

3. Connect → verify login → print a receipt. For a production page use a real
   `HDM_BRIDGE_TOKEN` and an HTTPS allow-origin instead of `--insecure-no-auth`.

## Generate a client in another language

The document is served by a running bridge, so any OpenAPI generator can consume it:

```bash
npx openapi-typescript http://127.0.0.1:8077/v1/openapi.json -o client.ts
# or: openapi-generator-cli generate -i http://127.0.0.1:8077/v1/openapi.json -g <lang>
```
