# hdm-am

A Rust client for the Armenian fiscal cash register protocol — the spec published by the State Revenue Committee of Armenia (ՊԵԿ) for integrating external (commercial) software with fiscal cash registers (Հսկիչ Դրամարկղային Մեքենա — **HDM**, ՀԴՄ).

Pick whichever piece fits your project — they all speak to the same device:

- **`hdm` (CLI)** — a command-line tool that runs every protocol operation. The quickest way to talk to a device.
- **`hdm-am` (library)** — the Rust crate: wire framing, encryption, and one typed request/response per operation.
- **`hdm-bridge` (HTTP server)** — a localhost HTTP server that lets a **browser / web app** reach the device (a browser can't open a raw TCP socket).
- **`hdm-app` (GUI)** — a native Slint desktop app (with Android/iOS packaging scaffolds) that exposes every operation with buttons.

## Install

Prebuilt binaries for **macOS** (Apple Silicon + Intel), **Linux** (x86-64 + ARM64), and **Windows** (x86-64) are attached to every [GitHub release](https://github.com/lobotomoe/hdm-am/releases). The installer scripts below pick the right binary for your machine and add it to your `PATH`.

### CLI (`hdm`)

**macOS / Linux:**

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/lobotomoe/hdm-am/releases/latest/download/hdm-am-cli-installer.sh | sh
```

**Windows (PowerShell):**

```powershell
powershell -ExecutionPolicy Bypass -c "[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; irm https://github.com/lobotomoe/hdm-am/releases/latest/download/hdm-am-cli-installer.ps1 | iex"
```

The `Tls12` prefix forces TLS 1.2 so the download also works on older, unpatched Windows; `-ExecutionPolicy Bypass` avoids the "running scripts is disabled" error. If anything goes wrong, see [Windows troubleshooting](#windows-troubleshooting).

**From source** (any OS with [Rust](https://rustup.rs) ≥ 1.85):

```sh
cargo install --git https://github.com/lobotomoe/hdm-am hdm-am-cli
```

Verify with `hdm --help`.

### HTTP bridge (`hdm-bridge`)

Install this if you want to drive the device from a browser or web app.

**macOS / Linux:**

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/lobotomoe/hdm-am/releases/latest/download/hdm-am-bridge-installer.sh | sh
```

**Windows (PowerShell):**

```powershell
powershell -ExecutionPolicy Bypass -c "[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; irm https://github.com/lobotomoe/hdm-am/releases/latest/download/hdm-am-bridge-installer.ps1 | iex"
```

**From source:**

```sh
cargo install --git https://github.com/lobotomoe/hdm-am hdm-am-bridge
```

### Rust library (`hdm-am`)

Published on [crates.io](https://crates.io/crates/hdm-am):

```sh
cargo add hdm-am
```

### GUI app (`hdm-app`)

Build and run it from source (needs a Rust toolchain):

```sh
cargo run -p hdm-am-app
```

Android/iOS toolchain setup and build commands live in [`crates/hdm-am-app/README.md`](crates/hdm-am-app/README.md).

### Updating

Check what you have installed first:

```sh
hdm --version          # or: hdm-bridge --version
```

Then update by the same method you installed with:

- **Self-updater (simplest).** Installs from the release scripts bundle a standalone updater that
  fetches the latest release and replaces the binary in place — just run it, on any OS:

  ```sh
  hdm-am-cli-update        # updates the hdm CLI
  hdm-am-bridge-update     # updates the hdm-bridge
  ```

  (The command is named after the package, not the `hdm` binary. It reads the install receipt written
  at install time, so it only works for installs done via the installer scripts above.)

- **Installer script (CLI / bridge).** Re-run the exact same install command — it downloads the
  latest release and replaces the existing binary in place. For example, on macOS / Linux:

  ```sh
  curl --proto '=https' --tlsv1.2 -LsSf https://github.com/lobotomoe/hdm-am/releases/latest/download/hdm-am-cli-installer.sh | sh
  ```

  (Windows: re-run the PowerShell one-liner. For the bridge, use the `hdm-am-bridge-installer` script.)
  To pin a specific version instead of the latest, swap `latest` in the URL for a release tag, e.g.
  `.../releases/download/v0.6.0/hdm-am-cli-installer.sh`.

- **From source (`cargo install --git`).** Re-run the install command with `--force` to rebuild from
  the latest commit:

  ```sh
  cargo install --git https://github.com/lobotomoe/hdm-am hdm-am-cli --force
  ```

- **Rust library (`hdm-am`).** Run `cargo update -p hdm-am` for a compatible (`0.6.x`) bump, or
  `cargo add hdm-am@<version>` to move to a newer minor (e.g. `0.7`).

- **GUI from source.** `git pull` in your clone, then `cargo run -p hdm-am-app` again.

### Windows troubleshooting

The Windows binaries statically link the C runtime, so there is **no Visual C++ Redistributable to
install** — they run on a clean Windows 10/11. If the installer or the tool misbehaves, this covers
the common cases (most also apply to the `hdm-bridge` installer — just swap `HDM_AM_CLI` for
`HDM_AM_BRIDGE` in the environment-variable names).

- **`'hdm' is not recognized` right after installing.** The installer adds itself to `PATH`, but an
  already-open terminal keeps its old environment. **Open a new terminal window** and try again. If
  it is still missing, the binary lives in `%USERPROFILE%\.cargo\bin` — confirm that folder is on
  your user `PATH` (Start → "Edit environment variables for your account"), or run it by full path.

- **`Could not create SSL/TLS secure channel`.** The machine is defaulting to an outdated TLS
  version. Use the install command **exactly as shown above** — it forces TLS 1.2. The permanent
  machine-wide fix (for IT) is to run Windows Update, or enable strong crypto:

  ```powershell
  Set-ItemProperty 'HKLM:\SOFTWARE\Microsoft\.NETFramework\v4.0.30319' SchUseStrongCrypto 1 -Type DWord
  Set-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\.NETFramework\v4.0.30319' SchUseStrongCrypto 1 -Type DWord
  ```

- **`running scripts is disabled on this system`.** The `-ExecutionPolicy Bypass` prefix was dropped
  — copy the full command. If a corporate Group Policy enforces the policy machine-wide, ask IT (a
  per-process bypass cannot override a locked-down `MachinePolicy`).

- **"Windows protected your PC" / antivirus warning.** The recommended PowerShell install avoids
  SmartScreen. If you instead downloaded a binary by hand and Windows blocks it, choose **More info →
  Run anyway**; if Microsoft Defender quarantines it, restore the file and add an exclusion. The
  binaries are unsigned for now, so a fresh release can trip a reputation warning.

- **Behind a corporate proxy or firewall.** The installer reads the `HTTPS_PROXY` (or `ALL_PROXY`)
  environment variable — set it before running:

  ```powershell
  $env:HTTPS_PROXY = "http://proxy.host:8080"   # add user:pass@ if the proxy needs auth
  ```

  IT must also allow-list `github.com`, `api.github.com`, and `objects.githubusercontent.com` (the
  release CDN), not just `github.com`.

- **Your Windows username has a space** (e.g. `C:\Users\John Smith`). A known cargo-dist bug can break
  the default install. Work around it by installing to a space-free folder, then add its `bin` to
  `PATH`:

  ```powershell
  $env:HDM_AM_CLI_INSTALL_DIR = "C:\hdm"
  powershell -ExecutionPolicy Bypass -c "[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; irm https://github.com/lobotomoe/hdm-am/releases/latest/download/hdm-am-cli-installer.ps1 | iex"
  ```

- **Very old Windows.** The installer needs **PowerShell 5+** (built into Windows 10/11). Windows 7/8
  are unsupported; install from source there, or upgrade PowerShell.

## Quick start

Point the tool at your device with flags or `HDM_*` environment variables:

```sh
export HDM_HOST=10.0.0.5 HDM_PORT=1025 HDM_PASSWORD=<hdm-password> HDM_CASHIER=3 HDM_PIN=1234
```

Then run your first commands:

```sh
hdm operators                                # list operators & departments — a quick connectivity check (no login)
hdm receipt --mode simple --cash 10 --dep 1  # print a fiscal receipt (asks to confirm first)
hdm --json datetime                          # machine-readable JSON on stdout
```

From here, jump to whichever interface you're using:

- [Command-line tool](#command-line-tool) — every operation as a subcommand
- [Rust library](#rust-library) — integrate the protocol into your own program
- [HTTP bridge](#http-bridge) — reach the device from a browser
- [GUI app](#gui-app) — click-through desktop app

## Command-line tool

Connection parameters come from flags or the `HDM_*` environment variables. The CLI exposes all 16
protocol operations; run `hdm --help` or `hdm <command> --help` for the full argument surface.

```sh
export HDM_HOST=10.0.0.5 HDM_PORT=1025 HDM_PASSWORD=<hdm-password> HDM_CASHIER=3 HDM_PIN=1234

hdm operators                                        # list operators & departments (no login)
hdm receipt --mode simple --cash 10 --dep 1          # print a fiscal receipt (prompts first)
hdm receipt --mode products --card 10 --items items.json --use-ext-pos --rrn 123456789012 --terminal-id 12345678 --emark <code>
hdm report --kind x                                  # interim X-report
hdm report --kind x --transaction-type 1             # X-report filtered by transaction type
hdm lookup-receipt --receipt-id 123 --crn 51815332   # read-only receipt lookup
hdm return --crn 51815332 --ticket 123 --return-items return-items.json --emark <code>
hdm --json datetime                                  # machine-readable output to stdout
```

`receipt --items` expects a JSON array of `ReceiptItem` objects. `return --return-items` expects a
JSON array like `[{"rpid":100,"quantity":1}]`, using the item row IDs returned by
`lookup-receipt`.

Irreversible operations (receipt, return, cash, Z-report) prompt for confirmation unless `--yes` is passed. `-v`/`-vv` raise log verbosity (logs go to stderr; `-vv` traces the raw decrypted payloads).

**Refund not going through?** If a return is rejected with device error **174** or **185**, the original
sale usually hasn't finished syncing with the tax authority yet (or the terminal is showing a modal).
Run `hdm time-sync`, make sure the terminal screen is idle, then retry the return. Note that
`--ticket` is the original sale's **sequence number** (`rseq`), not the printed fiscal number.

## Rust library

```rust
use std::net::TcpStream;
use std::time::Duration;
use hdm_am::{Client, InMemorySeq, PrintReceiptRequest, PrintMode, Decimal};

let tcp = TcpStream::connect("10.0.0.5:1025")?;
tcp.set_read_timeout(Some(Duration::from_secs(50)))?;
let mut client = Client::new(tcp, "<hdm-password>", InMemorySeq::default());

client.login(3, "1234")?;                 // cashier id + PIN
let receipt = client.print_receipt(PrintReceiptRequest {
    mode: PrintMode::Simple,
    paid_amount: Decimal::new(1000, 2),   // 10.00 cash
    paid_amount_card: Decimal::ZERO,
    partial_amount: Decimal::ZERO,
    pre_payment_amount: Decimal::ZERO,
    dep: Some(1),
    partner_tin: None,
    use_ext_pos: false,
    payment_system: None,
    rrn: None,
    terminal_id: None,
    e_marks: vec![],
    items: vec![],
})?;
println!("fiscal #{} (seq {})", receipt.fiscal, receipt.rseq);
client.logout()?;
```

All monetary and quantity fields use `rust_decimal::Decimal` (re-exported as `hdm_am::Decimal`); the wire encoding stays a JSON number.

## GUI app

The native GUI lives in [`crates/hdm-am-app/`](crates/hdm-am-app/) and uses Slint without a webview. It has buttons for all 16
protocol operations. HDM calls run on a worker thread so the UI
event loop is not blocked by the protocol's long response timeout.

```sh
cargo run -p hdm-am-app
```

The crate is both a desktop binary and a library:

- `crates/hdm-am-app/src/main.rs` — desktop entrypoint (`hdm-app`).
- `crates/hdm-am-app/src/lib.rs` — shared app runner plus Android `android_main` hook and iOS backend selection.
- `crates/hdm-am-app/ui/main.slint` — compiled Slint UI markup.
- `crates/hdm-am-app/src/bridge.rs` — UI callbacks, validation, TCP connection setup, and background HDM calls.
- `crates/hdm-am-app/ios/` — XcodeGen project template that delegates the iOS executable build to Cargo.

Current platform status:

- Desktop macOS/Windows/Linux: directly runnable with `cargo run -p hdm-am-app`.
- Android: scaffolded with `cdylib`, `android_main`, `cargo-apk` metadata, and TCP network
  permissions.
- iOS: scaffolded with an XcodeGen project, Cargo build script, Winit + Skia backend selection, and
  Local Network privacy text.

See [`crates/hdm-am-app/README.md`](crates/hdm-am-app/README.md) for Android/iOS toolchain setup and build commands.

The first GUI iteration deliberately keeps structured payload editing simple: receipt items,
return-item lists, and header/footer config are loaded from JSON file paths using the same shapes as
the CLI/library types; logo upload reads a BMP path and Base64-encodes it before sending. Operations
that print, submit data, configure the device, or otherwise change state require the `Confirm side
effect` checkbox before dispatch.

Before dispatching a request, the GUI validates connection settings and operation-specific fields:
numeric ranges, money precision, required department/cashier/PIN fields, CRN/TIN/RRN/terminal ID
formats, eMark length/character rules, report ranges, receipt/return item JSON, header/footer text
limits, and BMP logo depth. Device responses are formatted as task-oriented summaries; HDM error
codes are shown with their meaning and a suggested recovery action.

The GUI includes a Demo mode for store review, training, and first-run checks without fiscal
hardware. Demo mode returns synthetic responses for every operation and sends no network traffic.
Privacy policy and store-readiness notes live in [`PRIVACY.md`](PRIVACY.md) and
[`docs/store-compliance.md`](docs/store-compliance.md).

Slint is pinned to `=1.13.1` because it is the latest checked version whose `rust-version` matches
this workspace's MSRV (`1.85`). Newer Slint releases currently require Rust 1.88+. Slint's runtime is
licensed separately (`GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR
LicenseRef-Slint-Software-3.0`), so binary distribution of the GUI must account for Slint's license
terms.

## HTTP bridge

A browser can speak HTTP/WebSocket but not raw TCP, while the HDM protocol is raw 3DES-over-TCP. The
[`crates/hdm-am-bridge/`](crates/hdm-am-bridge/) crate closes that gap: `hdm-bridge` is a small localhost HTTP server that takes
JSON on one side and runs the HDM TCP protocol (via `hdm_am::Client`) on the other — one
`POST /v1/<op>` per operation. The server logic is exposed as `hdm_am_bridge::serve` so it can also be
embedded in another process (e.g. the GUI app).

```sh
HDM_BRIDGE_TOKEN=$(openssl rand -hex 16) \
HDM_BRIDGE_ALLOW_ORIGIN=https://your-web-app.example \
HDM_HOST=10.0.0.5 HDM_PASSWORD=… HDM_CASHIER=3 HDM_PIN=1234 \
  hdm-bridge          # listens on 127.0.0.1:8077 by default
```

The `hdm` CLI can also supervise it as a background process (Unix), so you don't have to keep a
terminal open. It runs the `hdm-bridge` binary as a child — the CLI takes on no dependency on the
bridge crate; device connection comes from the usual global `--host`/`--password`/`--cashier`/`--pin`
flags (or `HDM_*` env):

```sh
hdm --host 10.0.0.5 --password … --cashier 3 --pin 1234 \
  bridge start --token "$TOKEN" --allow-origin https://your-web-app.example
hdm bridge status     # running? pid / bind / uptime
hdm bridge stop       # graceful SIGTERM
hdm bridge run        # foreground instead (for a service manager: it execs the bridge)
```

Every operation is a `POST` with a uniform envelope: an optional per-request `connection` override
(merged field-by-field over the configured default device) and the operation's `params` (the library
request type verbatim — `PrintReceiptRequest`, `FiscalReportRequest`, …):

```jsonc
// POST /v1/receipt
{
  "connection": { "host": "10.0.0.5", "cashier": 3 },  // optional; falls back to the configured default
  "params": { "mode": 1, "paidAmount": 1000.0, "paidAmountCard": 0, "partialAmount": 0,
              "prePaymentAmount": 0, "useExtPOS": false, "dep": 1 }
}
```

Routes mirror the CLI: `/v1/operators`, `/v1/login`, `/v1/receipt`, `/v1/receipt/last`,
`/v1/receipt/lookup`, `/v1/return`, `/v1/report`, `/v1/cash`, `/v1/datetime`, `/v1/time-sync`,
`/v1/payment-systems`, `/v1/emark`, `/v1/sample`, `/v1/header-footer`, `/v1/logo`, plus `/v1/health`
(public liveness) and `/v1/info`. Errors render as a stable envelope carrying the device error code
and the library's recovery hints:

```jsonc
{ "error": { "kind": "device_error", "code": 174, "message": "…",
             "retryable": false, "requires_relogin": false, "requires_reconnect": true } }
```

Configuration comes from flags or `HDM_*` / `HDM_BRIDGE_*` environment variables (`--help` lists them).
Because a per-request `connection` can target any host, the bridge is a security boundary: it binds
loopback only, requires `Authorization: Bearer <HDM_BRIDGE_TOKEN>` on every route except `/v1/health`
(refusing to start without a token unless `--insecure-no-auth` is passed), restricts callers to an
explicit `--allow-origin` allow-list, and serializes device access to one session at a time.

**Calling it from an HTTPS page.** `http://127.0.0.1` is a "potentially trustworthy" origin, so mixed
content is not the obstacle. The bridge answers Chrome's Private Network Access preflight
(`Access-Control-Allow-Private-Network: true`), but recent Chrome additionally prompts the user to
allow a connection to a local device. For a frictionless production deployment, terminate TLS on a
loopback domain (a real certificate for a name that resolves to `127.0.0.1`) so the page talks
`https` to `https`; that path is a planned follow-up, not yet shipped.

### Interactive API explorer

The bridge serves a live API explorer at `GET /docs` and the machine-readable **OpenAPI 3.1**
document at `GET /v1/openapi.json`. The document is assembled from the same `schemars`-derived schemas
the handlers serialize — so the contract cannot drift from the code — and every operation carries a
description and a worked request/response example. It is also committed at
[`docs/openapi.json`](docs/openapi.json) (regenerated by
`cargo run -p hdm-am-bridge --example dump-openapi --features schema`, CI-checked with `--check`). Any
generator can consume it off a running bridge:

```bash
npx openapi-typescript http://127.0.0.1:8077/v1/openapi.json -o client.ts
```

## TypeScript packages & web demo

A browser cannot open a raw TCP socket, so the demo never talks to the device directly — it goes
through the bridge. One operation is a single round trip: the React UI calls a typed hook, which
calls a typed client method, which `POST`s JSON to the loopback bridge; the bridge runs the HDM TCP
protocol against the cash register and sends a typed result (or a typed error) back up the same
chain.

```mermaid
flowchart LR
    subgraph browser["Browser tab"]
        direction TB
        ui["demo<br/>Vite + React + shadcn/ui"]
        react["@hdm-am/react<br/>provider + typed hooks"]
        client["@hdm-am/client<br/>typed fetch, one method per operation"]
        ui --> react --> client
    end

    subgraph host["Local machine (loopback only)"]
        direction TB
        bridge["hdm-bridge :8077<br/>axum HTTP server, security boundary<br/>Bearer token, origin allow-list, one session at a time"]
        lib["hdm_am::Client<br/>12-byte header, 3DES-ECB, SHA-256 keys"]
        bridge --> lib
    end

    device["HDM device<br/>fiscal cash register"]

    client ==>|"POST /v1/{op}: JSON + Bearer (HTTP/CORS)"| bridge
    lib ==>|"encrypted JSON (raw TCP)"| device
    device -.->|"encrypted JSON"| lib
    bridge -.->|"2xx: response JSON / non-2xx: { error: code, retryable, ... }"| client
```

The bridge is the only component that holds the device password and decrypts the wire protocol; the
browser only ever sees the typed JSON envelope. Solid arrows are the request path, dashed arrows the
response.

The pnpm workspace builds two publishable packages and one demo app on top of that document:

- **`@hdm-am/client`** — an isomorphic, zero-dependency TS client (one typed method per operation,
  typed errors), with types generated from `docs/openapi.json`.
- **`@hdm-am/react`** — a provider and typed hooks over the client (`react` is the only peer dep).
- **`demo`** — a Vite + React + shadcn/ui app in [`apps/demo`](apps/demo/) that drives a real
  device from the browser.

See [`PACKAGES.md`](PACKAGES.md) for the full pipeline and how to run the demo.

---

# Reference

Everything below documents the protocol itself, observed hardware behaviour, the source spec, and the
crate's internals.

## Scope

The crate speaks the HDM TCP protocol directly:

- 12-byte fixed header with `D5 80 D4 B4 D5 84` magic ("ՀԴՄ" in UTF-8), 2-byte protocol version, 1-byte operation code, 2-byte big-endian payload length.
- 3DES-ECB-PKCS7-encrypted JSON payloads.
- Two-key model: a SHA-256-derived password key for operator login (and the operator/department listing), a session key returned by login for everything after.
- All 16 operations from spec v0.7.3.

It does **not** handle the surrounding business logic — selecting an HDM device, persisting fiscal receipts, deciding what to print. That belongs to the consumer.

## Operation coverage

All 16 operations are implemented and unit-tested. Hardware behaviour is recorded **per device and firmware** below — the protocol is the same across terminals, but what a given firmware actually does (logo rendering, returns, eMark validation) varies, so the status column is firmware-specific.

### Tested devices

| Device | OS / build | Firmware | HDM protocol / software |
|---|---|---|---|
| Newland N950 | Android 6 / Android 12 (`SKQ1.220119.001`) | `D_03_51_00_01010000` | `0.7` / `1.1.0` |

The device reports protocol version `0.7` (matching spec v0.7.x) in its responses, yet accepts the `0.5`-framed requests this crate sends — the request framing version (`05`, fixed by the spec) and the device's reported response version legitimately differ.

### Per-operation status

Each row is keyed by the protocol operation code (the byte sent on the wire, 1–16) and its function name.

| Code | Function | Newland N950 (`D_03_51_00_01010000`) |
|---:|---|---|
| 1 | List operators & departments | OK |
| 2 | Operator login | OK |
| 3 | Operator logout | OK |
| 4 | Print receipt | OK — registered a real fiscal sale |
| 5 | Reprint last receipt | OK |
| 6 | Print return | Needs re-test — previously exercised with codes 6/10 swapped [1] |
| 7 | Header / footer config | OK |
| 8 | Header logo | Accepted (`200`) but not rendered [2] |
| 9 | Fiscal report (X / Z) | OK — both; a Z-report does not lock the device [3] |
| 10 | Get returnable receipt (lookup) | Needs re-test — previously exercised with codes 6/10 swapped [1] |
| 11 | Cash in / out | OK — both directions |
| 12 | Date / time | OK |
| 13 | Receipt sample | OK |
| 14 | Time sync | OK |
| 15 | Payment systems list | OK |
| 16 | Single eMark | Error path only [4] |

**Notes:**

1. **Returns** (*print return*, op 6, and *get returnable receipt*, op 10). These tests were captured while the crate had operation codes 6 and 10 **swapped** relative to the spec's operation-codes table (§4.4.1) — so the recorded responses (`503` for the lookup, `174` for the return) were obtained by sending each request to the *wrong* op code and **do not characterise the corrected operations**; both rows need re-testing on hardware. What still holds independently of the code mix-up: returns key on a `Receipt_ID` that lives only in the receipt-print response's `qr` field, and this firmware omits `qr` entirely (confirmed on the raw decrypted payload, not just `null`), so a real `Receipt_ID` likely has to come out-of-band. The lookup response shape (`ReturnableReceiptResponse`) is modelled from the spec alone and remains **unverified** — see its doc comment.
2. **Header logo.** The protocol accepts a Base64 BMP and returns success, but no custom logo prints (tried 1-bit BMP/PNG at 384×4 and 384×64). The firmware appears to ignore custom header logos.
3. **Z-report.** Verified that a Z-report closes the fiscal shift without locking the device — the next receipt opens a new fiscal day.
4. **Single eMark.** Only the error path is verified: malformed codes return `195`. The success path needs a real, registered GS1 Data Matrix code from a marked product.

Behaviour on other models or firmware versions is unknown — additions to this table are welcome.

## Source spec

The State Revenue Committee of Armenia publishes the integration manual as a PDF on `src.am`. This crate targets **v0.7.3** (2025-04, 34 pages). The original and an unofficial English translation are checked in for offline reference:

- [`docs/history/hdm-protocol-v0.7.3-2025.pdf`](docs/history/hdm-protocol-v0.7.3-2025.pdf) — original Armenian spec from src.am, **authoritative**. It is the newest entry in the version archive below.
- [`docs/spec.md`](docs/spec.md) — English translation (unofficial; for developer convenience). Where the two disagree, trust the PDF — translator's notes in `spec.md` flag the corrections.
- [`docs/history/`](docs/history/) — every published revision from v0.3 (2015) to v0.7.3, archived offline with a per-version index and a wire-protocol changelog. The transport envelope (framing, 3DES-ECB, SHA-256 keys) has been stable since v0.3; the header version byte has been `05` since v0.5.

## Machine-readable schema

JSON Schema for every request/response payload lives in [`docs/schema/`](docs/schema/) — one file
per type, **generated from the Rust types** behind the `schema` feature so they can't drift from the
code. They cover the decrypted JSON bodies (not the binary framing / 3DES envelope); money fields are
JSON numbers and integer-coded enums are integers, matching the wire.

```sh
cargo run -p hdm-am --example dump-schema --features schema             # (re)generate docs/schema/*.json
cargo run -p hdm-am --example dump-schema --features schema -- --check  # CI guard: fail if stale
```

## Design

- `Client<T: Read + Write, S: SequenceProvider>` is generic over its transport and its sequence-number provider — pass a `TcpStream` + `InMemorySeq`/`FileSeq` in production, or any mock in tests.
- Synchronous API. Consumers needing async should wrap calls in `tokio::task::spawn_blocking` or similar.
- No global state. Each `Client` owns its session key and sequence counter.
- Sequence-counter persistence is the consumer's choice (`InMemorySeq`, `FileSeq`, or a custom `SequenceProvider`).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
