#!/bin/bash
# public-api.sh - check, or update, the public API of the contracts the companion apps build on.
#
# Formiga Hill, Home, Farm and Battle take these crates from Desktop by release tag, so a change to
# what they make public is a change those apps meet at their next repin. Each crate's API, as
# cargo-public-api lists it, is kept beside it in public-api.txt; CI fails when the two differ, so
# such a change is made on purpose and seen in review.
#
# formiga-core and formiga-art are taken by tag too, but they are the game itself and change
# with most work, so their API is not kept here: the companion-apps workflow builds and tests
# Hill, Home and Farm against them instead, which catches a change to anything those apps use.
# After an intended change to one of the crates below:
#
#   scripts/public-api.sh --update
#
# Needs cargo-public-api 0.52.0 (`cargo install cargo-public-api --locked --version 0.52.0`) and
# the nightly toolchain it reads rustdoc's JSON from
# (`rustup toolchain install nightly-2026-10-16 --profile minimal`). Both are pinned, since a newer
# nightly can change how rustdoc describes the same API; move them together, with --update.

set -euo pipefail
cd "$(dirname "$0")/.."

TOOLCHAIN="nightly-2026-10-16"
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
    # The toolchain is named the only way cargo-public-api takes one: as cargo's own.
    listed=$(cargo +"$TOOLCHAIN" public-api --manifest-path "crates/$crate/Cargo.toml" \
        --simplified --color never)
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
