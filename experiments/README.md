# Experiments

There is a very high risk of building a non-viable thing, so we have done a bunch of experiments to check that we're not building something impossible/pointless.

They are basically dead code at this point, but they are included for completeness.

## `fetch` and `breakdown`

These crates contain tools to fetch Cargo.toml and Cargo.lock files from all of github, and see who is using which packages with which feaures. The output was fed into an investigation in [a jupyter notebook](https://github.com/cargo-quick/quickbuild-analytics-data/blob/main/notebooks/deps.ipynb).

## `tar-investigation.sh`

This is to check the behaviour of `cargo build` when you tar/untar the target directory.

Run the script, and view the git log that it creates. You probably want to throw away the branch when you're done.

It turns out that `tar` defaults to only recording timestamps to the nearest second, which breaks cargo's fingerprints and triggers a full rebuild.
