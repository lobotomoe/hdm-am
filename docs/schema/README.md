# JSON Schema for the HDM wire payloads

Machine-readable [JSON Schema](https://json-schema.org/) (Draft 2020-12) for every request and response
body of the HDM protocol — one file per type. These describe the **decrypted JSON payload only**,
not the binary framing or the 3DES envelope (for those see [`../spec.md`](../spec.md) §4.4).

The files are **generated from this crate's own Rust types**, so they match the implementation
exactly:

- Field names are the on-the-wire names (`paidAmount`, `eMarks`, `returnTicketId`, …).
- Monetary and quantity fields are JSON **numbers** (`number`/`double`), not strings — matching what
  the device actually sends and accepts.
- Integer-coded enums (receipt mode, discount type, taxation kind, report type) are **integers**.
- Each file is self-contained; nested types are inlined under `$defs`.

## Regenerating / checking

```sh
cargo run -p hdm-am --example dump-schema --features schema             # rewrite docs/schema/*.json
cargo run -p hdm-am --example dump-schema --features schema -- --check  # exit non-zero if out of date
```

Because the schemas are derived from the same serde-annotated types the client uses, they cannot
drift from the implementation as long as `--check` is run (e.g. in CI).

## Caveat

[`ReturnableReceiptResponse.json`](ReturnableReceiptResponse.json) (op 6) is modelled from the spec
alone and is **unverified against hardware** — see the note in [`../spec.md`](../spec.md) §4.5.6.
The spec section is internally inconsistent; treat that one schema's field types as approximate.
