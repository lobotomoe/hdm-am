#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: app/scripts/macos-bundle.bash <command>

Commands:
  bundle   Build a release macOS .app bundle without signing.
  sign     Build and sign the .app bundle.
  package  Build, sign, and create a Mac App Store .pkg.

Environment:
  MACOS_APP_NAME              Defaults to "HDM".
  MACOS_BUNDLE_ID             Defaults to com.lobotomoe.hdmam.
  MACOS_BUNDLE_VERSION        Defaults to 1.
  MACOS_SHORT_VERSION         Defaults to 0.1.0.
  MACOS_APP_PATH              Defaults to app/macos/build/HDM.app.
  MACOS_CODESIGN_IDENTITY     Required for sign/package.
  MACOS_INSTALLER_IDENTITY    Required for package.
  MACOS_PKG_PATH              Defaults to app/macos/build/HDMTester.pkg.

For Mac App Store signing, use the "3rd Party Mac Developer Application" and
"3rd Party Mac Developer Installer" identities from the Apple Developer account.
EOF
}

command="${1:-bundle}"
if [[ "$command" == "-h" || "$command" == "--help" ]]; then
    usage
    exit 0
fi

case "$command" in
    bundle | sign | package)
        ;;
    *)
        usage >&2
        exit 2
        ;;
esac

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
app_dir="$(cd -- "$script_dir/.." && pwd)"
repo_dir="$(cd -- "$app_dir/../.." && pwd)"
macos_dir="$app_dir/macos"

app_name="${MACOS_APP_NAME:-HDM}"
bundle_id="${MACOS_BUNDLE_ID:-com.lobotomoe.hdmam}"
bundle_version="${MACOS_BUNDLE_VERSION:-1}"
short_version="${MACOS_SHORT_VERSION:-0.1.0}"
bundle_path="${MACOS_APP_PATH:-$macos_dir/build/$app_name.app}"
pkg_path="${MACOS_PKG_PATH:-$macos_dir/build/HDMTester.pkg}"
codesign_identity="${MACOS_CODESIGN_IDENTITY:-}"
installer_identity="${MACOS_INSTALLER_IDENTITY:-}"

contents_dir="$bundle_path/Contents"
macos_contents_dir="$contents_dir/MacOS"
resources_dir="$contents_dir/Resources"

cargo build -p hdm-am-app --release --bin hdm-app

rm -rf "$bundle_path"
mkdir -p "$macos_contents_dir" "$resources_dir"

cp "$repo_dir/target/release/hdm-app" "$macos_contents_dir/$app_name"
chmod 755 "$macos_contents_dir/$app_name"

cp "$macos_dir/Info.plist" "$contents_dir/Info.plist"
if [[ -x /usr/libexec/PlistBuddy ]]; then
    /usr/libexec/PlistBuddy -c "Set :CFBundleExecutable $app_name" "$contents_dir/Info.plist"
    /usr/libexec/PlistBuddy -c "Set :CFBundleDisplayName $app_name" "$contents_dir/Info.plist"
    /usr/libexec/PlistBuddy -c "Set :CFBundleName $app_name" "$contents_dir/Info.plist"
    /usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $bundle_id" "$contents_dir/Info.plist"
    /usr/libexec/PlistBuddy -c "Set :CFBundleVersion $bundle_version" "$contents_dir/Info.plist"
    /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $short_version" "$contents_dir/Info.plist"
fi

if [[ -f "$macos_dir/AppIcon.icns" ]]; then
    cp "$macos_dir/AppIcon.icns" "$resources_dir/AppIcon.icns"
fi
cp "$app_dir/ios/PrivacyInfo.xcprivacy" "$resources_dir/PrivacyInfo.xcprivacy"

if command -v plutil >/dev/null 2>&1; then
    plutil -lint "$contents_dir/Info.plist" "$resources_dir/PrivacyInfo.xcprivacy" >/dev/null
fi

echo "App: $bundle_path"

if [[ "$command" == "bundle" ]]; then
    exit 0
fi

if [[ -z "$codesign_identity" ]]; then
    echo "MACOS_CODESIGN_IDENTITY is required for $command." >&2
    exit 1
fi

codesign \
    --force \
    --timestamp \
    --options runtime \
    --entitlements "$macos_dir/HDMTester.entitlements" \
    --sign "$codesign_identity" \
    "$bundle_path"

codesign --verify --deep --strict --verbose=2 "$bundle_path"

if [[ "$command" == "sign" ]]; then
    exit 0
fi

if [[ -z "$installer_identity" ]]; then
    echo "MACOS_INSTALLER_IDENTITY is required for package." >&2
    exit 1
fi

mkdir -p "$(dirname "$pkg_path")"
productbuild \
    --component "$bundle_path" \
    /Applications \
    --sign "$installer_identity" \
    "$pkg_path"

echo "Package: $pkg_path"
