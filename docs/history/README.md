# HDM protocol — version archive

Every published revision of the State Revenue Committee's HDM integration manual, from the first
public draft (v0.3, 2015) to the current v0.7.3 (2025). These are the **original Armenian PDFs** as
issued on `petekamutner.am` / `src.am`, kept here so the protocol's history is preserved offline and
independent of the registrar's site (the older ones are already gone from the live server and survive
only on the Wayback Machine).

The current authoritative spec — v0.7.3 — is [`hdm-protocol-v0.7.3-2025.pdf`](hdm-protocol-v0.7.3-2025.pdf)
in this folder (the newest entry below); its English translation is at [`../spec.md`](../spec.md). This
crate implements **v0.7.3**.

## Versions

| Version | PDF created | Pages | Wire header version byte | File |
|---|---|---:|:---:|---|
| v0.3 | 2015-04-22 | 17 | `03` req / `02` resp | [hdm-protocol-v0.3-2015.pdf](hdm-protocol-v0.3-2015.pdf) |
| v0.4 | 2016-04-12 | 27 | `04` | [hdm-protocol-v0.4-2016apr.pdf](hdm-protocol-v0.4-2016apr.pdf) |
| v0.5 | 2017-04-05 | 32 | `05` | [hdm-protocol-v0.5-2017.pdf](hdm-protocol-v0.5-2017.pdf) |
| v0.5.1 | 2018-07-09 | 33 | `05` | [hdm-protocol-v0.5.1-2018.pdf](hdm-protocol-v0.5.1-2018.pdf) |
| v0.6 | 2019-02-02 | 35 | `05` | [hdm-protocol-v0.6-2019.pdf](hdm-protocol-v0.6-2019.pdf) |
| v0.6.1 | 2020-12-23 | 32 | `05` | [hdm-protocol-v0.6.1-2020.pdf](hdm-protocol-v0.6.1-2020.pdf) |
| v0.7 | 2022-12-15 | 37 | `05` | [hdm-protocol-v0.7-2022.pdf](hdm-protocol-v0.7-2022.pdf) |
| v0.7.1 | 2023-06-16 | 35 | `05` | [hdm-protocol-v0.7.1-2022.pdf](hdm-protocol-v0.7.1-2022.pdf) |
| v0.7.2 | 2023-09-15 | 33 | `05` | [hdm-protocol-v0.7.2-2023.pdf](hdm-protocol-v0.7.2-2023.pdf) |
| v0.7.2 | 2025-04-01 (re-export) | 34 | `05` | [hdm-protocol-v0.7.2-2025.pdf](hdm-protocol-v0.7.2-2025.pdf) |
| **v0.7.3** | **2025-04-23** | 34 | `05` | [hdm-protocol-v0.7.3-2025.pdf](hdm-protocol-v0.7.3-2025.pdf) — **current spec** |

Dates are the PDF `CreationDate` metadata (when each file was generated), not the SRC filename token.
Two consequences worth noting: the file kept as `…v0.7.1-2022.pdf` retains the registrar's `2022`
publication token in its name but was actually authored **2023-06-16**; and `…v0.7.2-2025.pdf` is a
2025 re-export of the 2023 v0.7.2 content. All eleven PDFs are distinct files (different checksums).

## What actually changed (wire-protocol view)

Read from the PDFs above (text-extracted and compared). The takeaway for implementers: **the
transport envelope has been stable since the very first draft; only the operation set and message
fields grew.**

- **Framing & crypto are constant from v0.3 → v0.7.3.** Every version uses the same `ՀԴՄ`
  (`D5 80 D4 B4 D5 84`) magic, a fixed-size binary header, 3DES in **ECB** mode with PKCS7 padding
  over **Base64**-wrapped JSON, and **SHA-256**-derived keys (the two-key password/session model).
  An implementation of the envelope works across the whole lineage.
- **The wire header "version" byte is *not* the document version.** It went `03` (v0.3) → `04`
  (v0.4) → `05` (v0.5) and has been **frozen at `05` ever since — through v0.7.3.** So a
  v0.7.3-compliant client correctly sends header version `05`; this is intentional, not stale.
  (This crate's `wire::PROTOCOL_VERSION = [0x00, 0x05]` matches.)
- **Request and response protocol versions may differ — documented since v0.3 (2015).** That spec
  states outright that the request can carry one protocol version while the device replies in
  another (its example: request `0.3`, response `0.2`), and that the HDM "always returns data in its
  own protocol version." This is why a real terminal answers `0.7` to our `05`-framed request: the
  device reports its own protocol version in responses, which a spec-compliant client treats as
  documented, intended behaviour rather than a mismatch to reject.
- **The operation set grew ~8 → 16.** v0.3 defined roughly eight operations (operator list/login/
  logout, print receipt, reprint, return receipt, header/footer + logo config). Later revisions added
  cash in/out, date/time, receipt sample, fiscal reports (X/Z), tax-authority time-sync, payment-
  systems list, and single-eMark checking. By v0.7.2 the set reached the **16 operations** this crate
  implements; v0.7.3 refined fields (notably the return flow) without changing the count.

For a field-level diff of any two versions, compare the PDFs directly — this archive exists so that
comparison is always possible.

## Adopting a new spec version

When the registrar publishes a new manual (e.g. v0.7.4 or v0.8), use this archive as the diff base
rather than re-reading from scratch:

1. **Add the PDF.** Download the new manual, name it `hdm-protocol-vX.Y.Z-YYYY.pdf` (year = its
   `CreationDate` metadata, not the source filename token), drop it in this folder, and add a row to
   the table above plus a provenance link.
2. **Diff against v0.7.3.** `pdftotext -layout` both and compare. Focus on: the wire header version
   byte (has framing finally moved past `05`?), the operation list (§4.5–4.9), and per-operation
   request/response fields.
3. **Apply field changes additively.** New request fields → `Option<T>` with
   `#[serde(skip_serializing_if = "Option::is_none")]`; new response fields → `#[serde(default)]`.
   Never add `#[serde(deny_unknown_fields)]`. Most minor-version changes need nothing more.
4. **Bump the markers.** Update `SPEC_VERSION` in `src/lib.rs`, regenerate `docs/schema/`
   (`cargo run -p hdm-am --example dump-schema --features schema`), and refresh the English `docs/spec.md`.
5. **Bump the crate** by semver: additive/optional → minor; changed public types → major. Record the
   crate↔spec mapping in the changelog.
6. **Only if a new version is genuinely incompatible** (framing change, or request shapes that an
   older firmware in the field can't accept) introduce a per-version profile selected from the
   device's reported version (every response header carries the device's protocol and software
   version) — not before. The current architecture (framing isolated in `wire`, operations separate)
   already leaves that door open.

## Provenance (Wayback Machine snapshots)

| Version | Source snapshot |
|---|---|
| v0.3 | https://web.archive.org/web/20220813210748/https://www.petekamutner.am/Shared/Documents/_ts/_os/New_Generation_CCMs/uc_hhpek_hdm_integration_manual_protocol_v0_3.pdf |
| v0.4 | https://web.archive.org/web/20220813210802/https://www.petekamutner.am/Shared/Documents/_ts/_os/New_Generation_CCMs/uc_hhpek_hdm_integration_manual_2016_apr_arm.pdf |
| v0.5 | https://web.archive.org/web/20230222180828/https://www.petekamutner.am/Shared/Documents/_ts/_os/New_Generation_CCMs/uc_hhpek_hdm_integration_manual_2017_v05_arm.pdf |
| v0.5.1 | https://web.archive.org/web/20220813210733/https://www.petekamutner.am/Shared/Documents/_ts/_os/New_Generation_CCMs/uc_hhpek_hdm_integration_manual_2018_v051_arm.pdf |
| v0.6 | https://web.archive.org/web/20190710200720/http://www.petekamutner.am:80/Shared/Documents/_ts/_os/New_Generation_CCMs/uc_hhpek_hdm_integration_manual_2019_v06_arm.pdf |
| v0.6.1 | https://web.archive.org/web/20220813210714/https://www.petekamutner.am/Shared/Documents/_ts/_os/New_Generation_CCMs/uc_hhpek_hdm_integration_manual_2020_v061_arm.pdf |
| v0.7 | https://web.archive.org/web/20230331135152/https://www.petekamutner.am/Shared/Documents/_ts/_os/New_Generation_CCMs/uc_hhpek_hdm_integration_manual_2022_v07_arm.pdf |
| v0.7.1 | https://web.archive.org/web/20231003162645/https://www.petekamutner.am/Shared/Documents/_ts/_os/New_Generation_CCMs/uc_hhpek_hdm_integration_manual_2022_v071_arm.pdf |
| v0.7.2 (2023) | https://web.archive.org/web/20240918063803/https://www.src.am/storage/menu_contents_144/uc_hhpek_hdm_integration_manual_2023_v072_arm_65082d55a8dbd.pdf |
| v0.7.2 (2025) | https://web.archive.org/web/20260510063325/https://www.src.am/storage/menu_contents_144/uc_hhpek_hdm_integration_manual_2025_v072_arm_67eba0542b1c7.pdf |
| v0.7.3 (2025) | https://web.archive.org/web/20260505213136/https://www.src.am/storage/menu_contents_144/uc_hhpek_hdm_integration_manual_2025_v073_arm_680f12e548bc8.pdf |
