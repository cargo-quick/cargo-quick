#!/bin/bash

set -euo pipefail

OTHER_GIT_REPO=${1?USAGE $0 path/to/rust/crate/repo target/problematic/file}
PROBLEM=${2?USAGE $0 path/to/rust/crate/repo target/problematic/file}

CARGO_QUICK_TARBALL_DIR="$HOME/tmp/$(basename "$OTHER_GIT_REPO")"
export CARGO_QUICK_TARBALL_DIR

# nightly gives backtraces for anyhow errors
cargo +nightly install --path $HOME/src/cargo-quick/cargo-quickbuild

rm -rf "$OTHER_GIT_REPO/target"

\in "$OTHER_GIT_REPO" cargo quickbuild && exit 0

for f in "$CARGO_QUICK_TARBALL_DIR"/*.tar ; do
    listing=$(~/.nix-profile/bin/tar --list -vv --full-time -f $f "$PROBLEM" 2>/dev/null) \
      || continue
    echo "$f"
    echo "$listing"
    ~/.nix-profile/bin/tar -x --to-stdout -f $f "$PROBLEM"
done
