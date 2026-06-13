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
import { HdmProvider, useHdmInfo, usePrintReceipt } from '@hdm-am/react';

function Root() {
  return (
    <HdmProvider
      options={{
        baseUrl: 'http://127.0.0.1:8077',
        connection: { host: '192.168.1.5', password: '...', cashier: 3, pin: '...' },
      }}
    >
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

- **Queries** (auto-run, cancellable): `useHdmHealth`, `useHdmInfo`, `useProbe`, `useOperators`,
  `usePaymentSystems`, `useDateTime`.
- **Mutations** (manual): `useLogin`, `usePrintReceipt`, `usePrintLastReceipt`, `useLookupReceipt`,
  `usePrintReturn`, `useReport`, `useCashInOut`, `useTimeSync`, `useReceiptSample`, `useEmark`,
  `useHeaderFooter`, `useHeaderLogo`.

Low-level `useQuery` / `useMutation` primitives and `useHdmClient` are exported too. The whole
`@hdm-am/client` surface (types, `HdmBridgeError`) is re-exported for convenience.

## License

MIT OR Apache-2.0
