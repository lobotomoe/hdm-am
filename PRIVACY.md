# HDM Tester Privacy Policy

Last updated: 2026-06-08

HDM Tester is a native utility for testing Armenian fiscal cash registers (HDM) over a local TCP
connection.

## Data Handling

HDM Tester does not use analytics, ads, tracking, crash reporting, or developer-operated servers.
The developer does not collect, receive, sell, or share personal data through the app.

The app uses the data entered by the user only on the device running the app:

- HDM host, port, password, cashier id, and cashier PIN.
- Receipt, return, report, cash, payment, eMark, header/footer, and logo operation inputs.
- User-selected or user-entered JSON and BMP file contents for operations that require structured
  payloads or a logo.

Operation data is sent only to the HDM address entered by the user. The app does not intentionally
persist HDM credentials, fiscal responses, receipt payloads, or operation history.

## Local Network

The app needs local network access to connect to fiscal devices over TCP. The HDM protocol encrypts
JSON payloads with the device protocol's password/session-key encryption. The app does not route HDM
traffic through developer infrastructure.

## Fiscal Device And Tax Authority

Some operations can print paper, change device configuration, register fiscal receipts, or cause the
HDM to communicate with Armenian tax authority infrastructure according to the HDM firmware and legal
workflow. Those device-side and tax-authority-side systems are outside the developer's control.

## Retention And Deletion

Because HDM Tester does not intentionally store app data, closing the app or clearing the input fields
removes the data from the app UI. Data already sent to an HDM is controlled by the HDM and any fiscal
systems it communicates with.

## Children

HDM Tester is intended for business and developer use, not for children.

## Contact

Privacy questions can be sent to: selfsurfer@gmail.com
