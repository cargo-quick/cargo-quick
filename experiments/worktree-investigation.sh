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
rm -rf /tmp/cargo-quickbuild-hack
git worktree add -f /tmp/cargo-quickbuild-hack HEAD

(
    cd /tmp/cargo-quickbuild-hack
    rm -rf target
    cargo build -p regex-automata
    tar --format=pax -c target > /tmp/target.tar
)
rm -rf target
tar -x -f /tmp/target.tar

commit "tar extraction timestamps"

sleep 2
cargo build -p regex-automata

commit "cargo build after untarring"

git log --color=always -p --reverse | less  -R +?"$LAST_COMMIT_MESSAGE"
