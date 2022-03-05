#!/bin/bash

set -euxo pipefail

LAST_COMMIT_MESSAGE=""

commit() {
    LAST_COMMIT_MESSAGE="$1"

    gls --full-time -Rl target > timestamps.txt
    git add .
    git commit --allow-empty -am "$LAST_COMMIT_MESSAGE"
}

REPO_ROOT="$PWD"

rm -rf target
rm -rf /tmp/cargo-quickbuild-hack

mkdir /tmp/cargo-quickbuild-hack
cargo init /tmp/cargo-quickbuild-hack

for file in `git ls-files | grep -E '(Cargo|lib.rs|main.rs)' `; do
    rm -rf /tmp/cargo-quickbuild-hack/$file
    mkdir -p /tmp/cargo-quickbuild-hack/$file
    rm -rf /tmp/cargo-quickbuild-hack/$file
    cp $file /tmp/cargo-quickbuild-hack/$file
done

(
    cd /tmp/cargo-quickbuild-hack
    # The dependencies section happens to be at the bottom of the file.
    # This may come in handy later.
    cargo build -p regex-automata
    tar --format=pax -c target > /tmp/target.tar
)


rm -rf target
tar -x -f /tmp/target.tar
commit "build regex-automata from a tempdir and untar it here"

sleep 2
cargo build -p regex-automata
commit "build regex-automata from inside the repo"

# Conclusion: building from a tempdir doesn't produce something that can be reused

git log --color=always -p --reverse | less  -R +?"$LAST_COMMIT_MESSAGE"
