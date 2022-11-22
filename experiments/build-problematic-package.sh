#!/bin/bash

set -euo pipefail

cargo install --locked --path $HOME/src/cargo-quick/cargo-quickbuild

# Identified by experiments/build-popular-quickinstall-releases.sh
crate=bat

cargo quickbuild install $crate
