# HDM Integration Protocol — English Translation

> **Unofficial translation.** This document is an English translation of the Armenian-language specification *"ՀՍԿԻՉ - ԴՐԱՄԱՐԿՂԱՅԻՆ ՄԵՔԵՆԱՅԻ ԻՆՏԵԳՐՈՒՄԸ ԱՐՏԱՔԻՆ (ԱՌԵՎՏՐԱՅԻՆ) ԾՐԱԳՐԵՐԻ ՀԵՏ"* ("Integration of the HDM with External (Commercial) Software"), published by the State Revenue Committee of Armenia (ՀՀՊԵԿ, *HHPEK*) on `src.am`. **Version 0.7.3** (April 2025), 34 pages. The original Armenian text is authoritative; this translation is provided for developer convenience.
>
> The source PDF is included in the version archive as [`history/hdm-protocol-v0.7.3-2025.pdf`](history/hdm-protocol-v0.7.3-2025.pdf).
>
> **Terminology notes used throughout this translation:**
> - **HDM** (Հսկիչ Դրամարկղային Մեքենա, "Control Cash Register Machine") — the fiscal cash register hardware.
> - **AS** (Արտաքին Ծրագիր, "External Software") — the application that integrates with the HDM. In this crate's context, that is the consumer of `hdm-am`.
> - **TIN** — Tax Identification Number (ՀՎՀՀ).
> - **ATG / ADG code** — Armenian tax classification code that products must carry. Lookup at `taxservice.am`.

## Table of Contents

1. [General notes](#1-general-notes)
2. [Definitions](#2-definitions)
3. [Activating integration mode](#3-activating-integration-mode)
   - 3.1 [Description of integration password fields](#31-description-of-integration-password-fields)
4. [Data exchange description](#4-data-exchange-description)
   - 4.1 [Operating modes](#41-operating-modes)
   - 4.2 [Data exchange sequence](#42-data-exchange-sequence)
   - 4.3 [Operations](#43-operations)
   - 4.4 [Request/response format](#44-requestresponse-format)
     - 4.4.1 [Request header byte layout](#441-request-header-byte-layout)
     - 4.4.2 [Response header byte layout](#442-response-header-byte-layout)
     - 4.4.3 [Request and response encryption](#443-request-and-response-encryption)
     - 4.4.4 [Request and response body](#444-request-and-response-body)
     - 4.4.5 [Request sequence number](#445-request-sequence-number)
   - 4.5 [Data exchange operations](#45-data-exchange-operations)
     - 4.5.1 [Get list of HDM operators and departments](#451-get-list-of-hdm-operators-and-departments)
     - 4.5.2 [HDM operator login](#452-hdm-operator-login)
     - 4.5.3 [HDM operator logout](#453-hdm-operator-logout)
     - 4.5.4 [HDM receipt print](#454-hdm-receipt-print)
     - 4.5.5 [Print copy of the last HDM receipt](#455-print-copy-of-the-last-hdm-receipt)
     - 4.5.6 [Get returnable receipt](#456-get-returnable-receipt)
     - 4.5.7 [Print return receipt](#457-print-return-receipt)
     - 4.5.8 [Cash drawer in/out](#458-cash-drawer-inout)
   - 4.6 [Get device date and time](#46-get-device-date-and-time)
     - 4.6.1 [Receipt sample](#461-receipt-sample)
     - 4.6.2 [HDM fiscal report](#462-hdm-fiscal-report)
     - 4.6.3 [HDM header and footer configuration](#463-hdm-header-and-footer-configuration)
     - 4.6.4 [HDM header logo configuration](#464-hdm-header-logo-configuration)
   - 4.7 [HDM time synchronization](#47-hdm-time-synchronization)
   - 4.8 [Get list of payment systems installed on the HDM](#48-get-list-of-payment-systems-installed-on-the-hdm)
   - 4.9 [Single eMark code request](#49-single-emark-code-request)
   - 4.10 [Response codes table](#410-response-codes-table)

---

## 1. General notes

The integration of the Control Cash Register Machine (HDM) with External Software (AS) implies the automation of operations performed using the HDM. The integration is intended to reduce the time spent on entering sold goods into the HDM through the AS, and to also offer the option of receiving the fiscal data of the cheque registered in the AS into the HDM. The integration also offers the option of printing the HDM receipt either through the device itself or through another printer connected to the AS.

Data exchange between the AS and HDM is performed via the network, using the TCP protocol. By default, the HDM is configured to operate over its own internal network — via WiFi or Ethernet (using a USB-to-Ethernet adapter) — via cable.

---

## 2. Definitions

| Acronym | Description |
|---|---|
| AS | External Software (Armenian: Արտաքին Ծրագիր) |
| HDM | Control Cash Register Machine (Armenian: Հսկիչ Դրամարկղային Մեքենա) |
| JSON | See <http://www.json.org/> |
| TCP | See <https://hy.wikipedia.org/wiki/TCP> |

---

## 3. Activating integration mode

The HDM device may operate as a standalone unit in standard mode, or in integration mode with an AS.

To activate integration mode, it is necessary to enter the HDM as either *Administrator* or *Setup-Administrator* and press the **Integration screen** button shown in *Image 1*.

**Image 1 — Activating integration mode.** (Original figure: a UI dialog labeled *"Արտաքին ծրագրերի հետ ինտեգրացիայի կարգաբերումներ"* / "External software integration settings") with the following fields:

- *Ակտիվացնել կապը արտաքին ծրագրի հետ* — "Activate connection with external software" (checkbox)
- *Տպել կտրոնը ՀԴՄ* — "Print receipt on HDM" (checkbox)
- *Ավտոմատ համակարգչի IP հասցեն* — "Automatic computer IP address"
- *ՀԴՄ գաղտնաբառ* — "HDM password" (with **Generate** button)
- *ՀԴՄ IP հասցե* — "HDM IP address"
- Buttons: *Չեղարկել* — "Cancel", *Պահպանել* — "Save"

### 3.1 Description of integration password fields

1. **Activate connection with external software** — when checked, the standard operating mode of the HDM device is changed to integration mode.
2. **Print receipt on HDM** — receipt data is taken from the AS, the HDM uses it to generate the cheque. If this field is *not* selected, then according to the data sent, the AS sends to the HDM the fiscal data, the HDM generates them and sends them, taking into account that the receipt is printed by means of the AS — without the HDM. If this field is selected, then the receipt is printed via the HDM as well.
3. **Automatic computer IP address** — the IP address of the AS's computer; the HDM will exchange data with it.
4. **HDM password** — the password through which the AS will be able to connect with the HDM. Press **Generate** to generate the password.
5. **HDM IP address** — the IP address of the HDM through which the AS will connect with the HDM.
6. **Port** — the HDM port number that the AS will use to connect to the HDM. The port number is automatically generated by the HDM.

---

## 4. Data exchange description

Armenian-character data items are encoded as UTF-8. The document is intended for editing in Armenian, Russian, and English. The use of other encodings may cause the device to malfunction.

### 4.1 Operating modes

The integration operating modes are presented in the table below:

| Code | Name | Description |
|---|---|---|
| **P** | Working with paper | The HDM generates and prints the fiscal receipt and provides it on its own. |
| **R** | Working without paper | The HDM generates and provides the fiscal receipt; the receipt's fiscal data are printed by the AS (the external system). |

### 4.2 Data exchange sequence

To make a correct request, it is necessary to perform the following steps:

1. Establish a physical TCP connection with the HDM via the parameters specified during configuration (`IP`, `Port`).
2. Send a 12-byte standard request header.
3. Send the operation-specific encrypted request.
4. Receive the standard response header.
5. Receive the response.
6. Receive other notifications or terminate the connection.
7. The maximum waiting time for a request response shall not exceed 50 seconds.

### 4.3 Operations

In integration mode, the data exchange operations possible with the HDM are as follows:

- Get list of HDM operators and departments.
- Operator login (sign in).
- Operator logout (sign out).
- Receipt print (requires an active session).
- Print copy of the last receipt (requires an active session).
- Get returnable receipt — read-only lookup (requires an active session).
- Receipt header and footer configuration (requires an active session).
- Receipt header Logo configuration (requires an active session).
- Print HDM reports (requires an active session).
- Print return receipt (requires an active session).
- Cash drawer in/out (requires an active session).
- Receipt sample (requires an active session).
- Receipt date and time (requires an active session).
- HDM device synchronization (requires an active session).
- Get list of payment systems installed on the HDM (requires an active session).
- eMark code request (requires an active session).

### 4.4 Request/response format

#### 4.4.1 Request header byte layout

| Byte | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | … |
|---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|---|
| Hex | `D5` | `80` | `D4` | `B4` | `D5` | `84` | `00` | `05` | `01` | `00` | `02` | `05` | … |
| Field | HDM text identifier | | | | | | Protocol version (here `05`) | | Operation code | Reserved | Encrypted request length (big-endian) | | 3DES-encrypted standard-format request |

The operation codes for bytes 9 (Operation code) are:

| Code | Operation |
|---:|---|
| 1 | Get list of HDM operators and departments |
| 2 | Operator login (sign in) |
| 3 | Operator logout (sign out) |
| 4 | Receipt print (requires an active session) |
| 5 | Print copy of the last receipt (requires an active session) |
| 6 | Print return receipt — full / by-amount / per-item (requires an active session) |
| 7 | Receipt header and footer configuration (requires an active session) |
| 8 | Receipt header Logo configuration (requires an active session) |
| 9 | Print HDM reports (requires an active session) |
| 10 | Get returnable receipt — read-only lookup (requires an active session) |
| 11 | Cash drawer in/out (requires an active session) |
| 12 | Receipt date and time (requires an active session) |
| 13 | Receipt sample (requires an active session) |
| 14 | HDM device synchronization (requires an active session) |
| 15 | Get list of payment systems installed on the HDM (requires an active session) |
| 16 | eMark code request (requires an active session) |

#### 4.4.2 Response header byte layout

| Byte | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | … |
|---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|---|
| Hex | `00` | `05` | `02` | `02` | `10` | `02` | `00` | `00` | `00` | `C8` | `00` | … |
| Field | Protocol version (`05`) | | Software version | | | | Response code (big-endian) | | Response length (big-endian) | | Reserved | 3DES-encrypted standard-format response |

> **Note on the byte order shown in the PDF:** the published table places the response code at offsets 7-8 and the payload length at offsets 9-10. The reserved bytes follow. This crate's `wire::ResponseHeader::read` reads the values in that order: protocol-version (2 bytes), software-version (3 bytes), response code (2 bytes BE), payload length (2 bytes BE), then 2 reserved bytes (skipped) — total of 11 bytes of header before the encrypted body.

#### 4.4.3 Request and response encryption

To encrypt requests, the **3DES** standard with **ECB mode** and **PKCS7 padding** is used. The encryption keys are two in number. The first key is derived from the SHA-256 hash of the HDM password, truncated to the **first 24 bytes**. The second key is generated by the HDM after the "Login" operation.

**Operations encrypted by the first key:**

- Get list of HDM operators and departments
- Operator login

**Operations encrypted by the second key:**

- Operator logout (sign out)
- Receipt print (requires an active session)
- Print copy of the last receipt (requires an active session)
- Get returnable receipt — read-only lookup (requires an active session)
- Print HDM reports (requires an active session)
- Print return receipt (requires an active session)
- Receipt header and footer configuration (requires an active session)
- Receipt header Logo configuration (requires an active session)
- Cash drawer in/out (requires an active session)
- Receipt date and time (requires an active session)
- Receipt sample (requires an active session)
- HDM device synchronization (requires an active session)
- Get list of payment systems installed on the HDM (requires an active session)
- eMark code request (requires an active session)

#### 4.4.4 Request and response body

Each request and response is represented as a JSON-formatted object. Some requests and responses can be empty. For textual values it is recommended to use UTF-8 encoding. Other UTF-8-compatible encodings can be used when sending Armenian, Russian, and English letters as well as control symbols. The use of other UTF-8-incompatible encodings may cause the device to malfunction.

#### 4.4.5 Request sequence number

For each successful client request, an outgoing sequence number is returned. It must be unique and conform to the following requirements:

1. The first request, as a sequence number, can use any number from the available range.
2. For each subsequent successful request, the sequence number must be greater than the previous successful request's sequence number.

This mechanism is necessary in order to exclude the possibility of *replay attacks* against the HDM device.

### 4.5 Data exchange operations

The request shapes and corresponding response shapes for all operations are presented below.

#### 4.5.1 Get list of HDM operators and departments

This operation makes it possible to obtain the list of all operators and departments that work on and are active for the HDM, as well as the departments attached to the operator.

**Encryption:** by password.

**Request:**

| Field | Type | Description |
|---|---|---|
| `password` | String | HDM password |

**Example:**

```json
{ "password": "1234ABCD" }
```

**Response:**

| Field | Type | Description |
|---|---|---|
| `list` | Object (optional) | Container for the response data |
| `c` | Array of objects | List of working operators. Each row is described in `list.c` |
| `>id` | Integer | Operator's unique ID, which is also the username |
| `>name` | String | Operator's name |
| `>deps` | Array of integers | Unique IDs of the departments assigned to the operator |
| `d` | Array of objects | List of departments. Each row is described in `list.d` |
| `>id` | Integer | Department's unique ID |
| `>name` | String | Department's name |
| `>type` | Integer | Department's taxation kind* |

\* Taxation kinds:

| ID | Taxation kind |
|---:|---|
| 1 | VAT-taxable |
| 2 | Not VAT-taxable |
| 3 | Turnover tax |
| 4 | Production licensee* |
| 5 | Patented* |
| 6 | Family business* |
| 7 | Micro-business |

(*Currently not in use.)

**Example:**

```json
{
  "c": [
    {
      "id": 2,
      "name": "Գայանե",
      "deps": [1, 2]
    }
  ],
  "d": [
    {
      "id": 1,
      "name": "Մրգեր",
      "type": 1
    }
  ]
}
```

#### 4.5.2 HDM operator login

**Encryption:** by password.

**Request:**

| Field | Type | Description |
|---|---|---|
| `password` | String | HDM password |
| `cashier` | Integer | Operator code |
| `pin` | String | Operator password |

**Example:**

```json
{
  "password": "1234ABCD",
  "cashier": 3,
  "pin": "3233"
}
```

**Response:**

| Field | Type | Description |
|---|---|---|
| `key` | String | The current session's 24-byte response key, encoded in Base64 |

**Example:**

```json
{ "key": "46AF00E.." }
```

#### 4.5.3 HDM operator logout

**Encryption:** by session key.

**Request:**

| Field | Type | Description |
|---|---|---|
| `seq` | Integer | Request sequence number |

**Example:**

```json
{ "seq": 32 }
```

#### 4.5.4 HDM receipt print

##### 4.5.4.1 Encryption: by session key

The amounts sent to the HDM, when adding the totals, must be rounded up to **2 decimal places**, and the quantities up to **3 decimal places**. It should be noted that the maximum quantity of one item is calculated as gram, milliliter or similar small fractional value: e.g. `101 grams of bread` is presented as `0.101 kilograms of bread`. Discounts of cheques sent to the HDM are calculated using rounding up to 2 decimal places: e.g. `0.005=0.01 ; 0.004=0`.

**Request:**

| Field | Type | Description |
|---|---|---|
| `seq` | Long | Request sequence number |
| `paidAmount` | Double | Cash paid amount |
| `paidAmountCard` | Double | Cashless paid amount |
| `partialAmount` | Double | Partial payment amount |
| `prePaymentAmount` | Double | Prepayment used amount |
| `mode` | Integer | Receipt print mode: `1` — simple cheque, `2` — products mode, `3` — prepayment (other items not accepted) |
| `partnerTin` | String | Partner's TIN (Tax Identification Number), 8 digits or `null` |
| `dep` | Integer | Department of a simple-mode cheque. Used only on a simple cheque print request |
| `useExtPOS` | Boolean | Use of another payment terminal. Used only on a cashless payment (if there is a payment terminal, the HDM transmits to it the program for cashless payment) |
| `PaymentSystem` | Integer | Payment system code, used in case `useExtPOS = false`. May also be `null` if the HDM device has only one payment system, in which case it is not necessary to specify which payment system to choose on the HDM device. Codes are listed in the [Payment systems list](#48-get-list-of-payment-systems-installed-on-the-hdm). |
| `rrn` | String | Transaction unique identifier (12 chars) |
| `terminalId` | String | Payment terminal unique number (8 chars) |
| `eMarks` | Array of strings | The eMark codes of marked goods in the receipt, in string-array form. Used only in product-mode cheques. The count of eMark codes sent to the HDM must be at least **29** (inclusive) and at most **110** (inclusive). The eMark codes must contain ASCII-table printable symbols only — from ASCII code 33 (inclusive) up to ASCII code 126 (inclusive) and ASCII code 29. Inside the code: any `"` character must be escaped as `\"`, any `\` character must be escaped as `\\`, and ASCII code 29 must be escaped as ``. |
| `items` | Array | Receipt's content (list of products). Each row is described in the following `item` description |
| `>dep` | Integer | Department of the product |
| `>qty` | Double | Quantity of the product. Allowed to have a precision of up to 3 decimal places after the period |
| `>discount` | Double | Discount on the product. Allowed to have a precision of up to 2 decimal places. **If no discount applied to the product, the field should not be set.** |
| `>discountType` | Integer | What kind of discount is set: %, price reduction, or total reduction on quantity: <br>• `%` discount in the case takes the value `1`, `(total_price * %)` <br>• Reduction on price (Դ - price): value `2`, `(initial_price – discount) * quantity` <br>• Reduction on total amount: value `4`, `(price * quantity) – discount` <br>If there is no corresponding discount, then it should not be set either. |
| `>additionalDiscount` | Double | Additional discount on the product. Allowed to have a precision of up to 2 decimal places. **If no discount applied to the product, the field should not be set.** |
| `>additionalDiscountType` | Integer | What kind of discount is set, on the total of monetary-priced items, or on % of items: in the case of percentage discount it takes the value `8`, in the case of monetary discount it takes the value `16`. If no corresponding discount, the field should not be set either. |
| `>price` | Double | Unit price of the product (precision: 2 decimal places) |
| `>productCode` | String | Product code (max 50 chars, must not be empty) |
| `>productName` | String | Product name (max 50 chars, must not be empty) |
| `>adgCode` | String | Product ATG code. The value should match the list which can be found at `taxservice.am`, by the path **ELECTRONIC SERVICES** → relevant **NEW SERIES HSKICH-DRAMARKGHAYIN MEKENA** documents containing the codes published in two Excel files Order N 1406-N and N 875-N, are also presented on ATG codes. |
| `>unit` | String | Unit-of-measure name (max 50 chars, must not be empty) |

**Code Block 1 — example (Simple receipt):**

```json
{
  "items": null,
  "eMarks": [
    "***********",
    "***********"
  ],
  "paidAmount": 2200,
  "paidAmountCard": 10,
  "partialAmount": 0,
  "prePaymentAmount": 0,
  "useExtPOS": false,
  "dep": 1,
  "mode": 1,
  "partnerTin": null,
  "seq": 1
}
```

**Code Block 2 — example (Products mode):**

```json
{
  "items": [
    {
      "adgCode": "0104",
      "dep": 1,
      "productCode": "001",
      "productName": "Pepsi",
      "qty": 3.0,
      "unit": "litr",
      "price": 1000.0,
      "additionalDiscount": 500.0,
      "additionalDiscountType": 16,
      "discount": 10.0,
      "discountType": 1
    }
  ],
  "rrn": "12345678",
  "terminalId": "12345678",
  "eMarks": [
    "***********",
    "***********"
  ],
  "paidAmount": 2200.0,
  "paidAmountCard": 10.0,
  "partialAmount": 0.0,
  "prePaymentAmount": 0.0,
  "useExtPOS": false,
  "mode": 2,
  "partnerTin": null,
  "seq": 4
}
```

**Code Block 3 — example (Prepayment):**

```json
{
  "seq": 1,
  "items": null,
  "paidAmount": 3000,
  "paidAmountCard": 0.00,
  "partialAmount": 0.0,
  "prePaymentAmount": 0.0,
  "mode": 3,
  "useExtPOS": false,
  "rrn": "12345678",
  "terminalId": "12345678",
  "partnerTin": null
}
```

**Response:**

| Field | Type | Description |
|---|---|---|
| `rseq` | Long | Receipt sequence number |
| `crn` | String | HDM registration number |
| `sn` | String | HDM serial number |
| `tin` | String | Taxpayer's TIN |
| `taxpayer` | String | Taxpayer's name |
| `address` | String | Taxpayer's address |
| `time` | Long | Receipt registration/print date and time, Greenwich time |
| `fiscal` | String | Fiscal number |
| `lottery` | String | Lottery number |
| `prize` | Integer | `0` — no prize\*, `1` — prize\*\* |
| `total` | Double (precision: 2 dp) | Total amount |
| `change` | Double (precision: 2 dp) | Change |
| `qr` | String | Required to be printed by the receipt's QR code text |
| `emarksCount` | String | Quantity of registered marks |
| `verificationNumber` | String | Receipt verification number: max 13 chars (consisting of digits) |

\* In this case, write *"Did not win"* on the receipt.

\*\* In this case, write *"You won money"* on the receipt.

\*\*\* The prize of receipts is no longer working.

**Code Block 4 — example response (Simple receipt):**

```json
{
  "rseq": 179,
  "crn": "31008940",
  "sn": "Q80414503833",
  "tin": "00000019",
  "taxpayer": "LUSARD",
  "address": "Yerevan, Smbat Kanyans 2 B31",
  "time": 1490190340000.0,
  "fiscal": "68287355",
  "lottery": "00000002",
  "prize": 0,
  "total": 3000.0,
  "change": 0.0,
  "emarksCount": 1,
  "verificationNumber": 128503,
  "qr": "TIN:00000019, CRN:31008940, SERIAL:, Receipt_ID: 365, Receipt_Time:02/03/2023 2:49:44 PM, FISCAL:92543463,TOTAL_CASH:0.0,TOTAL_NONCASH:0.0,PREP_USAGE:0.0,PARTIAL:0.0,TOTAL:0.0"
}
```

**Code Block 5 — example response (Products mode):**

```json
{
  "rseq": 166,
  "crn": "31008940",
  "sn": "Q80414503833",
  "tin": "00000019",
  "taxpayer": "LUSARD",
  "address": "Yerevan, Smbat Kanyans 2 B31",
  "time": 1490017838000.0,
  "fiscal": "92543463",
  "lottery": "00000002",
  "prize": 0,
  "total": 2200.0,
  "change": 0.00,
  "emarksCount": 1,
  "verificationNumber": 128503,
  "qr": "TIN:00000019, CRN:31008940, SERIAL:, Receipt_ID: 365, Receipt_Time:02/03/2023 2:49:44 PM, FISCAL:92543463,TOTAL_CASH:0.0,TOTAL_NONCASH:0.0,PREP_USAGE:0.0,PARTIAL:0.0,TOTAL:0.0"
}
```

**Code Block 6 — example response (Prepayment):**

```json
{
  "rseq": 175,
  "crn": "31008940",
  "sn": "Q80414503833",
  "tin": "00000019",
  "taxpayer": "LUSARD",
  "address": "Yerevan, Smbat Kanyans 2 B31",
  "time": 1490190340000.0,
  "fiscal": "68287355",
  "lottery": "00000002",
  "prize": 0,
  "total": 3000.0,
  "change": 0.0,
  "qr": "TIN:00000019, CRN:31008940, SERIAL:, Receipt_ID: 365, Receipt_Time:02/03/2023 2:49:44 PM, FISCAL:92543463,TOTAL_CASH:0.0,TOTAL_NONCASH:0.0,PREP_USAGE:0.0,PARTIAL:0.0,TOTAL:0.0"
}
```

#### 4.5.5 Print copy of the last HDM receipt

This operation makes it possible to obtain a copy of the last receipt via the Cashier and Administrator.

**Encryption:** by session key.

**Request:**

| Field | Type | Description |
|---|---|---|
| `seq` | Integer | Request sequence number |

**Example:**

```json
{ "seq": 32 }
```

#### 4.5.6 Get returnable receipt

> **Translator's note.** The original Armenian title is *"ՀԴՄ վերադարձվող կտրոնի ստացում"* — **get the returnable receipt**, not "print return receipt". The spec describes this in section §4.5.6, but its **wire operation code is 10** per the operation-codes table (§4.4.1) — the section number is not the operation code. This is a **read-only lookup**: given a receipt number it returns that receipt's full fiscal contents (items, amounts, eMarks, sale type) so the integrator can build the actual return via op 6 (§4.5.7). It registers nothing. (The receipt looked up is the *վերադարձվող* one — "the one to be returned", i.e. the original sale; op 6 by contrast prints the *վերադարձի* receipt — the new refund document.)

**Encryption:** by session key.

**Request:**

| Field | Type | Description |
|---|---|---|
| `seq` | Integer | Request sequence number |
| `receiptID` | String | Number of the receipt to look up |
| `crn` | String | HDM registration number of the device on which the looked-up receipt was printed |

**Example:**

```json
{
  "crn": "31005178",
  "receiptId": "1",
  "seq": 3
}
```

**Response:**

> **Spec caveat.** §4.5.6's response is internally inconsistent: the field table below and the Code Block 7 example disagree (the example adds a `type` field, omits `rseq`/`subType`/`refcrn`, and uses JSON numbers where the table says String). It is also **unverified against hardware** — an earlier integration test sent this lookup to op 6 and got vendor code 503 with an empty body, but op 6 is in fact the print-return code, so that result does not characterise this lookup (which uses op 10). The lookup also keys on a server-side `Receipt_ID` the firmware never exposes (op 4 omits the `qr` field entirely), so it may still be hard to satisfy in practice. The table below is reproduced faithfully from the original; treat field types as approximate.

| Field | Type | Description |
|---|---|---|
| `rseq` | Long | Receipt sequence number |
| `cid` | String | **Cashier** ID (`Գանձապահի ID`). *An earlier translation said "Customer ID" — that is wrong.* |
| `time` | String | Receipt registration/print date and time, Greenwich time |
| `ta` | String | Total amount |
| `cash` | String | Cash paid |
| `card` | String | Cashless (card) paid |
| `ppa` | String | Partial-payment amount |
| `ppu` | String | Used prepayment |
| `saleType` | String | Transaction type: `0` sale, `2` return, `3` prepayment |
| `subType` | Integer | Receipt type: `1` simple, `2` itemised |
| `did` | Integer | Department of a simple receipt |
| `pTin` | String | Buyer's TIN (8-digit) or null |
| `ref` | String | Number of the receipt being returned (set when this receipt is itself a return) |
| `refcrn` | String | Registration number of the HDM that printed the returned receipt |
| `eMarks` | Array | eMark codes of marked goods on the receipt |
| `totals` | Array | Line items (null for prepayment/simple receipts). Each row described below |
| `>gc` | String | Product code |
| `>gn` | String | Product name |
| `>qty` | String | Quantity |
| `>p` | String | Unit price |
| `>mu` | String | Unit of measure |
| `>rpid` | String | Item row ID (the handle for per-item partial returns in op 6) |
| `>dsc` | String | Discount |
| `>adsc` | String | Proportional secondary discount |
| `>dsct` | String | Discount type |
| `>did` | String | Department number |
| `>dt` | String | Department VAT |
| `>dtm` | String | Department tax regime |
| `>t` | String | Line total excluding VAT |
| `>tt` | String | Line total including VAT |

**Code Block 7 — example:**

```json
{
  "time": 1450260000,
  "type": 0,
  "ref": 0,
  "cid": 3,
  "ta": 3000,
  "cash": 1000,
  "card": 1000,
  "ppu": 0,
  "ppa": 1000,
  "saleType": 0,
  "pTin": "12345678",
  "eMarks": ["***********", "***********"],
  "totals": [
    {
      "gc": "001", "gn": "Product1", "qty": 1, "p": 1000, "mu": "տուփ",
      "rpid": 0, "dsc": null, "adsc": null, "dsct": null,
      "did": 1, "dt": 16.67, "dtm": 1, "t": 833.33, "tt": 1000.00
    }
  ]
}
```

#### 4.5.7 Print return receipt

> **Translator's note.** The original Armenian title is *"ՀԴՄ վերադարձի կտրոնի տպում"* — **print return receipt**, not "get receipt info". The spec describes this in section §4.5.7, but its **wire operation code is 6** per the operation-codes table (§4.4.1). This is the operation that actually **registers** a return (optionally partial: by amount, or by item via `returnItemList`). The read-only lookup of the receipt being returned is op 10 (§4.5.6).

**Encryption:** by session key.

**Request:**

| Field | Type | Description |
|---|---|---|
| `seq` | Integer | Request sequence number |
| `crn` | String | HDM registration number |
| `returnTicketId` | Integer | Receipt-to-return number |
| `cashAmountForReturn` | BigDecimal | Cash amount to be returned (set only if the partial-payment return case applies) |
| `cardAmountForReturn` | BigDecimal | Cashless (card) amount to be returned (set only if the partial-payment return case applies) |
| `prePaymentAmountForReturn` | BigDecimal | Prepayment used amount to be returned (set only if the partial-payment return case applies) |
| `rrn` | String | Transaction unique identifier (12 chars) |
| `terminalId` | String | Payment terminal unique number (8 chars) |
| `eMarks` | Array | The eMark codes of marked goods in the receipt, in string-array form. Used only in product-mode cheques. The count of eMark codes sent to the HDM must be at least **29** (inclusive) and at most **110** (inclusive). The eMark codes must contain ASCII-table printable symbols only — from ASCII code 33 (inclusive) up to ASCII code 126 (inclusive) and ASCII code 29: inside the code, any `"` character must be escaped as `\"`, any `\` character must be escaped as `\\`, and ASCII code 29 must be escaped as ``. |
| `returnItemList` | Array | List of return items. **Set only if the partial-payment return case applies for items in the receipt.** Each row is described in the following `returnItemList` description |
| `>rpid` | Long | Item's row sequence number |
| `>quantity` | Double | Quantity of this row's items |

**Code Block 8 — example:**

```json
{
  "seq": 2,
  "crn": "31008940",
  "returnTicketId": "205",
  "eMarks": [
    "***********",
    "***********"
  ],
  "cashAmountForReturn": 1000.0,
  "cardAmountForReturn": 0.0,
  "rrn": "12345678",
  "terminalId": "12345678",
  "prePaymentAmountForReturn": 0.0,
  "returnItemList": [
    {
      "rpid": 0,
      "quantity": "1.0"
    }
  ]
}
```

**Response:**

| Field | Type | Description |
|---|---|---|
| `rseq` | Long | Receipt sequence number |
| `crn` | String | HDM registration number |
| `sn` | String | HDM serial number |
| `tin` | String | Taxpayer's TIN |
| `taxpayer` | String | Taxpayer's name |
| `address` | String | Taxpayer's address |
| `time` | Long | Receipt registration/print date and time, Greenwich time |
| `rtime` | Long | Return receipt registration/print date and time, Greenwich time |
| `fiscal` | String | Fiscal number |
| `lottery` | String | Lottery number |
| `prize` | Integer\*\*\* | `0` — no prize\*, `1` — prize\*\* |
| `total` | Double (precision: 2 dp) | Total amount |
| `change` | Double (precision: 2 dp) | Change |
| `emarksCount` | String | Quantity of registered marks |
| `verificationNumber` | String | Receipt verification number: max 13 chars (consisting of digits) |

\* In this case, write *"Did not win"* on the receipt. <br>
\*\* In this case, write *"You won money"* on the receipt. <br>
\*\*\* The prize of receipts is no longer working.

#### 4.5.8 Cash drawer in/out

**Encryption:** by session key.

| Field | Type | Description |
|---|---|---|
| `seq` | Integer | Request sequence number |
| `amount` | Double | Print amount |
| `isCashIn` | Boolean | `true` — cash in; `false` — cash out |
| `cashierId` | Integer | Cashier number |
| `description` | String | Comment |

**Example:**

```json
{
  "seq": 1,
  "amount": 5000.0,
  "isCashIn": true,
  "description": "Cash in example"
}
```

### 4.6 Get device date and time

**Encryption:** by session key.

**Request:**

| Field | Type | Description |
|---|---|---|
| `seq` | Integer | Request sequence number |

**Example:**

```json
{ "seq": 2 }
```

**Response:**

| Field | Type | Description |
|---|---|---|
| `dt` | String | Date and time |

#### 4.6.1 Receipt sample

**Encryption:** by session key.

**Request:**

| Field | Type | Description |
|---|---|---|
| `seq` | Integer | Request sequence number |

**Example:**

```json
{ "seq": 2 }
```

#### 4.6.2 HDM fiscal report

**Encryption:** by session key.

**Request:**

| Field | Type | Description |
|---|---|---|
| `seq` | Integer | Request sequence number |
| `reportType` | Integer | Report kind selection\* |
| `deptId` | Integer | Department selection: if the report is generated without a department kind, the field should be left empty. Can be considered only one filter. |
| `cashierId` | Integer | Cashier selection: if the report is generated without a cashier kind, the field should be left empty. Can be considered only one filter. |
| `transactionTypeId` | Integer | Sale type — cash, cashless: if the report is generated without a particular selection, the field should be left empty. Can be considered Only filter. |
| `startDate` | Integer | Start of time range |
| `endDate` | Integer | End of time range |

\* `1` — X-report, `2` — Z-report.

**Examples:**

```json
{
  "seq": 55,
  "reportType": 1,
  "deptId": 18,
  "startDate": 123231324,
  "endDate": 123271324
}
```

```json
{
  "seq": 55,
  "reportType": 1,
  "transactionTypeId": 1,
  "startDate": 123231324,
  "endDate": 123271324
}
```

```json
{
  "seq": 55,
  "reportType": 1,
  "cashierId": 3,
  "startDate": 123231324,
  "endDate": 123271324
}
```

The report is printed via the HDM device; response information is not returned.

#### 4.6.3 HDM header and footer configuration

**Encryption:** by session key.

| Field | Type | Description |
|---|---|---|
| `headers` | Array | List of headers |
| `headers[0].align` | Integer | Alignment: values — `1` left, `2` centered, `3` right |
| `headers[0].bold` | Boolean | Bold |
| `headers[0].fsize` | Integer | Font size (1, 2, 3, 4, 5) |
| `headers[0].text` | String | Header's text |
| … | … | … |
| `footers` | Array | List of footers |
| `footers[0].align` | Integer | Alignment: values — `1` left, `2` centered, `3` right |
| `footers[0].bold` | Boolean | Bold |
| `footers[0].fsize` | Integer | Font size (1, 2, 3, 4, 5) |
| `footers[0].text` | String | Footer's text |
| … | … | … |

**Example:**

```json
{
  "headers": [
    {
      "align": 1,
      "bold": 1,
      "fsize": 2,
      "text": "Bari galust"
    }
  ],
  "footers": [
    {
      "align": 1,
      "bold": 1,
      "fsize": 2,
      "text": "..."
    }
  ]
}
```

#### 4.6.4 HDM header logo configuration

**Encryption:** by session key.

| Field | Type | Description |
|---|---|---|
| `seq` | Integer | Request sequence number |
| `headerLogo` | Base64String | The header Logo's image bytes encoded as a Base64String. The picture must be in **BMP** format and must not contain **more than 4 bits of colour** (≤16 colours). *(Translator's note: the original Armenian "չպարունակի 4 բիթից ավել գույներ" constrains colour depth, not pixel height — an earlier rendering as "4 pixels in height" was wrong.)* |

**Example:**

```json
{
  "seq": 435,
  "headerLogo": "IMAGE_ENCODED_AS_BASE64STRING"
}
```

### 4.7 HDM time synchronization

**Encryption:** by session key.

**Request:**

| Field | Type | Description |
|---|---|---|
| `seq` | Integer | Request sequence number |

**Example:**

```json
{ "seq": 82 }
```

### 4.8 Get list of payment systems installed on the HDM

(*Requires updated version.*)

**Encryption:** by session key.

**Request:**

| Field | Type | Description |
|---|---|---|
| `seq` | Integer | Request sequence number |

**Example:**

```json
{ "seq": 83 }
```

**Response:**

| Field | Type | Description |
|---|---|---|
| `PaymentSystems` | Array | Payment systems |
| `>code` | Integer | Code: at this time, the codes presented in the table below are included |
| `>name` | String | Name |

**Payment system codes:**

| Code | Name (Armenian) | Name (transliteration) |
|---:|---|---|
| 1 | Քարտային Վճարում | Card Payment |
| 10 | ԹԵԼ-ՍԵԼ | TEL-CELL |
| 11 | ԻՋԻ ՓԵՅ | EASYPAY |
| 12 | ՄՈԲԻ ԴՐԱՄ | MOBIDRAM |
| 13 | ԻԴՐԱՄ | IDRAM |
| 14 | ՍՏԱԿ ՊՐՈՑԵՍԻՆԳ | STAK PROCESSING |
| 15 | ՖԱՍՏ ՇԻՖԹ | FASTSHIFT |
| 16 | ԹԻՄ ՓԵՅ | TEAMPAY |
| 17 | ՊԵՅԻՔՍ | PAYIX |
| 18 | ԻՆԵԿՈ ՓԵՅ | INECO PAY |

**Example:**

```json
{
  "PaymentSystems": [
    {
      "code": 1,
      "name": "Card Payment"
    },
    {
      "code": 13,
      "name": "IDRAM"
    }
  ]
}
```

### 4.9 Single eMark code request

**Encryption:** by session key.

**Request:**

| Field | Type | Description |
|---|---|---|
| `seq` | Integer | Request sequence number |
| `eMark` | String | eMark code. The count of eMark codes sent to the HDM must be at least **29** (inclusive) and at most **110** (inclusive). The eMark codes must contain ASCII-table printable symbols only — from ASCII code 33 (inclusive) up to ASCII code 126 (inclusive) and ASCII code 29: inside the code, any `"` character must be escaped as `\"`, any `\` character must be escaped as `\\`, and ASCII code 29 must be escaped as ``. |

**Example (request JSON):**

```json
{
  "seq": 55,
  "eMark": "***********"
}
```

### 4.10 Response codes table

| Code | Name | Description | Stops the server connection |
|---:|---|---|:---:|
| **General** | | | |
| 200 | Operation completed successfully | | |
| 500 | Internal HDM error | Generic uncategorized error | X |
| 400 | Request error | Returned when the request is not processed | X |
| 402 | Bad protocol version | | X |
| 403 | Unauthorised connection | Returned when the registered IP address in the HDM does not match the IP address of the actually established connection's server | X |
| 404 | Bad operation code | Returned when the operation code specified in the header is wrong | X |
| 101 | Cryptographic encryption error | | X |
| 102 | Encryption error by session | | |
| 103 | Header format error | | X |
| 104 | Request sequence number error | | X |
| 105 | JSON format error | | X |
| 141 | Last receipt archive is empty | | |
| 142 | Last receipt belongs to a different user | | |
| 143 | Print error — general | | |
| 144 | Printer initialization error | | |
| 145 | Printer is out of paper | | |
| **Login operation errors** | | | |
| 111 | Operator password error | | X |
| 112 | No such operator | Possible in three cases: 1) the user's role is not Operator; 2) the user is not active; 3) the user is not registered as such | X |
| 113 | Operator is inactive | | X |
| 121 | Print error | | X |
| **Receipt print operation errors** | | | |
| 151 | No such department | This error is also returned in the case when the operator does not have the specified department assigned | |
| 152 | Paid amount is less than the total amount | | |
| 153 | Receipt amount exceeds the limit | | |
| 154 | Receipt amount must be a positive number | | |
| 155 | HDM synchronization required | | X |
| 156 | Synchronization not completed | | |
| 157 | Return receipt number error | | |
| 158 | Receipt already returned | | |
| 159 | Product price and quantity cannot be non-positive | | |
| 160 | Discount percent must be a non-negative number and less than 100 | | |
| 161 | Product code error | | |
| 162 | Product name error | | |
| 163 | Product unit-of-measure cannot be empty | | |
| 164 | Cashless payment failure | | |
| 165 | Product price cannot be 0 | | |
| 166 | Final price calculation error | | |
| 167 | Cashless amount is greater than the receipt's total amount | | |
| 168 | Cashless amount covers the total amount (cash amount is redundant) | | |
| 169 | Fiscal report filters selection error (more than one filter has been sent) | | |
| 170 | Fiscal report time-range selection error: range must not exceed 2 months | | |
| 171 | Item's price is an invalid value | | |
| 172 | Receipt is not a product, simple, or prepayment receipt | | |
| 173 | Invalid discount type | | |
| 174 | Receipt-to-return does not exist | | |
| 175 | Bad registration number for the receipt-to-return | | |
| 176 | Last receipt does not exist | | |
| 177 | Cannot perform return for the specified receipt type | | |
| 178 | Requested amount cannot be returned | | |
| 179 | Partial-payment receipt must be returned in full | | |
| 180 | Full-return amount more | | |
| 181 | Return product quantity error | | |
| 182 | Return receipt is of return-type itself | | |
| 183 | Bad ATG code | | |
| 184 | Inappropriate prepayment-return request | | |
| 185 | Could not return partial-payment receipt: HDM software synchronization required | | |
| 186 | Bad amount in prepayment case | | |
| 187 | Bad list in prepayment case | | |
| 188 | Bad amounts | | |
| 189 | Bad rounding | | |
| 190 | Payment is not available | | |
| 191 | On cash in/out, amount must be greater than 0 | | |
| 192 | ATG code is mandatory | | |
| 193 | Partner TIN format is wrong | | |
| 194 | In prepayment case, eMark codes are not allowed | | |
| 195 | Bad eMark code format | | |
| 196 | Code of another country (eMark code belongs to a foreign country) | | |

---

## Appendix — Receipt print examples (from spec §4.5.4)

### Prepayment receipt (cash)

```json
{
  "seq": 1,
  "paidAmount": 3000,
  "paidAmountCard": 0,
  "partialAmount": 0,
  "prePaymentAmount": 0,
  "mode": 3,
  "partnerTin": null,
  "useExtPOS": true,
  "items": []
}
```

### Prepayment receipt (card)

```json
{
  "seq": 1,
  "paidAmount": 0,
  "paidAmountCard": 3000,
  "partialAmount": 0,
  "prePaymentAmount": 0,
  "mode": 3,
  "partnerTin": null,
  "useExtPOS": true,
  "items": []
}
```

### Prepayment receipt (card + cash)

```json
{
  "seq": 1,
  "paidAmount": 3000,
  "paidAmountCard": 3000,
  "partialAmount": 0,
  "prePaymentAmount": 0,
  "mode": 3,
  "partnerTin": null,
  "useExtPOS": true,
  "items": []
}
```

### Products case

```json
{
  "seq": 1,
  "paidAmount": 4000,
  "paidAmountCard": 1000,
  "partialAmount": 0,
  "prePaymentAmount": 1000,
  "mode": 2,
  "partnerTin": null,
  "useExtPOS": true,
  "items": [
    {
      "dep": 1,
      "qty": 3,
      "price": 1000,
      "productCode": "001",
      "productName": "Coca cola",
      "adgCode": "VM01",
      "unit": "liter"
    },
    {
      "dep": 1,
      "qty": 3,
      "price": 1000,
      "productCode": "002",
      "productName": "Fanta",
      "adgCode": "VM02",
      "unit": "liter"
    }
  ]
}
```

### Simple case

```json
{
  "seq": 1,
  "paidAmount": 4000,
  "paidAmountCard": 1000,
  "partialAmount": 0,
  "prePaymentAmount": 1000,
  "mode": 2,
  "useExtPOS": true,
  "items": null,
  "dep": 1,
  "partnerTin": null
}
```

### Products with discounts

```json
{
  "seq": 1,
  "paidAmount": 52000,
  "paidAmountCard": 0,
  "partialAmount": 0,
  "prePaymentAmount": 0,
  "mode": 2,
  "useExtPOS": true,
  "items": [
    {
      "dep": 1,
      "qty": 1,
      "price": 65000,
      "productCode": "001",
      "discount": 60,
      "discountType": 1,
      "productName": "Coca cola",
      "adgCode": "VM01",
      "unit": "liter"
    },
    {
      "dep": 1,
      "qty": 1,
      "price": 65000,
      "productCode": "002",
      "discount": 60,
      "discountType": 1,
      "productName": "Fanta",
      "adgCode": "VM02",
      "unit": "liter"
    }
  ]
}
```

---

*End of translation — corresponds to the entire content of the source PDF at `history/hdm-protocol-v0.7.3-2025.pdf`.*
