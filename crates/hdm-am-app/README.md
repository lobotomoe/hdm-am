# hdm-am-app

Native Slint GUI for `hdm-am`. The app crate is intentionally separate from the protocol library
and CLI so desktop and mobile packaging can evolve without changing the core HDM client.

The UI includes Demo mode so store reviewers and first-time users can run every operation without an
HDM device. Demo mode never opens a network connection and never registers fiscal data.

## Desktop

```sh
cargo run -p hdm-am-app
```

This builds the `hdm-app` binary for macOS, Windows, or Linux.

### macOS package scaffold

Local `.app` bundle:

```sh
crates/hdm-am-app/scripts/generate-icons.bash
crates/hdm-am-app/scripts/macos-bundle.bash bundle
```

Mac App Store package, after installing Apple signing identities:

```sh
MACOS_CODESIGN_IDENTITY="3rd Party Mac Developer Application: Your Name (TEAMID)" \
MACOS_INSTALLER_IDENTITY="3rd Party Mac Developer Installer: Your Name (TEAMID)" \
crates/hdm-am-app/scripts/macos-bundle.bash package
```

The macOS bundle includes the privacy manifest and App Sandbox entitlements for outbound network
access. Store submission still needs Apple-side app metadata and a production signing setup.

### Windows package scaffold

On Windows with the Windows SDK installed:

```powershell
crates/hdm-am-app/scripts/windows-msix.ps1 layout
crates/hdm-am-app/scripts/windows-msix.ps1 pack
```

Signing requires a certificate visible to `signtool`:

```powershell
$env:WINDOWS_SIGN_CERT_THUMBPRINT = "..."
crates/hdm-am-app/scripts/windows-msix.ps1 sign
```

`crates/hdm-am-app/windows/Package.appxmanifest` is a Store-oriented MSIX manifest template. Partner Center may
replace the package identity after app association.

## Android

Android support is set up for Slint's Rust-only Android backend:

- `crates/hdm-am-app/src/lib.rs` exports `android_main`.
- `crates/hdm-am-app/Cargo.toml` builds the app as a `cdylib`.
- `crates/hdm-am-app/Cargo.toml` contains `cargo-apk` metadata, including the package id and network
  permissions required for TCP access to the HDM.

One-time toolchain setup:

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-apk
```

Install Android Studio's SDK, NDK, and platform tools, then expose them to build tools. Typical
environment variables are `ANDROID_HOME`, `ANDROID_NDK_ROOT`, and, for Skia fallback builds,
`ANDROID_NDK`.

Build and run with `cargo-apk`:

```sh
crates/hdm-am-app/scripts/android-apk.bash build
crates/hdm-am-app/scripts/android-apk.bash run
```

The script defaults to `aarch64-linux-android`, discovers the newest installed NDK under
`$ANDROID_HOME/ndk`, and sets the Android environment variables expected by Skia/cargo-apk. Override
the target with `ANDROID_TARGET=x86_64-linux-android`.

The `cargo-apk` path is kept for local APK smoke tests and CI.

Slint's current Android guide also supports `xbuild`:

```sh
cargo install --git https://github.com/rust-mobile/xbuild.git
cd app
x build --platform android --arch arm64 --format apk --release
```

The repository also includes a Play-oriented AAB wrapper:

```sh
crates/hdm-am-app/scripts/android-aab.bash build
```

The wrapper uses `cargo-apk`, `aapt2`, and `bundletool`. By default it builds a release APK first,
then converts it into a bundletool-validated AAB under `target/release/aab/`. Release builds need
cargo-apk signing configured through `CARGO_APK_RELEASE_KEYSTORE` and
`CARGO_APK_RELEASE_KEYSTORE_PASSWORD` or `[package.metadata.android.signing.release]`.

For Google Play upload, sign the final AAB with the upload key:

```sh
ANDROID_AAB_KEYSTORE=/secure/upload.jks \
ANDROID_AAB_KEYSTORE_PASSWORD=... \
ANDROID_AAB_KEY_ALIAS=upload \
crates/hdm-am-app/scripts/android-aab.bash build
```

CI runs the same conversion and `bundletool validate` against the debug APK as a packaging format
check; it is not a Play release artifact.

## iOS

iOS support is set up through XcodeGen plus a Cargo build script:

- `crates/hdm-am-app/ios/project.yml` describes an Xcode application target.
- `crates/hdm-am-app/ios/build_for_ios_with_cargo.bash` builds `hdm-app` for the selected iOS architecture and
  copies it into the app bundle executable path.
- `crates/hdm-am-app/src/lib.rs` selects Slint's Winit + Skia backend on iOS.

One-time toolchain setup:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
brew install xcodegen
```

Generate and open the Xcode project:

```sh
cd crates/hdm-am-app/ios
xcodegen generate
open HDM.xcodeproj
```

From Xcode, select an iOS simulator or device and build/run the `HDM` target.

CLI builds use the same generated project. Run these commands from the repository root:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
brew install xcodegen

# Compile-check a device app without signing.
crates/hdm-am-app/scripts/ios-build.bash unsigned

# Build a signed device app. Replace the team id with your Apple Developer team id.
IOS_TEAM_ID=T822AUM7XY crates/hdm-am-app/scripts/ios-build.bash build

# Create an .xcarchive.
IOS_TEAM_ID=T822AUM7XY crates/hdm-am-app/scripts/ios-build.bash archive

# Export an .ipa for local device testing.
IOS_TEAM_ID=T822AUM7XY IOS_EXPORT_METHOD=debugging crates/hdm-am-app/scripts/ios-build.bash export

# For TestFlight/App Store export, use:
IOS_TEAM_ID=T822AUM7XY IOS_EXPORT_METHOD=app-store-connect crates/hdm-am-app/scripts/ios-build.bash export
```

`xcodebuild -allowProvisioningUpdates` is enabled for signed commands, so Xcode can create or update
the app id and provisioning profile for `com.lobotomoe.hdmam` when the account has permission.
