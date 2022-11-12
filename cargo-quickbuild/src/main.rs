mod archive;
mod builder;
mod commands;
mod description;
mod pax;
mod quick_resolve;
mod repo;
mod resolve;
mod scheduler;
mod stats;
pub mod util;
mod vendor;

use anyhow::Result;

fn main() -> Result<()> {
    // hack: disable target/.rustc_info.json nonsense.
    std::env::set_var("CARGO_CACHE_RUSTC_INFO", "0");

    pretty_env_logger::init();

    let mut args: Vec<_> = std::env::args().collect();
    if args[1] == "quickbuild" {
        args.remove(1);
    }
    if args.is_empty() {
        args.push("build".to_string())
    }

    match args[1].as_str() {
        "build" => commands::cmd_build::exec(&args[1..]),
        "install" => commands::cmd_install::exec(&args[1..]),
        "repo" => commands::cmd_repo::exec(&args[1..]),
        // FIXME:
        // * I intend to use `cargo quick` as a thin bootstrapping tool. If we are being called
        //   from `cargo quick`, we should adjust our usage messages appropriately.
        // * We probably shouldn't panic here - the user doesn't need a backtrace. Is it possible
        //   to configure anyhow::Result so that it sets a nonzero exit code, but doesn't print a
        //   backtrace, or do I need to wrap it somehow (can I copy-pasta whatever `cargo` does?)
        _ => panic!("Usage: cargo quickbuild build"),
    }
}
