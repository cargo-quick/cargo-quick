# `cargo quick`

⚠️ Warning: Currently Vapourware

Please jump in on the issue tracker if you would like to help out with anything.

`cargo quick` is intended as an umberella command for a bunch of others. Specifically:

## `cargo quick install`

A faster replacement for `cargo install`.

This is a thin wrapper around [cargo-quickinstall](https://github.com/alsuren/cargo-quickinstall/) (and will install it if it doesn't already exist).

At some point, I might move the quickinstall client into this repo, but the package archive will probably live in the alsuren/cargo-quickinstall namespace forever.

## `cargo quick build`

A faster replacement for `cargo build`.

See [cargo-quickbuild](./cargo-quickbuild/README.md).

This is a project idea that has been on my mind for a while now. The idea is to split out the layers of your dependency tree into tarballs, and unpack them like layers of a docker image. With a bit of careful thought, it should be possible to re-use the packages lower down in the tree in lots of other projects.
