#!/usr/bin/env bash
set -euo pipefail

binary_name="${1:-hdm-app}"
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
app_dir="$(cd -- "$script_dir/.." && pwd)"

# Xcode's script environment may not include Cargo or Homebrew paths.
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH:$HOME/.cargo/bin"

if [[ "${CONFIGURATION:-Debug}" == "Debug" ]]; then
    cargo_profile="debug"
else
    cargo_profile="release"
fi

export CARGO_TARGET_DIR="${DERIVED_FILE_DIR:?}/cargo"

is_simulator=0
if [[ "${LLVM_TARGET_TRIPLE_SUFFIX:-}" == "-simulator" || "${SDK_NAME:-}" == *simulator* ]]; then
    is_simulator=1
fi

executables=()
for arch in ${ARCHS:?}; do
    case "$arch" in
        arm64)
            if [[ "$is_simulator" -eq 1 ]]; then
                cargo_target="aarch64-apple-ios-sim"
            else
                cargo_target="aarch64-apple-ios"
            fi
            ;;
        x86_64)
            cargo_target="x86_64-apple-ios"
            export CFLAGS_x86_64_apple_ios="-target x86_64-apple-ios"
            ;;
        *)
            echo "Unsupported iOS architecture: $arch" >&2
            exit 1
            ;;
    esac

    if [[ "$cargo_profile" == "release" ]]; then
        cargo build \
            --release \
            --manifest-path "$app_dir/Cargo.toml" \
            --target "$cargo_target" \
            --bin "$binary_name"
    else
        cargo build \
            --manifest-path "$app_dir/Cargo.toml" \
            --target "$cargo_target" \
            --bin "$binary_name"
    fi

    executables+=("$CARGO_TARGET_DIR/$cargo_target/$cargo_profile/$binary_name")
done

mkdir -p "$TARGET_BUILD_DIR/$(dirname "$EXECUTABLE_PATH")"
lipo -create -output "$TARGET_BUILD_DIR/$EXECUTABLE_PATH" "${executables[@]}"

if [[ "$is_simulator" -eq 0 && "${CODE_SIGNING_ALLOWED:-YES}" != "NO" ]]; then
    codesign \
        --force \
        --sign "${EXPANDED_CODE_SIGN_IDENTITY:?}" \
        --entitlements "${TARGET_TEMP_DIR:?}/${PRODUCT_NAME:?}.app.xcent" \
        "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"
fi
