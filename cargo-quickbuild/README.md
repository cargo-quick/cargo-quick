# Cargo Quickbuild

`cargo-quickbuild` is a project idea that came out of discussions around [cargo-quickinstall](https://github.com/alsuren/cargo-quickinstall/). The grand plan is that you can say `cargo quickbuild` (or `cargo quickrun` or `cargo quicktest`?) and it will call out to a service to fetch precompiled assets for dependencies, and then hand off to the non-quick version of the cargo command to finish the job. It is probably a moon-shot, so we did a bunch of analysis before we start, to get an idea of how viable it is. See [VIABILITY.md](./VIABILITY.md) for details.

## License

Cargo is primarily distributed under the terms of both the MIT license
and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
