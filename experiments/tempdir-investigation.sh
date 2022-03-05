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
# cat experiments/breakdown/Cargo.toml \
#     | grep -B1000000 -F [dependencies] \
#     > /tmp/cargo-quickbuild-hack/hack/Cargo.toml

# For some reason, if I exclude csv then the build doesn't find regex-automata at all
# for package in `cargo tree -p regex-automata --invert --prefix=none | sed 's/ .*//'`; do
#     cat experiments/breakdown/Cargo.toml \
#         | (grep "^$package" || true) \
#         | (grep -v "^globwalk" || true) \
#         >> /tmp/cargo-quickbuild-hack/hack/Cargo.toml
# done

# TODO: work out how to infer the set of features programmatically.
# It looks like `cargo metadata` will give us what we need here.
echo 'regex-automata = { version = "=0.1.9", default-features = false }' >> /tmp/cargo-quickbuild-hack/hack/Cargo.toml
echo 'byteorder = { version = "=1.4.3", default-features = false }' >> /tmp/cargo-quickbuild-hack/hack/Cargo.toml

# For some reason, workspace vs non-workspace seems to matter?
cat > /tmp/cargo-quickbuild-hack/Cargo.toml <<EOF
[workspace]
members = ["hack"]
EOF
cp Cargo.lock /tmp/cargo-quickbuild-hack/

(
    cd /tmp/cargo-quickbuild-hack/

    # omiting `-p regex-automata`` causes it to build more stuff, but not produce
    # libregex_automata.d and libregex_automata.rlib in the top level of
    # target/debug
    cargo build -p regex-automata
    tar --format=pax -c target > /tmp/target.tar
)

# TODO: investigate whether you can delete some of the workspace crates,
# sand still get the same build result.
# for file in experiments/breakdown/Cargo.toml experiments/breakdown/src/main.rs # Cargo.lock
# do
#     rm -rf /tmp/cargo-quickbuild-hack/$file
#     mkdir -p /tmp/cargo-quickbuild-hack/$file
#     rm -rf /tmp/cargo-quickbuild-hack/$file
#     cp $file /tmp/cargo-quickbuild-hack/$file
# done



rm -rf target
tar -x -f /tmp/target.tar
commit "build regex-automata from a tempdir and untar it here"

sleep 2
cargo build -p regex-automata
commit "build regex-automata from inside the repo"

git log --color=always -p --reverse | less  -R +?"$LAST_COMMIT_MESSAGE"

git reset $INITIAL_COMMIT
