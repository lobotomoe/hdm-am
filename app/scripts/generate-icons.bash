#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
app_dir="$(cd -- "$script_dir/.." && pwd)"
source_svg="$app_dir/assets/icon.svg"

if ! command -v magick >/dev/null 2>&1; then
    echo "ImageMagick 'magick' command is required to generate app icons." >&2
    exit 1
fi

make_png() {
    local size="$1"
    local output="$2"
    mkdir -p "$(dirname "$output")"
    magick -background none "$source_svg" \
        -resize "${size}x${size}" \
        -gravity center \
        -extent "${size}x${size}" \
        -alpha remove \
        -alpha off \
        -strip \
        "$output"
}

make_canvas_png() {
    local width="$1"
    local height="$2"
    local icon_size="$3"
    local output="$4"
    mkdir -p "$(dirname "$output")"
    magick -background "#14324A" "$source_svg" \
        -resize "${icon_size}x${icon_size}" \
        -gravity center \
        -extent "${width}x${height}" \
        -alpha remove \
        -alpha off \
        -strip \
        "$output"
}

make_png 48 "$app_dir/android/res/mipmap-mdpi/ic_launcher.png"
make_png 72 "$app_dir/android/res/mipmap-hdpi/ic_launcher.png"
make_png 96 "$app_dir/android/res/mipmap-xhdpi/ic_launcher.png"
make_png 144 "$app_dir/android/res/mipmap-xxhdpi/ic_launcher.png"
make_png 192 "$app_dir/android/res/mipmap-xxxhdpi/ic_launcher.png"

ios_icons="$app_dir/ios/Assets.xcassets/AppIcon.appiconset"
make_png 20 "$ios_icons/Icon-App-20x20@1x.png"
make_png 40 "$ios_icons/Icon-App-20x20@2x.png"
make_png 60 "$ios_icons/Icon-App-20x20@3x.png"
make_png 29 "$ios_icons/Icon-App-29x29@1x.png"
make_png 58 "$ios_icons/Icon-App-29x29@2x.png"
make_png 87 "$ios_icons/Icon-App-29x29@3x.png"
make_png 40 "$ios_icons/Icon-App-40x40@1x.png"
make_png 80 "$ios_icons/Icon-App-40x40@2x.png"
make_png 120 "$ios_icons/Icon-App-40x40@3x.png"
make_png 120 "$ios_icons/Icon-App-60x60@2x.png"
make_png 180 "$ios_icons/Icon-App-60x60@3x.png"
make_png 76 "$ios_icons/Icon-App-76x76@1x.png"
make_png 152 "$ios_icons/Icon-App-76x76@2x.png"
make_png 167 "$ios_icons/Icon-App-83.5x83.5@2x.png"
make_png 1024 "$ios_icons/Icon-App-1024x1024@1x.png"

macos_iconset="$app_dir/macos/AppIcon.iconset"
make_png 16 "$macos_iconset/icon_16x16.png"
make_png 32 "$macos_iconset/icon_16x16@2x.png"
make_png 32 "$macos_iconset/icon_32x32.png"
make_png 64 "$macos_iconset/icon_32x32@2x.png"
make_png 128 "$macos_iconset/icon_128x128.png"
make_png 256 "$macos_iconset/icon_128x128@2x.png"
make_png 256 "$macos_iconset/icon_256x256.png"
make_png 512 "$macos_iconset/icon_256x256@2x.png"
make_png 512 "$macos_iconset/icon_512x512.png"
make_png 1024 "$macos_iconset/icon_512x512@2x.png"

if command -v iconutil >/dev/null 2>&1; then
    iconutil -c icns "$macos_iconset" -o "$app_dir/macos/AppIcon.icns"
else
    echo "Skipping macOS .icns generation because iconutil is not available." >&2
fi

windows_assets="$app_dir/windows/Assets"
make_png 44 "$windows_assets/Square44x44Logo.png"
make_png 50 "$windows_assets/StoreLogo.png"
make_png 71 "$windows_assets/Square71x71Logo.png"
make_png 150 "$windows_assets/Square150x150Logo.png"
make_png 310 "$windows_assets/Square310x310Logo.png"
make_canvas_png 310 150 120 "$windows_assets/Wide310x150Logo.png"
make_canvas_png 620 300 160 "$windows_assets/SplashScreen.png"
