# HDM Privacy Policy

Last updated: 2026-06-08

HDM is a native client for Armenian fiscal cash registers over a local TCP connection.

## Data Handling

HDM does not use analytics, ads, tracking, crash reporting, or developer-operated servers.
The developer does not collect, receive, sell, or share personal data through the app.

The app uses the data entered by the user only on the device running the app:

- HDM host, port, password, cashier id, and cashier PIN.
- Receipt, return, report, cash, payment, eMark, header/footer, and logo operation inputs.
- User-selected or user-entered JSON and BMP file contents for operations that require structured
  payloads or a logo.

Operation data is sent only to the HDM address entered by the user. The app does not persist fiscal
responses, receipt payloads, or operation history.

## On-Device Storage

So the user does not have to re-enter connection details every session, the app stores the following
on the device only, never on any developer or third-party server:

- **Connection settings and saved connections** (host, port, timeout, operator id, default
  department, interface language, and the names of saved connections) are stored in a local file in
  the app's private data directory.
- **The HDM password and cashier PIN** are stored in the operating system's secure credential store
  (the iOS Keychain), encrypted at rest and marked device-only — they are excluded from iCloud
  Keychain, encrypted backups, and device-to-device transfer.

This data never leaves the device except as part of the HDM request the user initiates.

## Local Network

The app needs local network access to connect to fiscal devices over TCP. The HDM protocol encrypts
JSON payloads with the device protocol's password/session-key encryption. The app does not route HDM
traffic through developer infrastructure.

## Fiscal Device And Tax Authority

Some operations can print paper, change device configuration, register fiscal receipts, or cause the
HDM to communicate with Armenian tax authority infrastructure according to the HDM firmware and legal
workflow. Those device-side and tax-authority-side systems are outside the developer's control.

## Retention And Deletion

Deleting a saved connection in the app removes its stored settings and erases its password and PIN
from the Keychain. Uninstalling the app removes all of its on-device storage, including the settings
file and its Keychain credentials. Data already sent to an HDM is controlled by the HDM and any
fiscal systems it communicates with.

## Children

HDM is intended for business and developer use, not for children.

## Contact

Privacy questions can be sent to: selfsurfer@gmail.com
