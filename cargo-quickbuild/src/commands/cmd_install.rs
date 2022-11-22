use std::collections::HashSet;

use anyhow::bail;
use cargo::core::compiler::{CompileMode, UnitInterner};
use cargo::core::resolver::features::FeaturesFor;
use cargo::core::{Dependency, Package, PackageId, Source, SourceId, Workspace};
use cargo::ops::CompileOptions;
use cargo::sources::SourceConfigMap;
use cargo::util::Filesystem;
use cargo::{CargoResult, Config};

use crate::builder::unpack_tarballs_of_deps;
use crate::quick_resolve::{create_quick_resolve, BuildFor};
use crate::repo::Repo;
use crate::resolve::create_resolve;
use crate::scheduler::build_missing_packages;
use crate::util::command::{command, CommandExt};
use crate::util::fixed_tempdir::FixedTempDir as TempDir;

// At some point I will pick a command-line parsing crate, but for now this will do.
pub fn exec(args: &[String]) -> anyhow::Result<()> {
    assert_eq!(args[0], "install");
    if args.len() != 2 {
        bail!("USAGE: cargo quickbuild install $package_name");
    }
    let krate = args[1].as_str();
    assert_eq!(args, &["install", krate]);

    let mut config = Config::default()?;
    config.reload_rooted_at(home::cargo_home()?)?;
    let tempdir = TempDir::new("cargo-quickbuild-install-scratchpad")?;
    config.configure(
        0,
        false,
        None,
        false,
        false,
        false,
        &Some(tempdir.path().join("target")),
        &[],
        &[],
    )?;

    let source_id = SourceId::crates_io(&config)?;
    let map = SourceConfigMap::new(&config)?;

    let mut source = map.load(source_id, &HashSet::new())?;

    // Avoid pre-release versions from crate.io
    // unless explicitly asked for
    let vers = Some(String::from("*"));
    let dep = Dependency::parse(krate, vers.as_deref(), source_id)?;
    let package = select_dep_pkg(&mut source, dep, &config, false)?;

    {
        let target_dir = Filesystem::new(tempdir.path().join("target"));

        let mut ws = Workspace::ephemeral(package.clone(), &config, Some(target_dir), false)?;
        ws.set_ignore_lock(true);
        let options = CompileOptions::new(&config, CompileMode::Build)?;

        let interner = UnitInterner::new();
        let workspace_resolve = create_resolve(&ws, &options, &interner)?;
        let resolve = create_quick_resolve(&ws, &options, &workspace_resolve, &interner)?;

        let repo = Repo::from_env();
        build_missing_packages(&resolve, &repo, package.package_id())?;

        unpack_tarballs_of_deps(
            &resolve,
            &repo,
            package.package_id(),
            BuildFor(FeaturesFor::NormalOrDev),
            tempdir.path(),
        )?;
    }

    // FIXME: this doesn't end up building with the same profile as `run_cargo_build()`, so it
    // duplicates all of the work.
    command([
        "cargo",
        "install",
        "--offline",
        "--debug",
        "--force",
        "--target-dir",
        tempdir.path().join("target").to_str().unwrap(),
        krate,
    ])
    .try_execute()?;

    Ok(())
}

/// Gets a Package based on command-line requirements.
/// Copy-pasta from cargo/ops/common_for_install_and_uninstall.rs
pub fn select_dep_pkg<T>(
    source: &mut T,
    dep: Dependency,
    config: &Config,
    needs_update: bool,
) -> CargoResult<Package>
where
    T: Source,
{
    // This operation may involve updating some sources or making a few queries
    // which may involve frobbing caches, as a result make sure we synchronize
    // with other global Cargos
    let _lock = config.acquire_package_cache_lock()?;

    if needs_update {
        source.update()?;
    }

    let deps = source.query_vec(&dep)?;
    match deps.iter().map(|p| p.package_id()).max() {
        Some(pkgid) => {
            let pkg = Box::new(source).download_now(pkgid, config)?;
            Ok(pkg)
        }
        None => {
            let is_yanked: bool = if dep.version_req().is_exact() {
                let version: String = dep.version_req().to_string();
                PackageId::new(dep.package_name(), &version[1..], source.source_id())
                    .map_or(false, |pkg_id| source.is_yanked(pkg_id).unwrap_or(false))
            } else {
                false
            };
            if is_yanked {
                bail!(
                    "cannot install package `{}`, it has been yanked from {}",
                    dep.package_name(),
                    source.source_id()
                )
            } else {
                bail!(
                    "could not find `{}` in {} with version `{}`",
                    dep.package_name(),
                    source.source_id(),
                    dep.version_req(),
                )
            }
        }
    }
}
