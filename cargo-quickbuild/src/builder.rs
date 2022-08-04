use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use cargo::core::PackageId;
use filetime::FileTime;
use tempdir::TempDir;

use crate::archive::tar_target_dir;
use crate::archive::untar_target_dir;
use crate::description::get_tarball_path;
use crate::description::packages_to_cargo_toml_deps;
use crate::quick_resolve::QuickResolve;
use crate::stats::ComputedStats;
use crate::stats::Stats;
use crate::std_ext::ExitStatusExt;

pub(crate) fn build_tarball<'cfg, 'a>(
    resolve: &QuickResolve<'cfg, 'a>,
    tarball_dir: &Path,
    package_id: PackageId,
) -> Result<()> {
    let tempdir = TempDir::new("cargo-quickbuild-scratchpad")?;
    let scratch_dir = tempdir.path().join("cargo-quickbuild-scratchpad");

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

    // We write to a temporary location and then mv because mv is an atomic operation in posix
    // This has to be on the same filesystem, so we can't put it in the tempdir.
    let tarball_path = get_tarball_path(resolve, tarball_dir, package_id);
    let stats_path = tarball_path.with_extension("stats.json");

    let temp_tarball_path = tarball_path.with_extension("temp.tar");
    let temp_stats_path = temp_tarball_path.with_extension("stats.json");

    tar_target_dir(scratch_dir, &temp_tarball_path, &file_timestamps)?;
    stats.tar_done();

    serde_json::to_writer_pretty(
        std::fs::File::create(&temp_stats_path)?,
        &ComputedStats::from(stats),
    )?;

    std::fs::rename(&temp_stats_path, stats_path)?;
    std::fs::rename(&temp_tarball_path, &tarball_path)?;
    println!("wrote to {tarball_path:?}");

    Ok(())
}

pub(crate) fn cargo_init(scratch_dir: &std::path::PathBuf) -> Result<()> {
    command(["cargo", "init"])
        .arg(scratch_dir)
        .status()?
        .exit_ok_ext()?;

    Ok(())
}

pub(crate) fn unpack_tarballs_of_deps<'cfg, 'a>(
    resolve: &QuickResolve<'cfg, 'a>,
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
        file_timestamps.append(&mut untar_target_dir(
            resolve,
            tarball_dir,
            dep,
            scratch_dir,
        )?);
    }

    Ok(file_timestamps)
}

pub(crate) fn add_deps_to_manifest_and_run_cargo_build(
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

    // command(["cargo", "tree", "-vv", "--no-dedupe", "--edges=all"])
    //     .current_dir(scratch_dir)
    //     .status()?
    //     .exit_ok_ext()?;

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

pub(crate) fn command(args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Command {
    let mut args = args.into_iter();
    let mut command = Command::new(
        args.next()
            .expect("command() takes command and args (at least one item)"),
    );
    command.args(args);
    command
}
