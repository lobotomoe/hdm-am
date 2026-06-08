#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
app_dir="$(cd -- "$script_dir/.." && pwd)"

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"

if [[ -z "${ANDROID_NDK:-}" ]]; then
    ndk_dir="$(find "$ANDROID_HOME/ndk" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort -V | tail -1)"
    if [[ -z "$ndk_dir" ]]; then
        echo "No Android NDK found under $ANDROID_HOME/ndk" >&2
        exit 1
    fi
    export ANDROID_NDK="$ndk_dir"
fi

export ANDROID_NDK_ROOT="${ANDROID_NDK_ROOT:-$ANDROID_NDK}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_NDK}"
export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/tools/bin:$PATH"

target="${ANDROID_TARGET:-aarch64-linux-android}"
action="${1:-build}"
shift || true

case "$action" in
    build)
        cargo apk build --manifest-path "$app_dir/Cargo.toml" --target "$target" --lib "$@"
        ;;
    run)
        cargo apk run --manifest-path "$app_dir/Cargo.toml" --target "$target" --lib "$@"
        ;;
    check)
        cargo check --manifest-path "$app_dir/Cargo.toml" --target "$target" "$@"
        ;;
    *)
        echo "Usage: $0 [build|run|check] [extra cargo args...]" >&2
        exit 2
        ;;
esac
