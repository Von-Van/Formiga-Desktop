#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_dir="$(cd "$script_dir/.." && pwd)"
dist_dir="$repo_dir/dist"
app_dir="$dist_dir/Formiga.app"

cd "$repo_dir"
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release -p formiga-desktop --target aarch64-apple-darwin
cargo build --release -p formiga-desktop --target x86_64-apple-darwin

mkdir -p "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources"
cp "$repo_dir/packaging/macos/Info.plist" "$app_dir/Contents/Info.plist"
lipo -create \
    "$repo_dir/target/aarch64-apple-darwin/release/formiga" \
    "$repo_dir/target/x86_64-apple-darwin/release/formiga" \
    -output "$app_dir/Contents/MacOS/Formiga"
chmod 755 "$app_dir/Contents/MacOS/Formiga"

if [[ -n "${FORMIGA_CODESIGN_IDENTITY:-}" ]]; then
    codesign --force --deep --options runtime --timestamp \
        --sign "$FORMIGA_CODESIGN_IDENTITY" "$app_dir"
else
    codesign --force --deep --sign - "$app_dir"
fi

ditto -c -k --sequesterRsrc --keepParent "$app_dir" "$dist_dir/Formiga-0.1.0-universal-macos.zip"
echo "Packaged $dist_dir/Formiga-0.1.0-universal-macos.zip"
