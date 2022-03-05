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

REPO_ROOT="$PWD"

rm -rf target
rm -rf /tmp/cargo-quickbuild-hack

mkdir /tmp/cargo-quickbuild-hack

# TODO: think about how the features get resolved.
cargo init /tmp/cargo-quickbuild-hack/hack
### This doesn't work:
# # The [dependencies] section is at the bottom of the Cargo.toml
# cargo tree -p regex-automata --prefix none \
#     | sed -e 's/ v/ = "=/' -e 's/$/"/' \
#     >> /tmp/cargo-quickbuild-hack/hack/Cargo.toml
cp experiments/breakdown/Cargo.toml /tmp/cargo-quickbuild-hack/hack/

# For some reason, workspace vs non-workspace seems to matter?
cat > /tmp/cargo-quickbuild-hack/Cargo.toml <<EOF
[workspace]
members = ["hack"]
EOF
cp Cargo.lock /tmp/cargo-quickbuild-hack/

(
    cd /tmp/cargo-quickbuild-hack

    cargo build -p regex-automata
    tar --format=pax -c target > /tmp/target.tar
)

# TODO: investigate whether you can delete some of the workspace crates,
# sand still get the same build result.
for file in Cargo.lock experiments/breakdown/Cargo.toml experiments/breakdown/src/main.rs; do
    rm -rf /tmp/cargo-quickbuild-hack/$file
    mkdir -p /tmp/cargo-quickbuild-hack/$file
    rm -rf /tmp/cargo-quickbuild-hack/$file
    cp $file /tmp/cargo-quickbuild-hack/$file
done



rm -rf target
tar -x -f /tmp/target.tar
commit "build regex-automata from a tempdir and untar it here"

sleep 2
cargo build -p regex-automata
commit "build regex-automata from inside the repo"

git log --color=always -p --reverse | less  -R +?"$LAST_COMMIT_MESSAGE"

git reset $INITIAL_COMMIT
