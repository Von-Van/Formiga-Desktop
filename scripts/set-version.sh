#!/usr/bin/env bash
# Writes a new release version into every place docs/BUILD.md lists for it, so a release commit
# never misses one, and refreshes Cargo.lock for the workspace's own crates.
#
# The words a person writes for a release stay theirs: this script never writes the CHANGELOG
# entry, the README's "New in" text or docs/RELEASE_NOTES.md, and only reminds you which of them
# still needs writing.
#
# Usage:
#   scripts/set-version.sh <version>   e.g. scripts/set-version.sh 0.67.2
#   scripts/set-version.sh --check     says whether every place agrees with Cargo.toml
#
# Tools used: cargo, perl, grep and sed.

set -euo pipefail

repo_dir="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_dir"

# Every file that names the current version, besides Cargo.toml and Cargo.lock.
files=(
  README.md
  docs/GENERATION.md
  scripts/package-macos.sh
  scripts/package-windows.ps1
  packaging/itch/page.md
  packaging/windows/winget/README.md
  scripts/winget-manifest.sh
)

current="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
if [ -z "$current" ]; then
  echo "Cargo.toml names no workspace version" >&2
  exit 1
fi

if [ "${1:-}" = "--check" ]; then
  missing=0
  for file in "${files[@]}"; do
    if ! grep -qF "$current" "$file"; then
      echo "$file does not name $current" >&2
      missing=1
    fi
  done
  if awk '/^name = "formiga-/ { getline; print }' Cargo.lock | grep -vqF "version = \"$current\""; then
    echo "Cargo.lock has a workspace crate at another version than $current" >&2
    missing=1
  fi
  [ "$missing" -eq 0 ] && echo "Every place names $current."
  exit "$missing"
fi

new="${1:-}"
new="${new#v}"
if ! [[ "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: $(basename "$0") <version>   e.g. $(basename "$0") 0.67.2" >&2
  echo "       $(basename "$0") --check" >&2
  exit 2
fi
# Running it again with the same version is how an interrupted run is finished: the files are left
# as they are and Cargo.lock is brought up to them.

# The dots are literal: 0.67.1 must not also match 0x67y1.
pattern="$(printf '%s' "$current" | sed 's/\./\\./g')"

perl -pi -e "s/^version = \"$pattern\"\$/version = \"$new\"/" Cargo.toml
for file in "${files[@]}"; do
  perl -pi -e "s/(?<![0-9.])$pattern(?![0-9])/$new/g" "$file"
done
cargo update --workspace --quiet

# Anything still naming the old version was missed, or is prose about an earlier release.
if [ "$new" != "$current" ] && grep -nF "$current" Cargo.toml "${files[@]}"; then
  echo "The lines above still name $current; check whether each should." >&2
fi

echo "The version is now $new. Still to write by hand:"
grep -qF "## [$new]" CHANGELOG.md || echo "  - the CHANGELOG entry, ## [$new] - $(date +%F)"
echo "  - the README's \"New in $new\" text, and the same at the top of docs/RELEASE_NOTES.md"
