#!/bin/bash

set -euxo pipefail

LAST_COMMIT_MESSAGE=""

commit() {
    LAST_COMMIT_MESSAGE="$1"

    gls --full-time -Rl target > timestamps.txt
    git add .
    git commit --allow-empty -am "$LAST_COMMIT_MESSAGE"
}


rm -rf target
cargo build

commit "clean cargo build timestamps"

sleep 2
cargo build

commit "noop cargo build timestamps"

sleep 2
touch cargo-quickinstall/src/main.rs
cargo build

commit "touched-main cargo build timestamps"

sleep 2
cargo build

commit "noop cargo build 2 timestamps"

# `pax` format seems to provide nanosecond accuracy, and is portable to bsd+gnu. 
# No idea why that's not the default.
tar --format=pax -c target > /tmp/target.tar
rm -rf target
tar -x -f /tmp/target.tar

commit "tar round-trip timestamps"

sleep 2
cargo build

commit "noop cargo build after tar timestamps"

git log --color=always -p --reverse | less  -R +?"$LAST_COMMIT_MESSAGE"
