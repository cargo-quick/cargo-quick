# Experiments

There is a very high risk of building a non-viable thing, so we have done a bunch of experiments to check that we're not building something impossible/pointless.

Most if the experiments in this folder are basically dead code, but they are included for completeness.

## Rust crates (`fetch` and `breakdown`)

These crates contain tools to fetch Cargo.toml and Cargo.lock files from all of github, and see who is using which packages with which feaures. The output was fed into an investigation in [a jupyter notebook](https://github.com/cargo-quick/quickbuild-analytics-data/blob/main/notebooks/deps.ipynb).

## Shell scripts

A few of the shell scripts assume that you have this repo and a few others checked out in `$HOME/src`. They also tend to make changes to `$HOME/tmp` (if for no other reason that the cargo quickbuild tarball repo defaults to `$HOME/tmp/quick`).
