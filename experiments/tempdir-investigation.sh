#!/bin/bash

set -euxo pipefail

INITIAL_COMMIT=`git rev-parse HEAD`
LAST_COMMIT_MESSAGE=""

commit() {
    LAST_COMMIT_MESSAGE="$1"

    gls --full-time -Rl target > timestamps.txt
    git add .
    git commit --allow-empty -am "TMP: $LAST_COMMIT_MESSAGE"
}

REPO_ROOT="$PWD"

rm -rf target
rm -rf /tmp/cargo-quickbuild-hack

mkdir /tmp/cargo-quickbuild-hack

cargo init /tmp/cargo-quickbuild-hack/

# TODO: work out how to infer the set of features programmatically.
# It looks like `cargo metadata` will give us what we need here.
echo 'regex-automata = { version = "=0.1.9", default-features = false }' >> /tmp/cargo-quickbuild-hack/Cargo.toml
echo 'byteorder = { version = "=1.4.3", default-features = false }' >> /tmp/cargo-quickbuild-hack/Cargo.toml

(
    cd /tmp/cargo-quickbuild-hack/

    # omiting `-p regex-automata`` causes it to build more stuff, but not produce
    # libregex_automata.d and libregex_automata.rlib in the top level of
    # target/debug
    cargo build -p regex-automata
    tar --format=pax -c target > /tmp/target.tar
)

rm -rf target
tar -x -f /tmp/target.tar
commit "build regex-automata from a tempdir and untar it here"

sleep 2
cargo build -p regex-automata
commit "build regex-automata from inside the repo"

git log --color=always -p --reverse | less  -R +?"$LAST_COMMIT_MESSAGE"

git reset $INITIAL_COMMIT
