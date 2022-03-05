#!/bin/bash

set -euxo pipefail

INITIAL_COMMIT=`git rev-parse HEAD`

LAST_COMMIT_MESSAGE=""

commit() {
    LAST_COMMIT_MESSAGE="$1"

    gls --full-time -Rl target > timestamps.txt
    git add .
    git commit --allow-empty -am "$LAST_COMMIT_MESSAGE"
}


rm -rf target
cargo build -p regex-automata

commit "clean cargo build timestamps"

sleep 2
cargo build -p regex-automata

commit "noop cargo build timestamps"

# `pax` format seems to provide nanosecond accuracy, and is portable to bsd+gnu. 
# No idea why that's not the default.
tar --format=pax -c target > /tmp/target.tar
rm -rf target
tar -x -f /tmp/target.tar

commit "tar round-trip timestamps"

sleep 2
cargo build -p regex-automata

commit "noop cargo build after tar timestamps"

git log --color=always -p --reverse | less  -R +?"$LAST_COMMIT_MESSAGE"

git reset $INITIAL_COMMIT
