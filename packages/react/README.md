# @hdm-am/react

React bindings for the [HDM bridge](https://github.com/lobotomoe/hdm-am) — a provider and typed
hooks over [`@hdm-am/client`](https://www.npmjs.com/package/@hdm-am/client). No data-fetching
dependency; `react` is the only peer dependency.

## Install

```bash
npm install @hdm-am/react @hdm-am/client react
```

## Usage

```tsx
import { useMemo } from 'react';
import { HdmProvider, useHdmInfo, usePrintReceipt } from '@hdm-am/react';

function Root() {
  const options = useMemo(
    () => ({
      baseUrl: 'http://127.0.0.1:8077',
      connection: { host: '192.168.1.5', password: '...', cashier: 3, pin: '...' },
    }),
    [],
  );

  return (
    <HdmProvider options={options}>
      <Register />
    </HdmProvider>
  );
}

function Register() {
  const info = useHdmInfo(); // auto-runs: { data, error, loading, refetch }
  const print = usePrintReceipt(); // manual: { mutate, data, error, loading, reset }

  return (
    <>
      <p>{info.data?.name}</p>
      <button
        disabled={print.loading}
        onClick={() =>
          void print.mutate({
            mode: 1,
            paidAmount: 10,
            paidAmountCard: 0,
            partialAmount: 0,
            prePaymentAmount: 0,
            useExtPOS: false,
            dep: 1,
          })
        }
      >
        Print 10 AMD receipt
      </button>
      {print.data ? <span>fiscal {print.data.fiscal}</span> : null}
    </>
  );
}
```

## Hooks

- **Queries** (auto-run, cancellable): `useHdmHealth`, `useHdmInfo`, `useOperators`,
  `usePaymentSystems`, `useDateTime`.
- **Mutations** (manual): `useLogin`, `usePrintReceipt`, `usePrintLastReceipt`, `useLookupReceipt`,
  `usePrintReturn`, `useReport`, `useCashInOut`, `useTimeSync`, `useReceiptSample`, `useEmark`,
  `useHeaderFooter`, `useHeaderLogo`.

Low-level `useQuery` / `useMutation` primitives and `useHdmClient` are exported too. The whole
`@hdm-am/client` surface (types, `HdmBridgeError`) is re-exported for convenience.

`useQuery` cancels stale runs with `AbortSignal` and avoids setting state after unmount.
`useMutation` supports concurrent calls with latest-result-wins state updates while keeping
`loading` true until active calls settle.

## Runtime validation

`HdmProvider` accepts the same options as `HdmBridgeClient`, so generated Zod validation can be
enabled without any React-specific adapter:

```tsx
import { useMemo } from 'react';
import { HdmProvider } from '@hdm-am/react';
import { withZodValidation } from '@hdm-am/client/zod';

function Root() {
  const options = useMemo(
    () =>
      withZodValidation({
        baseUrl: 'http://127.0.0.1:8077',
        connection: { host: '192.168.1.5', password: '...', cashier: 3, pin: '...' },
      }),
    [],
  );

  return (
    <HdmProvider options={options}>
      <Register />
    </HdmProvider>
  );
}
```

Install `zod` only when importing `@hdm-am/client/zod`.

## Package

The npm package ships ESM, CJS, `.d.ts`, `.d.cts`, source maps, README, and the MIT/Apache-2.0
license texts. `react` is a peer dependency and `@hdm-am/client` is a runtime dependency. The
publish gate runs typecheck, lint, tests, build, and a `pnpm pack` smoke check.

## License

MIT OR Apache-2.0
