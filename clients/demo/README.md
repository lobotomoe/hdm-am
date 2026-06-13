# HDM bridge demo

A small Vite + React + shadcn/ui app that connects an HDM fiscal cash register to the browser
through the [bridge](../../bridge/) and drives it: probe, login, list operators/departments, print
a receipt, cash in/out. Built on [`@hdm-am/react`](../react/). Private — not published.

## Run

1. Start the bridge with an allow-origin for the dev server (loopback dev only):

   ```bash
   cargo run -p hdm-am-bridge -- --insecure-no-auth --allow-origin http://localhost:5173
   ```

2. Start the demo and open <http://localhost:5173>:

   ```bash
   pnpm --filter @hdm-am/demo dev
   ```

3. In the **Connection** card, enter the bridge URL and the device connection (host, password,
   cashier, PIN), then **Connect**. Health and Info load immediately; device-touching actions
   (Probe, Login, receipts) run on click.

Secrets (token, password, PIN) are kept only in the tab's `sessionStorage` and dropped when it
closes. For a production page, use a real `HDM_BRIDGE_TOKEN` and an HTTPS allow-origin instead of
`--insecure-no-auth`.

## Notes

- Interactive elements carry `data-testid` hooks (`field-*`, `btn-*`, `print-receipt`,
  `receipt-outcome`, …) for automation.
- The shadcn/ui primitives under `src/components/ui/` are vendored (owned, regenerable via the
  shadcn CLI) and excluded from the workspace lint.
