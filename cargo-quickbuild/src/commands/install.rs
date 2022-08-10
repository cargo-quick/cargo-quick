use std::collections::HashSet;

use anyhow::bail;
use cargo::core::compiler::{CompileMode, UnitInterner};
use cargo::core::{Dependency, Package, PackageId, Source, SourceId, Workspace};
use cargo::ops::CompileOptions;
use cargo::sources::SourceConfigMap;
use cargo::util::Filesystem;
use cargo::{CargoResult, Config};
use tempdir::TempDir;

use crate::builder::unpack_tarballs_of_deps;
use crate::quick_resolve::create_quick_resolve;

use crate::repo::Repo;
use crate::resolve::create_resolve;
use crate::scheduler::build_missing_packages;

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
        true,
        true,
        true,
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

        let ws = Workspace::ephemeral(package.clone(), &config, Some(target_dir), false)?;
        let options = CompileOptions::new(&config, CompileMode::Build)?;

        let interner = UnitInterner::new();
        let workspace_resolve = create_resolve(&ws, &options, &interner)?;
        let resolve = create_quick_resolve(&ws, &options, &workspace_resolve)?;

        let repo = Repo::from_env();
        build_missing_packages(&resolve, &repo, package.package_id())?;

        unpack_tarballs_of_deps(&resolve, &repo, package.package_id(), tempdir.path())?;
    }

    log::warn!("Not actually running cargo install because it doesn't work (isn't faster) yet. \
        The purpose of `cargo quickbuild install` in its current form is to smoke-test the \
        package building process, with a bunch of packages from the `cargo-quickinstall` most-requested-packages list.");
    // FIXME: this isn't actually any cheaper than doing a cargo install from scratch,
    // so it kind-of defeats the point of using cargo-quickbuild.
    // command([
    //     "cargo",
    //     "install",
    //     "--offline",
    //     "--debug",
    //     "--force",
    //     "--target-dir",
    //     tempdir.path().join("target").to_str().unwrap(),
    //     krate,
    // ])
    // .status()?
    // .exit_ok_ext()?;

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