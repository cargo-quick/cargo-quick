#!/bin/bash

set -euo pipefail

if [ "$(hostname)" != "admins-Virtual-Machine.local" ] && [ "$DRY_RUN" != 1 ]; then
    echo "This script must be run in a sandbox.
        brew install cirruslabs/cli/tart
        tart clone ghcr.io/cirruslabs/macos-monterey-base:latest monterey-base
        tart run monterey-base
    (or something more convenient without a UI if you prefer).
    "
    exit 1
fi

cargo install --locked --path $HOME/src/cargo-quick/cargo-quickbuild

cat ../quickbuild-analytics-data/stats-2022-07-24.json \
    | jq 'to_entries | sort_by(-.value) | map(.key) | .[]' -r \
    | grep aarch64-apple-darwin \
    | head -n 100 \
    | while read path; do
        tag="$(echo "$path" | sed 's:/:-:g')"
        crate="$(echo "$path" | sed 's:/.*::')"
        echo $tag
        if [ "$DRY_RUN" = 1 ]; then
            echo "would build $crate"
            continue
        fi
        if [ "$(hostname)" != "admins-Virtual-Machine.local" ]; then
            echo "this script must be run in a sandbox"
            continue
        fi

        \in ../cargo-quickinstall git rev-parse "$tag" || continue
        if cargo quickbuild install "$crate" 2>&1 > "$crate.out" ; then
            echo "$crate" >> success.txt
        else
            echo "$crate" >> failure.txt
        fi
    done
