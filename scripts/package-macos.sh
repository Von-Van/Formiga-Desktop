#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_dir="$(cd "$script_dir/.." && pwd)"
dist_dir="$repo_dir/dist"
app_dir="$dist_dir/Formiga.app"
version="${FORMIGA_VERSION:-0.51.6}"
version="${version#v}"
build_number="${FORMIGA_BUILD_NUMBER:-1}"
archive="$dist_dir/Formiga-$version-macOS-universal.zip"
disk_image="$dist_dir/Formiga-$version-macOS-universal.dmg"
icon_source="$repo_dir/packaging/shared/Formiga.icns"
dmg_staging="$dist_dir/Formiga-dmg"

cd "$repo_dir"
export FORMIGA_BUILD_VERSION="$version"
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release -p formiga-desktop --target aarch64-apple-darwin
cargo build --release -p formiga-desktop --target x86_64-apple-darwin

rm -rf "$app_dir" "$dmg_staging"
mkdir -p "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources"
cp "$repo_dir/packaging/macos/Info.plist" "$app_dir/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$app_dir/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $build_number" "$app_dir/Contents/Info.plist"
lipo -create \
    "$repo_dir/target/aarch64-apple-darwin/release/formiga" \
    "$repo_dir/target/x86_64-apple-darwin/release/formiga" \
    -output "$app_dir/Contents/MacOS/Formiga"
chmod 755 "$app_dir/Contents/MacOS/Formiga"

if [[ -f "$icon_source" ]]; then
    cp "$icon_source" "$app_dir/Contents/Resources/Formiga.icns"
fi

if [[ -n "${FORMIGA_CODESIGN_IDENTITY:-}" ]]; then
    codesign --force --deep --options runtime --timestamp \
        --sign "$FORMIGA_CODESIGN_IDENTITY" "$app_dir"
else
    codesign --force --deep --sign - "$app_dir"
fi

ditto -c -k --sequesterRsrc --keepParent "$app_dir" "$archive"
mkdir -p "$dmg_staging"
ditto "$app_dir" "$dmg_staging/Formiga.app"
ln -s /Applications "$dmg_staging/Applications"
cp "$repo_dir/packaging/macos/README.txt" "$dmg_staging/Read Me.txt"
hdiutil create -volname "Formiga" -srcfolder "$dmg_staging" -ov -format UDZO "$disk_image"

if [[ -n "${FORMIGA_NOTARY_PROFILE:-}" ]]; then
    xcrun notarytool submit "$disk_image" --keychain-profile "$FORMIGA_NOTARY_PROFILE" --wait
    xcrun stapler staple "$disk_image"
fi

shasum -a 256 "$archive" > "$archive.sha256"
shasum -a 256 "$disk_image" > "$disk_image.sha256"

echo "Packaged $disk_image and $archive"
