#!/bin/bash
# public-api.sh - check, or update, the public API of the crates the companion apps build on.
#
# Formiga Hill, Home, Farm and Battle take these crates from Desktop by release tag, so a change to
# what they make public is a change those apps meet at their next repin. Each crate's API, as
# cargo-public-api lists it, is kept beside it in public-api.txt; CI fails when the two differ, so
# such a change is made on purpose and seen in review. After an intended change:
#
#   scripts/public-api.sh --update
#
# Needs cargo-public-api (`cargo install cargo-public-api --locked`) and a nightly toolchain
# (`rustup toolchain install nightly --profile minimal`), which it uses for rustdoc's JSON output.

set -euo pipefail
cd "$(dirname "$0")/.."

CRATES="formiga-expansion-rulebook formiga-travel formiga-home-contract formiga-farm-contract formiga-battle-contract"
UPDATE=0
if [ "${1:-}" = "--update" ]; then
    UPDATE=1
fi

# What each crate lists now is also written under target/public-api/, so CI can hand it back.
mkdir -p target/public-api
changed=0
for crate in $CRATES; do
    snapshot="crates/$crate/public-api.txt"
    listed=$(cargo public-api --manifest-path "crates/$crate/Cargo.toml" --simplified --color never)
    printf '%s\n' "$listed" > "target/public-api/$crate.txt"
    if [ "$UPDATE" = 1 ]; then
        printf '%s\n' "$listed" > "$snapshot"
        echo "updated $snapshot"
    elif ! diff -u "$snapshot" <(printf '%s\n' "$listed") > /dev/null 2>&1; then
        echo "::error file=$snapshot::$crate's public API is not the one in $snapshot"
        diff -u "$snapshot" <(printf '%s\n' "$listed") || true
        changed=1
    else
        echo "$crate: public API as kept"
    fi
done

if [ "$changed" = 1 ]; then
    echo
    echo "A public API the companion apps build on has changed. If that is meant, run"
    echo "scripts/public-api.sh --update and commit the files it writes."
    exit 1
fi
