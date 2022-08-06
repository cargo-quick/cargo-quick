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

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use cargo::core::compiler::RustcTargetData;
use cargo::core::compiler::{CompileMode, UnitInterner};
use cargo::core::dependency::DepKind;
use cargo::core::Package;
use cargo::core::{PackageId, Workspace};
use cargo::ops::{CompileOptions, Packages};
use cargo::Config;
use repo::Repo;

use crate::builder::{command, unpack_tarballs_of_deps};
use crate::quick_resolve::QuickResolve;
use crate::resolve::create_resolve;
use crate::scheduler::build_missing_packages;
use crate::std_ext::ExitStatusExt;
use crate::vendor::tree::{Charset, EdgeKind, Prefix, Target, TreeOptions};

fn main() -> Result<()> {
    // hack: disable target/.rustc_info.json nonsense.
    std::env::set_var("CARGO_CACHE_RUSTC_INFO", "0");

    pretty_env_logger::init();

    let mut args: Vec<_> = std::env::args().collect();
    if args[1] == "quickbuild" {
        args.remove(1);
    }
    // example invocation for testing:
    //     \in cargo-quickbuild/ cargo run -- $HOME/tmp/`git describe`
    let tarball_dir = match args.get(1) {
        Some(path) => PathBuf::from(path),
        None => home::home_dir().unwrap().join("tmp/quick"),
    };
    let repo = Repo::new(tarball_dir);

    let config = Config::default()?;

    // FIXME: compile cargo in release mode
    let ws = Workspace::new(&Path::new("Cargo.toml").canonicalize()?, &config)?;
    let options = CompileOptions::new(&config, CompileMode::Build)?;

    let interner = UnitInterner::new();
    let workspace_resolve = create_resolve(&ws, &options, &interner)?;
    let requested_kinds = &options.build_config.requested_kinds;
    let target_data = RustcTargetData::new(&ws, requested_kinds)?;
    let package_map: HashMap<PackageId, &Package> = workspace_resolve
        .pkg_set
        .packages()
        .map(|pkg| (pkg.package_id(), pkg))
        .collect();

    let packages = match &options.spec {
        Packages::Default => Packages::Default,
        Packages::All => Packages::All,
        Packages::OptOut(vec) => Packages::OptOut(vec.clone()),
        Packages::Packages(vec) => Packages::Packages(vec.clone()),
    };

    let opts = TreeOptions {
        cli_features: options.cli_features.clone(),
        packages,
        target: Target::Host,
        edge_kinds: [
            EdgeKind::Dep(DepKind::Normal),
            EdgeKind::Dep(DepKind::Build),
        ]
        .into_iter()
        .collect(),
        invert: Default::default(),
        pkgs_to_prune: Default::default(),
        prefix: Prefix::None,
        no_dedupe: Default::default(),
        duplicates: Default::default(),
        charset: Charset::Ascii,
        format: Default::default(),
        graph_features: Default::default(),
        max_display_depth: Default::default(),
        no_proc_macro: Default::default(),
    };
    let graph = vendor::tree::graph::build(
        &ws,
        &workspace_resolve.targeted_resolve,
        &workspace_resolve.resolved_features,
        &options.spec.to_package_id_specs(&ws)?,
        &options.cli_features,
        &target_data,
        requested_kinds,
        package_map,
        &opts,
    )
    .unwrap();
    let resolve = QuickResolve {
        ws: &ws,
        workspace_resolve: &workspace_resolve,
        graph,
    };

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
