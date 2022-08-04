use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;
use cargo::core::PackageId;
use filetime::FileTime;
use tar::Archive;
use tempdir::TempDir;

use crate::archive::tar_target_dir;
use crate::archive::tracked_unpack;

use crate::description::PackageDescription;
use crate::quick_resolve::QuickResolve;
use crate::repo::Repo;

use crate::stats::Stats;
use crate::std_ext::ExitStatusExt;

pub fn build_tarball<'cfg, 'a>(
    resolve: &QuickResolve<'cfg, 'a>,
    repo: &Repo,
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

    let file_timestamps = unpack_tarballs_of_deps(resolve, repo, package_id, &scratch_dir)?;
    stats.untar_done();

    let description = PackageDescription::new(resolve, package_id);

    add_deps_to_manifest_and_run_cargo_build(description.cargo_toml_deps(), &scratch_dir)?;
    stats.build_done();

    let description = PackageDescription::new(resolve, package_id);
    let file = repo.write(&description)?;
    tar_target_dir(scratch_dir, file, &file_timestamps)?;
    stats.tar_done();

    repo.commit(&description, stats)?;

    Ok(())
}

pub fn cargo_init(scratch_dir: &std::path::PathBuf) -> Result<()> {
    command(["cargo", "init"])
        .arg(scratch_dir)
        .status()?
        .exit_ok_ext()?;

    Ok(())
}

pub fn unpack_tarballs_of_deps<'cfg, 'a>(
    resolve: &QuickResolve<'cfg, 'a>,
    repo: &Repo,
    package_id: PackageId,
    scratch_dir: &Path,
) -> Result<BTreeMap<PathBuf, FileTime>> {
    let mut file_timestamps = BTreeMap::default();
    for dep in resolve
        .recursive_deps_including_self(package_id)
        .into_iter()
        .filter(|id| id != &package_id)
    {
        let description = PackageDescription::new(resolve, dep);
        let file = repo
            .read(&description)
            .with_context(|| format!("reading description {description:?} for {package_id:?}"))?;
        let mut archive = Archive::new(file);
        // These should be *guaranteed* to already be built.
        file_timestamps.append(&mut tracked_unpack(&mut archive, scratch_dir)?);
    }

    Ok(file_timestamps)
}

pub fn add_deps_to_manifest_and_run_cargo_build(
    deps_string: &str,
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

pub fn command(args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Command {
    let mut args = args.into_iter();
    let mut command = Command::new(
        args.next()
            .expect("command() takes command and args (at least one item)"),
    );
    command.args(args);
    command
}
