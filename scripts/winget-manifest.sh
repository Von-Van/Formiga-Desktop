#!/usr/bin/env bash
# Fills in the winget manifest templates under packaging/windows/winget/ for one already-
# released version and writes them to packaging/windows/winget/out/<version>/, ready to copy
# into a microsoft/winget-pkgs clone at manifests/v/VonVan/Formiga/<version>/.
#
# This only reads metadata GitHub already published for the release; it never builds, signs, or
# uploads anything, and it never touches microsoft/winget-pkgs itself.
#
# Usage:
#   scripts/winget-manifest.sh <version>
#     <version> is the release version, with or without a leading "v" (e.g. 0.58.7 or v0.58.7).
#
# Tools used: curl, gh (its own --jq filter, not a system jq install), shasum, sed - plus
# ordinary shell builtins (cd, mkdir, mktemp, read, trap) that any bash script needs.
#
# See packaging/windows/winget/README.md for what to do with the generated files.

set -euo pipefail

REPO="Von-Van/Formiga-Desktop"

if [[ $# -ne 1 || "$1" == "-h" || "$1" == "--help" ]]; then
  echo "usage: $(basename "$0") <version>   e.g. $(basename "$0") 0.58.7" >&2
  exit 1
fi

VERSION="${1#v}"
TAG="v${VERSION}"
ASSET="Formiga-${VERSION}-windows-x64.msi"
CHECKSUM_ASSET="${ASSET}.sha256"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TEMPLATE_DIR="${REPO_ROOT}/packaging/windows/winget"
OUT_DIR="${TEMPLATE_DIR}/out/${VERSION}"

for tool in curl gh shasum sed; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "error: '$tool' is required on PATH but was not found" >&2
    exit 1
  fi
done

# --- Confirm the release and its assets exist -------------------------------------------------

echo "Looking up ${TAG} on ${REPO}..." >&2
ASSET_NAMES="$(gh release view "$TAG" --repo "$REPO" --json assets --jq '.assets[].name')"

has_asset() {
  local want="$1" line
  while IFS= read -r line; do
    [[ "$line" == "$want" ]] && return 0
  done <<<"$ASSET_NAMES"
  return 1
}

if ! has_asset "$ASSET"; then
  echo "error: release ${TAG} has no asset named ${ASSET}" >&2
  echo "assets found on that release:" >&2
  echo "$ASSET_NAMES" >&2
  exit 1
fi
if ! has_asset "$CHECKSUM_ASSET"; then
  echo "error: release ${TAG} has no checksum asset named ${CHECKSUM_ASSET}" >&2
  exit 1
fi

RELEASE_DATE="$(
  gh release view "$TAG" --repo "$REPO" --json publishedAt --jq '.publishedAt' \
    | sed -E 's/T.*$//'
)"
if [[ ! "$RELEASE_DATE" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
  echo "error: could not read a YYYY-MM-DD publish date for ${TAG} (got '${RELEASE_DATE}')" >&2
  exit 1
fi

# --- Read the published checksum, then verify it against the real installer bytes -------------
#
# The .sha256 asset is tiny (one line), so reading it is instant; the installer itself is not,
# but downloading it once here and checking it with `shasum` catches a truncated upload or a
# stale/mismatched checksum file before that hash reaches a public winget manifest.

CHECKSUM_URL="https://github.com/${REPO}/releases/download/${TAG}/${CHECKSUM_ASSET}"
CHECKSUM_LINE="$(curl -fsSL "$CHECKSUM_URL")"
SHA256="$(sed -E 's/^([0-9a-fA-F]{64}).*/\1/' <<<"$CHECKSUM_LINE")"
if [[ ! "$SHA256" =~ ^[0-9a-fA-F]{64}$ ]]; then
  echo "error: could not parse a sha256 hash out of ${CHECKSUM_URL}" >&2
  echo "got: ${CHECKSUM_LINE}" >&2
  exit 1
fi

INSTALLER_URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
TMP_MSI="$(mktemp -t formiga-winget-msi.XXXXXX)"
trap 'rm -f "$TMP_MSI"' EXIT
echo "Downloading ${ASSET} to verify its checksum (this is the real installer, ~7 MB)..." >&2
curl -fsSL "$INSTALLER_URL" -o "$TMP_MSI"
COMPUTED_SHA256="$(shasum -a 256 "$TMP_MSI" | sed -E 's/^([0-9a-fA-F]+).*/\1/')"
if [[ "$COMPUTED_SHA256" != "$SHA256" ]]; then
  echo "error: downloaded installer hash does not match the published .sha256 file" >&2
  echo "  published: ${SHA256}" >&2
  echo "  computed:  ${COMPUTED_SHA256}" >&2
  exit 1
fi
echo "Checksum verified against the published installer." >&2

# --- Render the templates -----------------------------------------------------------------

mkdir -p "$OUT_DIR"

render() {
  local template="$1" out="$2"
  sed \
    -e "s/@@VERSION@@/${VERSION}/g" \
    -e "s/@@SHA256@@/${SHA256}/g" \
    -e "s/@@RELEASE_DATE@@/${RELEASE_DATE}/g" \
    "$template" > "$out"
  if [[ -n "$(sed -n -E '/@@(VERSION|SHA256|RELEASE_DATE)@@/p' "$out")" ]]; then
    echo "error: ${out} still has an unresolved placeholder token - check the template" >&2
    exit 1
  fi
}

render "${TEMPLATE_DIR}/VonVan.Formiga.yaml.template" \
  "${OUT_DIR}/VonVan.Formiga.yaml"
render "${TEMPLATE_DIR}/VonVan.Formiga.installer.yaml.template" \
  "${OUT_DIR}/VonVan.Formiga.installer.yaml"
render "${TEMPLATE_DIR}/VonVan.Formiga.locale.en-US.yaml.template" \
  "${OUT_DIR}/VonVan.Formiga.locale.en-US.yaml"

echo
echo "Wrote:"
echo "  ${OUT_DIR}/VonVan.Formiga.yaml"
echo "  ${OUT_DIR}/VonVan.Formiga.installer.yaml"
echo "  ${OUT_DIR}/VonVan.Formiga.locale.en-US.yaml"
echo
echo "Next steps (see packaging/windows/winget/README.md):"
echo "  winget validate --manifest \"${OUT_DIR}\""
echo "  winget install --manifest \"${OUT_DIR}\"      # on a Windows machine or VM"
