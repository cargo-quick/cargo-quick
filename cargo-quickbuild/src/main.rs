mod archive;
mod builder;
mod description;
mod pax;
mod quick_resolve;
mod repo;
mod resolve;
mod scheduler;
mod stats;
mod std_ext;
mod vendor;

use std::path::{Path, PathBuf};

use anyhow::Result;

use cargo::core::compiler::{CompileMode, UnitInterner};

use cargo::core::Workspace;
use cargo::ops::CompileOptions;
use cargo::Config;

use crate::builder::{command, unpack_tarballs_of_deps};
use crate::quick_resolve::create_quick_resolve;
use crate::repo::Repo;
use crate::resolve::create_resolve;
use crate::scheduler::build_missing_packages;
use crate::std_ext::ExitStatusExt;

fn main() -> Result<()> {
    // hack: disable target/.rustc_info.json nonsense.
    std::env::set_var("CARGO_CACHE_RUSTC_INFO", "0");

    pretty_env_logger::init();

    let mut args: Vec<_> = std::env::args().collect();
    if args[1] == "quickbuild" {
        args.remove(1);
    }

    let tarball_dir = match std::env::var("CARGO_QUICK_TARBALL_DIR") {
        Ok(path) => PathBuf::from(path),
        _ => home::home_dir().unwrap().join("tmp/quick"),
    };
    let repo = Repo::new(tarball_dir);

    let config = Config::default()?;

    let ws = Workspace::new(&Path::new("Cargo.toml").canonicalize()?, &config)?;
    let options = CompileOptions::new(&config, CompileMode::Build)?;

    let interner = UnitInterner::new();
    let workspace_resolve = create_resolve(&ws, &options, &interner)?;
    let resolve = create_quick_resolve(&ws, &options, &workspace_resolve)?;

    // FIXME: there has to be a better way to ask cargo for the list of root packages.
    let pkg = *resolve
        .workspace_resolve
        .targeted_resolve
        .sort()
        .last()
        .unwrap();
    let root_package = *resolve
        .workspace_resolve
        .targeted_resolve
        .path_to_top(&pkg)
        .last()
        .unwrap()
        .0;

    build_missing_packages(&resolve, &repo, root_package)?;
    let here = PathBuf::from(".");
    let repo_root = here.clone();

    assert!(
        !repo_root.join("target").exists(),
        "please remove your target dir before continuing"
    );

    unpack_tarballs_of_deps(&resolve, &repo, root_package, &repo_root)?;

    command(["cargo", "build"])
        .current_dir(&here)
        .status()?
        .exit_ok_ext()?;

    Ok(())
}
