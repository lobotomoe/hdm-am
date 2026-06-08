#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: app/scripts/android-aab.bash [build] [extra cargo-apk args...]

Build a bundletool-validated Android App Bundle from the cargo-apk package.

Environment:
  ANDROID_HOME                   Defaults to ~/Library/Android/sdk.
  ANDROID_NDK                    Defaults to the newest NDK under ANDROID_HOME/ndk.
  ANDROID_TARGET                 Defaults to aarch64-linux-android.
  ANDROID_PROFILE                Defaults to release. Use debug for unsigned CI format checks.
  ANDROID_AAB_SOURCE_APK         Optional existing APK to convert instead of building one.
  ANDROID_AAB_OUTPUT             Defaults to target/<profile>/aab/hdm-am.aab.
  ANDROID_BUILD_TOOLS            Optional Android build-tools version, for example 35.0.0.
  BUNDLETOOL_BIN                 Optional bundletool executable. Defaults to bundletool on PATH.
  BUNDLETOOL_JAR                 Optional bundletool-all jar; used with java -jar.

Release signing:
  cargo-apk release builds need CARGO_APK_RELEASE_KEYSTORE and
  CARGO_APK_RELEASE_KEYSTORE_PASSWORD, or [package.metadata.android.signing.release].

  To sign the final AAB for Play upload, set:
    ANDROID_AAB_KEYSTORE
    ANDROID_AAB_KEYSTORE_PASSWORD
    ANDROID_AAB_KEY_ALIAS
    ANDROID_AAB_KEY_PASSWORD      Defaults to ANDROID_AAB_KEYSTORE_PASSWORD.
EOF
}

action="${1:-build}"
if [[ "$action" == "-h" || "$action" == "--help" ]]; then
    usage
    exit 0
fi
shift || true

if [[ "$action" != "build" ]]; then
    usage >&2
    exit 2
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
app_dir="$(cd -- "$script_dir/.." && pwd)"
repo_dir="$(cd -- "$app_dir/.." && pwd)"

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
if [[ -z "${ANDROID_NDK:-}" ]]; then
    ndk_dir="$(find "$ANDROID_HOME/ndk" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort | tail -1)"
    if [[ -z "$ndk_dir" ]]; then
        echo "No Android NDK found under $ANDROID_HOME/ndk" >&2
        exit 1
    fi
    export ANDROID_NDK="$ndk_dir"
fi

export ANDROID_NDK_ROOT="${ANDROID_NDK_ROOT:-$ANDROID_NDK}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_NDK}"
export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/tools/bin:$PATH"

run_bundletool() {
    if [[ -n "${BUNDLETOOL_JAR:-}" ]]; then
        java -jar "$BUNDLETOOL_JAR" "$@"
    elif [[ -n "${BUNDLETOOL_BIN:-}" ]]; then
        "$BUNDLETOOL_BIN" "$@"
    else
        bundletool "$@"
    fi
}

if ! command -v zip >/dev/null 2>&1 || ! command -v unzip >/dev/null 2>&1; then
    echo "zip and unzip are required to build an AAB." >&2
    exit 1
fi

if [[ -n "${BUNDLETOOL_JAR:-}" ]]; then
    if ! command -v java >/dev/null 2>&1; then
        echo "java is required when BUNDLETOOL_JAR is set." >&2
        exit 1
    fi
elif [[ -n "${BUNDLETOOL_BIN:-}" ]]; then
    if [[ ! -x "$BUNDLETOOL_BIN" ]]; then
        echo "BUNDLETOOL_BIN is not executable: $BUNDLETOOL_BIN" >&2
        exit 1
    fi
elif ! command -v bundletool >/dev/null 2>&1; then
    echo "bundletool is required to build a valid AAB." >&2
    echo "Install it with Homebrew or set BUNDLETOOL_JAR=/path/to/bundletool-all.jar." >&2
    exit 1
fi

if [[ -n "${ANDROID_BUILD_TOOLS:-}" ]]; then
    build_tools_dir="$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS"
else
    build_tools_dir="$(find "$ANDROID_HOME/build-tools" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort | tail -1)"
fi
aapt2="$build_tools_dir/aapt2"
if [[ ! -x "$aapt2" ]]; then
    echo "aapt2 not found under Android build-tools: $build_tools_dir" >&2
    exit 1
fi

target="${ANDROID_TARGET:-aarch64-linux-android}"
profile="${ANDROID_PROFILE:-release}"
case "$profile" in
    debug | dev)
        cargo_profile="debug"
        apk_profile_dir="debug"
        apk_args=()
        ;;
    release)
        cargo_profile="release"
        apk_profile_dir="release"
        apk_args=(--release)
        ;;
    *)
        echo "Unsupported ANDROID_PROFILE '$profile'. Use debug or release." >&2
        exit 2
        ;;
esac

apk_path="${ANDROID_AAB_SOURCE_APK:-}"
if [[ -z "$apk_path" ]]; then
    ANDROID_TARGET="$target" "$script_dir/android-apk.bash" build "${apk_args[@]}" "$@"
    apk_path="$repo_dir/target/$apk_profile_dir/apk/hdm-am.apk"
fi

if [[ ! -f "$apk_path" ]]; then
    echo "APK not found: $apk_path" >&2
    if [[ "$cargo_profile" == "release" ]]; then
        echo "For release, configure cargo-apk signing first." >&2
    fi
    exit 1
fi

aab_output="${ANDROID_AAB_OUTPUT:-$repo_dir/target/$apk_profile_dir/aab/hdm-am.aab}"
work_dir="$repo_dir/target/$apk_profile_dir/aab/work"
proto_apk="$work_dir/hdm-proto.apk"
module_dir="$work_dir/base"
module_zip="$work_dir/base.zip"

rm -rf "$work_dir"
mkdir -p "$module_dir" "$(dirname "$aab_output")"

"$aapt2" convert --output-format proto -o "$proto_apk" "$apk_path"

(
    cd "$module_dir"
    unzip -q "$proto_apk"
    rm -rf META-INF
    mkdir -p manifest
    mv AndroidManifest.xml manifest/AndroidManifest.xml
    zip -qr "$module_zip" .
)

run_bundletool build-bundle \
    --modules="$module_zip" \
    --output="$aab_output" \
    --overwrite

keystore="${ANDROID_AAB_KEYSTORE:-}"
keystore_password="${ANDROID_AAB_KEYSTORE_PASSWORD:-}"
key_alias="${ANDROID_AAB_KEY_ALIAS:-}"
key_password="${ANDROID_AAB_KEY_PASSWORD:-$keystore_password}"

if [[ -n "$keystore" || -n "$keystore_password" || -n "$key_alias" ]]; then
    if [[ -z "$keystore" || -z "$keystore_password" || -z "$key_alias" ]]; then
        echo "ANDROID_AAB_KEYSTORE, ANDROID_AAB_KEYSTORE_PASSWORD, and ANDROID_AAB_KEY_ALIAS must be set together." >&2
        exit 1
    fi
    if ! command -v jarsigner >/dev/null 2>&1; then
        echo "jarsigner is required to sign the AAB." >&2
        exit 1
    fi
    jarsigner \
        -sigalg SHA256withRSA \
        -digestalg SHA-256 \
        -keystore "$keystore" \
        -storepass "$keystore_password" \
        -keypass "$key_password" \
        "$aab_output" \
        "$key_alias"
fi

run_bundletool validate --bundle="$aab_output"

echo "AAB: $aab_output"
