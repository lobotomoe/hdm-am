# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate version follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The **crate version is independent of the HDM spec version**. The spec revision each release targets
is recorded below and exposed in code as `hdm_am::SPEC_VERSION`.

## [Unreleased]

## [0.5.2] — 2026-06-29

Still targets HDM integration spec **v0.7.3**.

### Added
- **Wire-capture hook (`WireObserver`).** `Client::with_observer` / `set_observer` install an
  observer that receives every request/response exchange — the request plaintext, the exact framed
  bytes on the wire, the response header, the response ciphertext, and the decrypted response
  plaintext — for diagnostics and audit. `on_request` fires *before* the response is read, so a
  request whose reply never comes (a wedged single-session device) is captured together with a
  `Failed` outcome carrying the transport error; framed replies arrive as `Received` for any code,
  including non-200. The bytes are handed over **unmasked** (the password, PIN, and session key are
  visible): redaction, retention, and access control are the consumer's policy, since a masked
  capture is useless for the failures this exists to diagnose. The crate's own `log` output stays
  redacted regardless. No behaviour change when no observer is installed.

## [0.5.1] — 2026-06-29

Still targets HDM integration spec **v0.7.3**.

### Fixed
- **Session is now invalidated after reconnect-class errors, not only relogin-class ones.** After a
  §4.10 "device closes connection" code (e.g. `104` bad sequence number, `103`, `155`) or a transport
  failure, the device has torn down the session server-side. `Client` now clears its session key in
  that case too, so `is_logged_in()` no longer reports a session the protocol has already ended.
  Regression test added.

### Documentation
- **`Client` threading docs spell out device-level serialisation.** The HDM is single-session: the
  consumer must funnel *every* path that touches a given device — including background availability
  probes and scheduled syncs — through one serialisation point (`Mutex<Client>`, a single owning
  task, or a one-slot pool), cross-process where applicable. A single `Client` is already protected
  by `&mut self`; the unguardable risk is a second connection to the same device.

## [0.5.0] — 2026-06-23

Still targets HDM integration spec **v0.7.3**.

### Fixed
- **`GetReturnableReceiptRequest` (op 10) response now decodes against real hardware.** A live Newland
  N950 (fw 1.1.3) returns almost every numeric field as a JSON **string** (`"40.00"`, `"3"`, `"232"`)
  rather than a number, and sends `"totals":null` (not `[]`) for simple/prepayment receipts. The
  previous strict typing failed to deserialize a valid 200 body (it errored on the first string-typed
  integer, `cid`). `ReturnableReceiptResponse` / `ReturnableReceiptItem` now use string-or-number
  tolerant deserializers for every integer/decimal field and a null-tolerant decoder for `totals` /
  `eMarks`; output still serializes as numbers. Verified end-to-end against the live device. This
  makes op 10 usable as a pre-refund returnability check (a 200 means returnable; server code
  185/174/155/156 means not yet — post-sale sync pending).

## [0.4.0] — 2026-06-19

Still targets HDM integration spec **v0.7.3**.

### Fixed
- **Operation codes 6 and 10 were swapped.** Per the spec's operation-codes table (§4.4.1), wire
  code **6** is *print return receipt* and code **10** is *get returnable receipt* (the read-only
  lookup). The crate had them reversed because the spec describes them in section order (§4.5.6 get,
  §4.5.7 print), which is the reverse of the code assignment. `PrintReturnReceiptRequest` now sends
  op 6 and `GetReturnableReceiptRequest` op 10; a regression test pins both. The hardware-test matrix
  rows for these two operations in `README.md` were captured with the codes swapped and are now
  flagged as needing re-test.
- **Spec translation (`docs/spec.md`) corrections.** The operation-codes table had codes 6/10 and
  12/13 swapped, and rendered the recurring qualifier *"(պահանջվում է ընթացիկ սեսիա)"* ("requires an
  active session") as the invented phrase *"(kept for backward compatibility)"* / *"(requires the
  updated version)"* on ~39 lines. Response code **196** (*"Այլ երկրի ծածկագիր"*, an eMark from
  another country) was mistranslated as "other unknown error".

### Changed
- **`ServerErrorKind::OtherUnknownError` renamed to `ForeignCountryEmark`** (code 196) to match its
  real meaning. Breaking for any consumer matching on that variant.
- Human-facing operator listings now resolve assigned department IDs to department names and tax
  regimes in the CLI, native app, and web demo. Missing operator/department names are rendered
  explicitly as `[operator name not provided]` / `[department name not provided]`.

## [0.3.0] — 2026-06-13

Still targets HDM integration spec **v0.7.3**.

### Added
- **`format_receipt` — render a fiscal receipt as human-friendly text.** The device prints the legal
  receipt itself and returns only structured identifiers, so this reconstructs a faithful *summary*
  from the request/response pair (`hdm_am::format_receipt(&PrintReceiptRequest, &ReceiptResponse)`).
  Returns a width- and locale-agnostic `ReceiptLayout` of semantic `ReceiptLine`s with the device's
  own Armenian labels (Գ/Հ = registration number, ԱՀ = serial, ԿՀ = receipt number); render it with
  `ReceiptLayout::to_plain_text(width)` (or `Display` at `DEFAULT_WIDTH`). Not a pixel clone of the
  government layout — per-line VAT and taxation captions depend on data outside the pair and are
  omitted. `hdm receipt` now prints the rendered receipt instead of a field dump.
- **OpenAPI 3.1 document for the bridge.** Assembled from the same `schemars`-derived schemas the
  handlers serialize (`cargo run -p hdm-am-bridge --example dump-openapi --features schema`),
  committed at `docs/openapi.json` and CI-checked with `--check`, served at `GET /v1/openapi.json`,
  and rendered as a Scalar API explorer at `GET /docs`.
- **TypeScript/JavaScript packages and apps** generated from that document, with their own CI job
  (`gen:check`, typecheck, lint, test, build):
  - `@hdm-am/client` — isomorphic, zero-dependency client (one typed method per operation, typed
    `HdmBridgeError`/`HdmTransportError`), types generated by `openapi-typescript`.
  - `@hdm-am/react` — provider and typed query/mutation hooks over the client (`react` peer dep only).
  - `demo` — a private Vite + React + shadcn/ui app under `apps/demo` that drives a real device
    from the browser.

## [0.2.0] — 2026-06-08

Still targets HDM integration spec **v0.7.3**.

### Added
- `hdm-am-bridge` (binary `hdm-bridge`): a localhost HTTP server that exposes the protocol to a
  browser over CORS — one `POST /v1/<op>` per operation, a configured default device with optional
  per-request connection override, bearer-token auth, a CORS origin allow-list, the Private Network
  Access preflight header, and single-session serialization. Embeddable via `hdm_am_bridge::serve`.
- `hdm bridge start` / `stop` / `status` / `run` — the CLI supervises the bridge as a background
  process (Unix) with no compile-time dependency on the bridge crate.
- `Deserialize` for the operation request types and the request-only enums (`PrintMode`,
  `FiscalReportKind`, `ReportFilter`), and `Serialize` for `HdmIdentity` — so payloads round-trip
  through JSON (consumed by the bridge).
- Prebuilt cross-platform binaries (`hdm`, `hdm-bridge`) for Linux (x86_64/aarch64), macOS
  (x86_64/aarch64), and Windows (x86_64) via cargo-dist, with shell/PowerShell installers and
  checksums; CI now builds and tests the library, CLI, and bridge on macOS and Windows.

### Changed
- The bridge shuts down gracefully on `SIGTERM` as well as Ctrl-C/`SIGINT`.

## [0.1.0] — 2026-06-05

Initial release. Targets HDM integration spec **v0.7.3** (April 2025).

### Added
- Synchronous `Client<T, S>` over any `Read + Write` transport and any `SequenceProvider`.
- All 16 protocol operations from spec v0.7.3, one typed request/response per operation.
- Wire framing (`ՀԴՄ` magic, fixed header, big-endian length), 3DES-ECB-PKCS7 envelope, SHA-256
  two-key model (password key + session key).
- `probe::identify` — credential-less endpoint fingerprinting, matched on the protocol major version.
- `InMemorySeq` and crash-aware `FileSeq` sequence-number providers.
- Optional `schema` feature: JSON Schema for every payload, generated by `examples/dump-schema.rs`.
- `hdm-am-cli` (binary `hdm`): one subcommand per operation, text or `--json` output.
- Offline spec archive under `docs/history/` (v0.3–v0.7.3) with a wire-protocol changelog.

[Unreleased]: https://github.com/lobotomoe/hdm-am/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/lobotomoe/hdm-am/releases/tag/v0.4.0
[0.3.0]: https://github.com/lobotomoe/hdm-am/releases/tag/v0.3.0
[0.2.0]: https://github.com/lobotomoe/hdm-am/releases/tag/v0.2.0
[0.1.0]: https://github.com/lobotomoe/hdm-am/releases/tag/v0.1.0
