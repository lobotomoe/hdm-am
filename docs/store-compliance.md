# Store Compliance Notes

Last reviewed: 2026-06-08.

HDM is a technical utility for testing Armenian fiscal cash registers over a user-entered local
TCP endpoint. Store metadata and review notes must say this clearly: the app is useful with a real HDM
device, and it also includes Demo mode so reviewers can verify the UI without hardware.

## Sources Checked

- Apple App Review Guidelines: https://developer.apple.com/app-store/review/guidelines/
- Apple privacy manifests: https://developer.apple.com/documentation/bundleresources/privacy-manifest-files
- Google Play functionality policy: https://support.google.com/googleplay/android-developer/answer/9898783
- Google Play user data policy: https://support.google.com/googleplay/android-developer/answer/10144311
- Google Play target API level: https://support.google.com/googleplay/android-developer/answer/11926878
- Google Play Android App Bundle requirement: https://developer.android.com/guide/app-bundle
- Microsoft Store policies: https://learn.microsoft.com/en-us/windows/apps/publish/store-policies
- Microsoft app capabilities: https://learn.microsoft.com/en-us/windows/uwp/packaging/app-capability-declarations

## Current App-Side Compliance

- Demo mode exists and performs every operation without network access or fiscal side effects.
- Privacy information is available in-app through the Privacy action.
- Root privacy policy exists at `PRIVACY.md`.
- The app does not use analytics, ads, tracking, crash reporting, push notifications, accounts, or
  developer-operated servers.
- The app asks only for network permissions needed to connect to a user-entered HDM endpoint.
- iOS bundles `PrivacyInfo.xcprivacy`.
- iOS declares local network usage text.
- Android targets API 35.
- Destructive HDM operations require an explicit confirmation checkbox outside Demo mode.

## Store Metadata Baseline

Suggested short description:

> HDM checks and exercises Armenian fiscal cash registers over a local TCP connection.

Suggested full description:

> HDM is a native utility for developers, integrators, and support teams working with Armenian
> fiscal cash registers. It can probe an HDM endpoint, verify operator credentials, list operators and
> departments, print or return receipts, run reports, test cash operations, configure headers and
> logos, sync device time, list payment systems, and submit eMarks. Demo mode lets reviewers and new
> users inspect the workflow without connecting to fiscal hardware.

Required review note:

> The app is intended for Armenian HDM fiscal devices reachable on the user's local network. To test
> without hardware, enable Demo mode in the connection panel; every operation then returns a synthetic
> response and no network request or fiscal registration is performed.

## Apple iOS / TestFlight

Status: buildable locally and suitable for TestFlight after App Store Connect metadata is completed.

Still required in App Store Connect:

- Create app record for bundle id `com.lobotomoe.hdmam`.
- Upload with export method `app-store-connect`.
- Complete App Privacy answers consistently with `PRIVACY.md`.
- Complete export compliance. The app uses HDM protocol encryption and should not be described as
  having no cryptography.
- Add screenshots showing Demo mode, Probe, Operators, Receipt, and error handling.
- Add reviewer notes from this document.
- Use a monotonically increasing `CFBundleVersion` for every upload.

## Apple macOS / Mac App Store

Status: package scaffold exists, but final store signing/submission is not proven yet.

Current positives:

- `crates/hdm-am-app/scripts/macos-bundle.bash` creates a macOS `.app` bundle.
- The bundle includes `PrivacyInfo.xcprivacy`.
- `crates/hdm-am-app/macos/HDMTester.entitlements` enables App Sandbox and outbound network client access.

Known blockers:

- Need a real Mac App Store signing/export run with Apple certificates and provisioning.
- Current typed JSON/BMP file paths are not Mac App Store sandbox-friendly. Mac distribution should
  use native file pickers or another sandbox-safe import path before relying on those operations.
- Need final screenshots and App Store Connect metadata for the macOS platform.

## Google Play

Status: app-side behavior is close and AAB format creation is locally verified; signed Play release
upload is not proven yet.

Current positives:

- `target_sdk_version = 35`, matching the current Android 15 submission requirement.
- No sensitive Android permissions beyond network state and internet.
- Demo mode prevents broken-functionality rejection when reviewers lack HDM hardware.
- Privacy policy and in-app privacy text exist.
- `crates/hdm-am-app/scripts/android-aab.bash` builds a bundletool-validated `.aab` from the cargo-apk package.
- Local AAB format validation was verified on 2026-06-08 with `bundletool validate`.
- CI validates the AAB packaging path against the debug APK.

Known blockers:

- Need Play signing and release signing setup.
- Need a successful signed release `.aab` upload to Play Console.
- Need Play Console Data safety answers consistent with `PRIVACY.md`.
- Need content rating, app access instructions, screenshots, and closed testing setup.

Suggested Data safety posture:

- Data collected by developer: No.
- Data shared by developer: No.
- Data encrypted in transit to developer services: Not applicable because there are no developer
  services.
- App can function without an account: Yes.

## Microsoft Store / Windows

Status: package scaffold exists, but final Store packaging/submission is not proven yet.

Current positives:

- `crates/hdm-am-app/windows/Package.appxmanifest` defines a Store-oriented MSIX package template.
- `crates/hdm-am-app/scripts/windows-msix.ps1` builds a layout and can pack/sign MSIX when Windows SDK tools are
  available.
- The manifest declares only network capabilities plus `runFullTrust`, which is required for a
  packaged Win32 desktop app.

Known blockers:

- Need a successful signed MSIX/App Installer validation on Windows.
- Need Partner Center package identity association, icons, Store listing assets, and metadata.
- Need privacy policy URL in Partner Center. Microsoft policy requires privacy policies for Win32
  products and for products accessing personal information.

## Cross-Store Release Checklist

- Build and launch on each target OS.
- Run Demo mode for every operation.
- Run at least Probe, Operators, Verify login, Receipt, and Report against a real HDM where available.
- Verify no placeholder buttons or non-functional controls.
- Verify all store screenshots match the actual UI.
- Verify store listing says the app is for Armenian HDM fiscal devices and local network testing.
- Verify privacy policy, App Privacy, Google Data safety, and Microsoft privacy metadata all say the
  same thing.
- Verify build number/version is unique for every uploaded build.
