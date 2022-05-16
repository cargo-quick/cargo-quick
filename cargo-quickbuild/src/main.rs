mod stats;
mod std_ext;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::{error::Error, ffi::OsStr, process::Command};

use cargo::core::compiler::unit_graph::UnitDep;
use cargo::core::compiler::{CompileMode, Unit, UnitInterner};
use cargo::core::Workspace;
use cargo::ops::{create_bcx, CompileOptions};
use cargo::Config;
use crypto_hash::{hex_digest, Algorithm};
use itertools::Itertools;
use tempdir::TempDir;

use std_ext::ExitStatusExt;

use crate::stats::{ComputedStats, Stats};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<_> = std::env::args().collect();
    if args[0] == "quickbuild" {
        args.remove(0);
    }
    unpack_or_build_packages()?;
    Ok(())
}

fn unpack_or_build_packages() -> Result<(), Box<dyn Error>> {
    let config = Config::default()?;

    // FIXME: compile cargo in release mode
    let ws = Workspace::new(&Path::new("Cargo.toml").canonicalize()?, &config)?;
    let options = CompileOptions::new(&config, CompileMode::Build)?;
    let interner = UnitInterner::new();
    let bcx = create_bcx(&ws, &options, &interner)?;

    let mut units: Vec<(&Unit, &Vec<UnitDep>)> = bcx
        .unit_graph
        .iter()
        .filter(|(unit, _)| unit.target.is_lib())
        .collect();
    units.sort_unstable();

    let mut computed_deps = BTreeMap::<&Unit, &Vec<UnitDep>>::default();

    for level in 0..=10 {
        println!("START OF LEVEL {level}");
        let current_level;
        // libs with no lib unbuilt deps and no build.rs
        (current_level, units) = units.iter().partition(|(unit, deps)| {
            unit.target.is_lib()
                && deps
                    .iter()
                    .all(|dep| (!dep.unit.target.is_lib()) || computed_deps.contains_key(&dep.unit))
        });

        if current_level.is_empty() && !units.is_empty() {
            println!(
                "We haven't compiled everything yet, but there is nothing left to do\n\n {units:#?}"
            );
            Err("current_level.is_empty() && !units.is_empty()")?;
        }
        for (unit, deps) in current_level {
            computed_deps.insert(unit, deps);
            build_tarball_if_not_exists(&computed_deps, unit)?;
        }
    }

    Ok(())
}

fn build_tarball_if_not_exists(
    computed_deps: &BTreeMap<&Unit, &Vec<UnitDep>>,
    unit: &Unit,
) -> Result<(), Box<dyn Error>> {
    let deps_string = units_to_cargo_toml_deps(computed_deps, unit);

    let tarball_path = get_tarball_path(computed_deps, unit);
    println!("\n{tarball_path:?} deps:\n{}", deps_string);
    if tarball_path.exists() {
        println!("{tarball_path:?} already exists");
        return Ok(());
    }
    build_tarball(computed_deps, unit)
}

// FIXME: put a cache on this?
fn get_tarball_path(computed_deps: &BTreeMap<&Unit, &Vec<UnitDep>>, unit: &Unit) -> PathBuf {
    let deps_string = units_to_cargo_toml_deps(computed_deps, unit);

    let digest = hex_digest(Algorithm::SHA256, deps_string.as_bytes());
    let package_name = unit.deref().pkg.name();
    let package_version = unit.deref().pkg.version();

    let tarball_dir = home::home_dir().unwrap().join("tmp/quick");
    std::fs::create_dir_all(&tarball_dir).unwrap();

    tarball_dir.join(format!("{package_name}-{package_version}-{digest}.tar"))
}

fn units_to_cargo_toml_deps(computed_deps: &BTreeMap<&Unit, &Vec<UnitDep>>, unit: &Unit) -> String {
    let mut deps_string = String::new();
    std::iter::once(unit).chain(
        flatten_deps(computed_deps, unit)
    )
    .unique()
    .for_each(|unit| {
        let package = &unit.deref().pkg;
        let name = package.name();
        let version = package.version().to_string();
        let features = &unit.deref().features;
        // FIXME: this will probably break when we have multiple versions  of the same
        // package in the tree. Could we include version.replace('.', '_') or something?
        let safe_version = version.replace(|c: char| !c.is_alphanumeric(), "_");
        writeln!(deps_string,
            r#"{name}_{safe_version} = {{ package = "{name}", version = "={version}", features = {features:?}, default-features = false }}"#
        ).unwrap();
    });
    deps_string
}

fn flatten_deps<'a>(
    computed_deps: &'a BTreeMap<&Unit, &Vec<UnitDep>>,
    unit: &'a Unit,
) -> Box<dyn Iterator<Item = &'a Unit> + 'a> {
    if !unit.target.is_lib() {
        return Box::new(std::iter::empty());
    }
    Box::new(
        (&*computed_deps.get(unit).unwrap())
            .iter()
            .map(|dep| &dep.unit)
            .filter(|dep| dep.target.is_lib())
            .flat_map(move |dep| {
                assert!(dep.target.is_lib());
                assert_ne!(dep, unit);
                assert_ne!(
                    dep.pkg, unit.pkg,
                    "package clash between:\n{dep:?}\nand\n{unit:?}"
                );
                std::iter::once(dep).chain(flatten_deps(computed_deps, dep))
            }),
    )
}

fn build_tarball(
    computed_deps: &BTreeMap<&Unit, &Vec<UnitDep>>,
    unit: &Unit,
) -> Result<(), Box<dyn Error>> {
    let tempdir = TempDir::new("cargo-quickbuild-scratchpad")?;
    let scratch_dir = tempdir.path().join("cargo-quickbuild-scratchpad");

    // FIXME: this stats tracking is making it awkward to refactor this method into multiple bits.
    // It might be better to make a Context struct that contains computed_deps and stats or something?
    let mut stats = Stats::new();

    // FIXME: do this by hand or something?
    cargo_init(&scratch_dir)?;
    stats.init_done();

    unpack_tarballs_of_deps(computed_deps, unit, &scratch_dir)?;
    stats.untar_done();

    let deps_string = units_to_cargo_toml_deps(computed_deps, unit);
    add_deps_to_manifest_and_run_cargo_build(deps_string, &scratch_dir)?;
    stats.build_done();

    // we write to a temporary location and then mv because mv is an atomic operation in posix
    let temp_tarball_path = tempdir.path().join("target.tar");
    let temp_stats_path = temp_tarball_path.with_extension("stats.json");

    tar_target_dir(scratch_dir, &temp_tarball_path)?;
    stats.tar_done();

    serde_json::to_writer_pretty(
        std::fs::File::create(&temp_stats_path)?,
        &ComputedStats::from(stats),
    )?;

    let tarball_path = get_tarball_path(computed_deps, unit);
    std::fs::rename(&temp_stats_path, tarball_path.with_extension("stats.json"))?;
    std::fs::rename(&temp_tarball_path, &tarball_path)?;
    println!("wrote to {tarball_path:?}");

    Ok(())
}

fn cargo_init(scratch_dir: &std::path::PathBuf) -> Result<(), Box<dyn Error>> {
    command(["cargo", "init"])
        .arg(scratch_dir)
        .status()?
        .exit_ok_ext()?;

    Ok(())
}

fn unpack_tarballs_of_deps(
    computed_deps: &BTreeMap<&Unit, &Vec<UnitDep>>,
    unit: &Unit,
    scratch_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    for dep in flatten_deps(computed_deps, unit).unique() {
        // These should be *guaranteed* to already be built.
        untar_target_dir(computed_deps, &dep, scratch_dir)?;
    }

    Ok(())
}

fn untar_target_dir(
    computed_deps: &BTreeMap<&Unit, &Vec<UnitDep>>,
    unit: &Unit,
    scratch_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    let tarball_path = get_tarball_path(computed_deps, unit);
    assert!(tarball_path.exists(), "{tarball_path:?} does not exist");
    println!("unpacking {tarball_path:?}");
    command(["tar", "-xf", &tarball_path.to_string_lossy(), "target"])
        .current_dir(&scratch_dir)
        .status()?
        .exit_ok_ext()?;

    Ok(())
}

fn add_deps_to_manifest_and_run_cargo_build(
    deps_string: String,
    scratch_dir: &std::path::PathBuf,
) -> Result<(), Box<dyn Error>> {
    let cargo_toml_path = scratch_dir.join("Cargo.toml");
    let mut cargo_toml = std::fs::OpenOptions::new()
        .write(true)
        .append(true)
        .open(&cargo_toml_path)?;
    write!(cargo_toml, "{}", deps_string)?;
    cargo_toml.flush()?;
    drop(cargo_toml);

    command(["cargo", "build", "--offline"])
        .current_dir(scratch_dir)
        .status()?
        .exit_ok_ext()
        .or_else(|_| {
            // this is for crossbeam-utils = "=0.8.3", which might have been yanked?
            println!("retrying without --offline");
            command(["cargo", "build"])
                .current_dir(scratch_dir)
                .status()?
                .exit_ok_ext()
        })?;

    Ok(())
}

fn tar_target_dir(
    scratch_dir: std::path::PathBuf,
    temp_tarball_path: &std::path::PathBuf,
) -> Result<(), Box<dyn Error>> {
    // FIXME: cargo already bundles tar as a dep, so just use that
    // FIXME: each tarball contains duplicates of all of the dependencies that we just unpacked already
    command([
        "tar",
        "-f",
        &temp_tarball_path.to_string_lossy(),
        "--format=pax",
        "-c",
        "target",
    ])
    .current_dir(&scratch_dir)
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
