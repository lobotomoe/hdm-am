# hdm-am-app

Native Slint GUI for `hdm-am`. The app crate is intentionally separate from the protocol library
and CLI so desktop and mobile packaging can evolve without changing the core HDM client.

## Desktop

```sh
cargo run -p hdm-am-app
```

This builds the `hdm-app` binary for macOS, Windows, or Linux.

## Android

Android support is set up for Slint's Rust-only Android backend:

- `app/src/lib.rs` exports `android_main`.
- `app/Cargo.toml` builds the app as a `cdylib`.
- `app/Cargo.toml` contains `cargo-apk` metadata, including the package id and network
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
app/scripts/android-apk.bash build
app/scripts/android-apk.bash run
```

The script defaults to `aarch64-linux-android`, discovers the newest installed NDK under
`$ANDROID_HOME/ndk`, and sets the Android environment variables expected by Skia/cargo-apk. Override
the target with `ANDROID_TARGET=x86_64-linux-android`.

Slint's current Android guide also supports `xbuild`:

```sh
cargo install --git https://github.com/rust-mobile/xbuild.git
cd app
x build --platform android --arch arm64 --format apk --release
```

## iOS

iOS support is set up through XcodeGen plus a Cargo build script:

- `app/ios/project.yml` describes an Xcode application target.
- `app/ios/build_for_ios_with_cargo.bash` builds `hdm-app` for the selected iOS architecture and
  copies it into the app bundle executable path.
- `app/src/lib.rs` selects Slint's Winit + Skia backend on iOS.

One-time toolchain setup:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
brew install xcodegen
```

Generate and open the Xcode project:

```sh
cd app/ios
xcodegen generate
open HDM.xcodeproj
```

From Xcode, select an iOS simulator or device and build/run the `HDM` target.
