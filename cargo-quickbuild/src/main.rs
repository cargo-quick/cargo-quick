mod archive;
mod deps;
mod pax;
mod resolve;
mod resolve_ext;
mod stats;
mod std_ext;

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::fs::remove_dir_all;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{ffi::OsStr, process::Command};

use anyhow::{Context, Result};
use cargo::core::compiler::{CompileMode, UnitInterner};
use cargo::core::{PackageId, Workspace};
use cargo::ops::{CompileOptions, WorkspaceResolve};
use cargo::Config;
use crypto_hash::{hex_digest, Algorithm};
use filetime::FileTime;

use crate::resolve::create_resolve;
use crate::resolve_ext::ResolveExt;
use crate::stats::{ComputedStats, Stats};
use crate::std_ext::ExitStatusExt;

fn main() -> Result<()> {
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

    unpack_or_build_packages(&tarball_dir)?;
    Ok(())
}

fn outstanding_deps<'cfg>(
    resolve: &WorkspaceResolve<'cfg>,
    built_packages: &HashSet<PackageId>,
    package_id: PackageId,
) -> Vec<PackageId> {
    resolve
        .recursive_deps_including_self(package_id)
        .into_iter()
        .filter(|dep| dep != &package_id && !built_packages.contains(&dep))
        .collect()
}

fn unpack_or_build_packages(tarball_dir: &Path) -> Result<()> {
    let config = Config::default()?;

    // FIXME: compile cargo in release mode
    let ws = Workspace::new(&Path::new("Cargo.toml").canonicalize()?, &config)?;
    let options = CompileOptions::new(&config, CompileMode::Build)?;
    let interner = UnitInterner::new();
    let resolve = create_resolve(&ws, &options, &interner)?;

    // let root_package = resolve.sort()[0];
    let [root_package]: [_; 1] = resolve
        .targeted_resolve
        .iter()
        .filter(|id| id.name() == "curl-sys")
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let mut packages_to_build = resolve.recursive_deps_including_self(root_package);

    dbg!(&root_package);
    dbg!(&packages_to_build);
    assert!(packages_to_build.contains(&root_package));

    let mut built_packages: HashSet<PackageId> = Default::default();

    for level in 0..=7 {
        println!("START OF LEVEL {level}");
        let current_level;
        (current_level, packages_to_build) = packages_to_build.iter().partition(|package_id| {
            outstanding_deps(&resolve, &built_packages, **package_id).is_empty()
        });

        dbg!(&current_level);

        if current_level.is_empty() && !packages_to_build.is_empty() {
            println!(
                "We haven't compiled everything yet, but there is nothing left to do\n\npackages_to_build: {packages_to_build:#?}"
            );
            dbg!(&built_packages);
            for package_id in packages_to_build {
                dbg!((
                    package_id,
                    outstanding_deps(&resolve, &built_packages, package_id)
                ));
            }
            anyhow::bail!("current_level.is_empty() && !packages_to_build.is_empty()");
        }
        for package_id in current_level {
            build_tarball_if_not_exists(&resolve, tarball_dir, package_id)?;
            built_packages.insert(package_id);
        }
    }

    Ok(())
}

fn build_tarball_if_not_exists<'cfg>(
    resolve: &WorkspaceResolve<'cfg>,
    tarball_dir: &Path,
    package_id: PackageId,
) -> Result<()> {
    // if !package_id.target.is_lib() {
    //     log::info!("skipping {unit:?} for now, because it is not a lib");
    //     return Ok(());
    // }
    let deps_string = packages_to_cargo_toml_deps(resolve, package_id);

    let tarball_path = get_tarball_path(resolve, tarball_dir, package_id);
    println!("STARTING BUILD\n{tarball_path:?} deps:\n{}", deps_string);
    if tarball_path.exists() {
        println!("{tarball_path:?} already exists");
        return Ok(());
    }
    build_tarball(resolve, tarball_dir, package_id)
}

// FIXME: put a cache on this?
fn get_tarball_path<'cfg>(
    resolve: &WorkspaceResolve<'cfg>,
    tarball_dir: &Path,
    package_id: PackageId,
) -> PathBuf {
    let deps_string = packages_to_cargo_toml_deps(resolve, package_id);

    let digest = hex_digest(Algorithm::SHA256, deps_string.as_bytes());
    let package_name = package_id.name();
    let package_version = package_id.version();

    std::fs::create_dir_all(&tarball_dir).unwrap();

    tarball_dir.join(format!("{package_name}-{package_version}-{digest}.tar"))
}

fn packages_to_cargo_toml_deps<'cfg>(
    resolve: &WorkspaceResolve<'cfg>,
    package_id: PackageId,
) -> String {
    let mut deps_string = String::new();
    writeln!(
        deps_string,
        "# {} {}",
        package_id.name(),
        package_id.version()
    )
    .unwrap();

    dbg!(resolve.recursive_deps_including_self(package_id)).into_iter()
    .for_each(|package_id| {
        let name = package_id.name();
        let version = package_id.version().to_string();
        let features = resolve.targeted_resolve.features(package_id);
        let safe_version = version.replace(|c: char| !c.is_alphanumeric(), "_");
        writeln!(deps_string,
            r#"{name}_{safe_version} = {{ package = "{name}", version = "={version}", features = {features:?}, default-features = false }}"#
        ).unwrap();
    });
    deps_string
}

// HACK: keep tempdir location fixed to see if that fixes compilation issues.
struct FixedTempDir {
    path: PathBuf,
}

impl FixedTempDir {
    fn new(name: &str) -> Result<Self> {
        let path = std::env::temp_dir().join(name);
        let _ = remove_dir_all(&path);
        std::fs::create_dir(&path).with_context(|| format!("making tempdir in {path:?}"))?;
        Ok(FixedTempDir { path })
    }
}

impl Drop for FixedTempDir {
    fn drop(&mut self) {
        // let _ = remove_dir_all(&self.path);
    }
}

fn build_tarball<'cfg>(
    resolve: &WorkspaceResolve<'cfg>,
    tarball_dir: &Path,
    package_id: PackageId,
) -> Result<()> {
    let tempdir = FixedTempDir::new("cargo-quickbuild-scratchpad")?;
    let scratch_dir = tempdir.path.join("cargo-quickbuild-scratchpad");

    // FIXME: this stats tracking is making it awkward to refactor this method into multiple bits.
    // It might be better to make a Context struct that contains computed_deps and stats or something?
    let mut stats = Stats::new();

    // FIXME: do this by hand or something?
    cargo_init(&scratch_dir)?;
    stats.init_done();

    let file_timestamps = unpack_tarballs_of_deps(resolve, tarball_dir, package_id, &scratch_dir)?;
    stats.untar_done();

    let deps_string = packages_to_cargo_toml_deps(resolve, package_id);
    add_deps_to_manifest_and_run_cargo_build(deps_string, &scratch_dir)?;
    stats.build_done();

    // we write to a temporary location and then mv because mv is an atomic operation in posix
    let temp_tarball_path = tempdir.path.join("target.tar");
    let temp_stats_path = temp_tarball_path.with_extension("stats.json");

    archive::tar_target_dir(scratch_dir, &temp_tarball_path, &file_timestamps)?;
    stats.tar_done();

    serde_json::to_writer_pretty(
        std::fs::File::create(&temp_stats_path)?,
        &ComputedStats::from(stats),
    )?;

    let tarball_path = get_tarball_path(resolve, tarball_dir, package_id);
    std::fs::rename(&temp_stats_path, tarball_path.with_extension("stats.json"))?;
    std::fs::rename(&temp_tarball_path, &tarball_path)?;
    println!("wrote to {tarball_path:?}");

    Ok(())
}

fn cargo_init(scratch_dir: &std::path::PathBuf) -> Result<()> {
    command(["cargo", "init"])
        .arg(scratch_dir)
        .status()?
        .exit_ok_ext()?;

    Ok(())
}

fn unpack_tarballs_of_deps<'cfg>(
    resolve: &WorkspaceResolve<'cfg>,
    tarball_dir: &Path,
    package_id: PackageId,
    scratch_dir: &Path,
) -> Result<BTreeMap<PathBuf, FileTime>> {
    let mut file_timestamps = BTreeMap::default();
    for dep in resolve
        .recursive_deps_including_self(package_id)
        .into_iter()
        .filter(|id| id != &package_id)
    {
        // These should be *guaranteed* to already be built.
        file_timestamps.append(&mut archive::untar_target_dir(
            resolve,
            tarball_dir,
            dep,
            scratch_dir,
        )?);
    }

    Ok(file_timestamps)
}

fn add_deps_to_manifest_and_run_cargo_build(
    deps_string: String,
    scratch_dir: &std::path::PathBuf,
) -> Result<()> {
    let cargo_toml_path = scratch_dir.join("Cargo.toml");
    let mut cargo_toml = std::fs::OpenOptions::new()
        .write(true)
        .append(true)
        .open(&cargo_toml_path)?;
    write!(cargo_toml, "{}", deps_string)?;
    cargo_toml.flush()?;
    drop(cargo_toml);

    command(["cargo", "tree", "-vv", "--no-dedupe", "--edges=all"])
        .current_dir(scratch_dir)
        .status()?
        .exit_ok_ext()?;

    command(["cargo", "build", "--jobs=1", "--offline"])
        .current_dir(scratch_dir)
        .status()?
        .exit_ok_ext()?;

    command([
        "cargo",
        "clean",
        "--offline",
        "--package",
        "cargo-quickbuild-scratchpad",
    ])
    .current_dir(scratch_dir)
    .status()?
    .exit_ok_ext()?;

    Ok(())
}

fn command(args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Command {
    let mut args = args.into_iter();
    let mut command = Command::new(
        args.next()
            .expect("command() takes command and args (at least one item)"),
    );
    command.args(args);
    command
}
