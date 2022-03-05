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

(
    cd /tmp/cargo-quickbuild-hack
    # The dependencies section happens to be at the bottom of the file.
    # This may come in handy later.
    cargo init .
    cp $REPO_ROOT/experiments/breakdown/Cargo.toml .
    mkdir -p .cargo
    cat > .cargo/config.toml <<EOF
[build]
target-dir = "$REPO_ROOT/target"
EOF
    cat .cargo/config.toml
    time cargo build
)


commit "build breakdown from a tempdir"

cargo build -p breakdown
commit "build breakdown from inside the repo"

# Conclusion: building from a tempdir doesn't produce something that can be reused

git log --color=always -p --reverse | less  -R +?"$LAST_COMMIT_MESSAGE"
