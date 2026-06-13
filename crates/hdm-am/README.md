# hdm-am

A Rust client for the Armenian fiscal cash register protocol — the spec published by the State
Revenue Committee of Armenia (ՊԵԿ) for integrating external (commercial) software with fiscal cash
registers (Հսկիչ Դրամարկղային Մեքենա — **HDM**, ՀԴՄ).

This crate is the core library of the [`hdm-am` workspace](https://github.com/lobotomoe/hdm-am): the
wire framing, encryption, and one typed request/response per operation. The CLI (`hdm-am-cli`), HTTP
bridge (`hdm-am-bridge`), and GUI (`hdm-am-app`) all build on top of it.

## Scope

The crate speaks the HDM TCP protocol directly:

- 12-byte fixed header with `D5 80 D4 B4 D5 84` magic ("ՀԴՄ" in UTF-8), 2-byte protocol version,
  1-byte operation code, 2-byte big-endian payload length.
- 3DES-ECB-PKCS7-encrypted JSON payloads.
- Two-key model: a SHA-256-derived password key for operator login (and the operator/department
  listing), a session key returned by login for everything after.
- All 16 operations from spec v0.7.3.

It does **not** handle the surrounding business logic — selecting an HDM device, persisting fiscal
receipts, deciding what to print. That belongs to the consumer.

## Features

- `schema` — derive [`schemars::JsonSchema`] for every wire request/response type. Off by default to
  keep `schemars` out of the default dependency graph. The `dump-schema` example regenerates
  `docs/schema/*.json` from these impls.

## Documentation

See the [workspace README](https://github.com/lobotomoe/hdm-am#readme) for the full project overview
(CLI, HTTP bridge, GUI, TypeScript clients), per-device operation coverage, and the archived protocol
specifications.

## License

Licensed under either of [Apache-2.0](../../LICENSE-APACHE) or [MIT](../../LICENSE-MIT) at your
option.
