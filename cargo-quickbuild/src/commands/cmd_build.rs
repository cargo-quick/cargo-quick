// I am allergic to files named build.rs that aren't build scripts. They bring me out in a rash.

use std::path::{Path, PathBuf};

use cargo::core::compiler::{CompileMode, UnitInterner};
use cargo::core::resolver::features::FeaturesFor;
use cargo::core::Workspace;
use cargo::ops::CompileOptions;
use cargo::Config;

use crate::builder::unpack_tarballs_of_deps;
use crate::quick_resolve::{create_quick_resolve, BuildFor};
use crate::repo::Repo;
use crate::resolve::create_resolve;
use crate::scheduler::build_missing_packages;
use crate::util::command::{command, CommandExt};

// At some point I will pick a command-line parsing crate, but for now this will do.
pub fn exec(args: &[String]) -> anyhow::Result<()> {
    assert_eq!(args[0], "build");
    assert_eq!(
        &args[1..],
        &args[0..0],
        "unexpected argument to `cargo quick`"
    );

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

    let repo = Repo::from_env();

    build_missing_packages(&resolve, &repo, root_package)?;
    let here = PathBuf::from(".");
    let repo_root = here.clone();

    assert!(
        !repo_root.join("target").exists(),
        "please remove your target dir before continuing"
    );

    unpack_tarballs_of_deps(
        &resolve,
        &repo,
        root_package,
        // FIXME: assert that we've not been asked to build a proc-macro crate.
        BuildFor(FeaturesFor::NormalOrDev),
        &repo_root,
    )?;

    command(["cargo", "build"])
        .current_dir(&here)
        .try_execute()?;

    Ok(())
}
