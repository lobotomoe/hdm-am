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
