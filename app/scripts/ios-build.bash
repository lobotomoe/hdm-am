#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: app/scripts/ios-build.bash <command>

Commands:
  build       Build a device .app with automatic signing.
  unsigned   Build a device .app without signing, useful for local compile checks.
  simulator  Build a simulator .app without signing.
  archive    Create a signed Release .xcarchive.
  export     Create a signed Release .xcarchive and export an .ipa.
  export-only
             Export an .ipa from an existing .xcarchive.

Environment:
  IOS_TEAM_ID          Apple Developer team id for signed commands.
  IOS_CONFIGURATION    Debug or Release. Defaults to Debug for build, Release for archive/export.
  IOS_EXPORT_METHOD    Xcode export method. Defaults to debugging.
                     Use app-store-connect for TestFlight/App Store uploads.
  IOS_ARCHIVE_PATH     Defaults to app/ios/build/HDM.xcarchive.
  IOS_EXPORT_PATH      Defaults to app/ios/build/export.
  IOS_DERIVED_DATA     Defaults to app/ios/build/DerivedData.
EOF
}

command="${1:-build}"
if [[ "${command}" == "-h" || "${command}" == "--help" ]]; then
    usage
    exit 0
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
app_dir="$(cd -- "$script_dir/.." && pwd)"
ios_dir="$app_dir/ios"
project_path="$ios_dir/HDM.xcodeproj"
scheme="${IOS_SCHEME:-HDM}"
product_name="${IOS_PRODUCT_NAME:-HDM}"

case "$command" in
    build | unsigned | simulator)
        configuration="${IOS_CONFIGURATION:-Debug}"
        ;;
    archive | export | export-only)
        configuration="${IOS_CONFIGURATION:-Release}"
        ;;
    *)
        usage >&2
        exit 2
        ;;
esac

derived_data="${IOS_DERIVED_DATA:-$ios_dir/build/DerivedData}"
archive_path="${IOS_ARCHIVE_PATH:-$ios_dir/build/HDM.xcarchive}"
export_path="${IOS_EXPORT_PATH:-$ios_dir/build/export}"
export_options="${IOS_EXPORT_OPTIONS:-$ios_dir/build/ExportOptions.plist}"
export_method="${IOS_EXPORT_METHOD:-debugging}"
team_id="${IOS_TEAM_ID:-${DEVELOPMENT_TEAM:-}}"

cd "$ios_dir"
xcodegen generate
mkdir -p "$ios_dir/build"

common_args=(
    -project "$project_path"
    -scheme "$scheme"
    -configuration "$configuration"
    -derivedDataPath "$derived_data"
    -hideShellScriptEnvironment
)

signed_args=()
provisioning_flags=()
if [[ "$command" != "unsigned" && "$command" != "simulator" ]]; then
    if [[ -z "$team_id" ]]; then
        echo "IOS_TEAM_ID is required for signed iOS builds." >&2
        echo "Example: IOS_TEAM_ID=T822AUM7XY $0 $command" >&2
        exit 1
    fi

    signed_args=(
        DEVELOPMENT_TEAM="$team_id"
        CODE_SIGN_STYLE=Automatic
    )
    provisioning_flags=(
        -allowProvisioningUpdates
    )
fi

run_device_build() {
    xcodebuild \
        "${common_args[@]}" \
        -sdk iphoneos \
        "${provisioning_flags[@]}" \
        "${signed_args[@]}" \
        "$@" \
        build
}

run_archive() {
    xcodebuild \
        "${common_args[@]}" \
        -sdk iphoneos \
        -archivePath "$archive_path" \
        "${provisioning_flags[@]}" \
        "${signed_args[@]}" \
        archive
}

write_export_options() {
    cat > "$export_options" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>destination</key>
    <string>export</string>
    <key>method</key>
    <string>$export_method</string>
    <key>signingStyle</key>
    <string>automatic</string>
    <key>teamID</key>
    <string>$team_id</string>
</dict>
</plist>
EOF
}

run_export() {
    write_export_options
    xcodebuild \
        -exportArchive \
        -archivePath "$archive_path" \
        -exportPath "$export_path" \
        -exportOptionsPlist "$export_options" \
        "${provisioning_flags[@]}"
    echo "Export: $export_path"
    find "$export_path" -maxdepth 1 -name '*.ipa' -print
}

case "$command" in
    build)
        run_device_build
        echo "App: $derived_data/Build/Products/$configuration-iphoneos/$product_name.app"
        ;;
    unsigned)
        run_device_build CODE_SIGNING_ALLOWED=NO
        echo "App: $derived_data/Build/Products/$configuration-iphoneos/$product_name.app"
        ;;
    simulator)
        xcodebuild \
            "${common_args[@]}" \
            -sdk iphonesimulator \
            -destination "${IOS_SIMULATOR_DESTINATION:-generic/platform=iOS Simulator}" \
            CODE_SIGNING_ALLOWED=NO \
            build
        echo "App: $derived_data/Build/Products/$configuration-iphonesimulator/$product_name.app"
        ;;
    archive)
        run_archive
        echo "Archive: $archive_path"
        ;;
    export)
        run_archive
        run_export
        ;;
    export-only)
        run_export
        ;;
esac
