mod stats;
mod std_ext;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write;
use std::ops::Deref;
use std::path::Path;
use std::{error::Error, ffi::OsStr, process::Command};

use cargo::core::compiler::unit_graph::UnitDep;
use cargo::core::compiler::{CompileMode, Unit, UnitInterner};
use cargo::core::Workspace;
use cargo::ops::{create_bcx, CompileOptions};
use cargo::Config;
use crypto_hash::{hex_digest, Algorithm};
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

    let mut units: Vec<(&Unit, &Vec<UnitDep>)> = bcx.unit_graph.iter().collect();
    units.sort_unstable();

    let mut computed_deps = BTreeMap::<&Unit, &Vec<UnitDep>>::default();

    for level in 0..=2 {
        println!("START OF LEVEL {level}");
        let current_level;
        // libs with no lib unbuilt deps and no build.rs
        (current_level, units) = units.iter().partition(|(unit, deps)| {
            unit.target.is_lib() && deps.iter().all(|dep| computed_deps.contains_key(&dep.unit))
        });

        for (unit, deps) in current_level {
            computed_deps.insert(unit, deps);
            build_scratch_package(&computed_deps, unit)?;
        }
    }

    Ok(())
}

fn build_scratch_package(
    computed_deps: &BTreeMap<&Unit, &Vec<UnitDep>>,
    unit: &Unit,
) -> Result<(), Box<dyn Error>> {
    let deps = *computed_deps.get(unit).unwrap();
    let deps_string = units_to_cargo_toml_deps(computed_deps, unit);

    let digest = hex_digest(Algorithm::SHA256, deps_string.as_bytes());

    let package_name = unit.deref().pkg.name();
    let package_version = unit.deref().pkg.version();
    let tarball_prefix = format!("{package_name}-{package_version}-{digest}");
    println!("\n{tarball_prefix} deps:\n{}", deps_string);

    for dep in deps {
        build_scratch_package(computed_deps, &dep.unit)?;
    }
    build_tarball(deps_string, tarball_prefix)?;
    Ok(())
}

fn units_to_cargo_toml_deps(computed_deps: &BTreeMap<&Unit, &Vec<UnitDep>>, unit: &Unit) -> String {
    let mut deps_string = String::new();
    std::iter::once(unit).chain(
        flatten_deps(computed_deps, unit)
    )
    .for_each(|unit| {
        let package = &unit.deref().pkg;
        let name = package.name();
        let version = package.version().to_string();
        let features = &unit.deref().features;
        // FIXME: this will probably break when we have multiple versions  of the same
        // package in the tree. Could we include version.replace('.', '_') or something?
        // We probably also want to deduplicate by unit equality.
        writeln!(deps_string,
            r#"{name} = {{ version = "={version}", features = {features:?}, default-features = false }}"#
        ).unwrap();
    });
    deps_string
}

fn flatten_deps<'a>(
    computed_deps: &'a BTreeMap<&Unit, &Vec<UnitDep>>,
    unit: &'a Unit,
) -> Box<dyn Iterator<Item = &'a Unit> + 'a> {
    Box::new(
        (&*computed_deps.get(unit).unwrap()).iter().flat_map(|dep| {
            std::iter::once(&dep.unit).chain(flatten_deps(computed_deps, &dep.unit))
        }),
    )
}

fn build_tarball(deps_string: String, tarball_prefix: String) -> Result<(), Box<dyn Error>> {
    let mut stats = Stats::new();
    let tarball_path = Path::new("/Users/alsuren/tmp").join(format!("{tarball_prefix}.tar"));
    if tarball_path.exists() {
        println!("{tarball_path:?} already exists");
        return Ok(());
    }
    let tempdir = TempDir::new("cargo-quickbuild-scratchpad")?;
    let scratch_dir = tempdir.path().join("cargo-quickbuild-scratchpad");

    cargo_init(&scratch_dir)?;
    stats.init_done();

    add_deps_to_manifest_and_run_cargo_build(deps_string, &scratch_dir)?;
    stats.build_done();

    // we write to a temporary location and then mv because mv is an atomic operation in posix
    let temp_tarball_path = tempdir.path().join("target.tar");
    let temp_stats_path = tarball_path.with_extension("stats.json");

    tar_target_dir(scratch_dir, &temp_tarball_path)?;
    stats.tar_done();

    serde_json::to_writer_pretty(
        std::fs::File::create(&temp_stats_path)?,
        &ComputedStats::from(stats),
    )?;
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
        .exit_ok_ext()?;

    Ok(())
}

fn tar_target_dir(
    scratch_dir: std::path::PathBuf,
    temp_tarball_path: &std::path::PathBuf,
) -> Result<(), Box<dyn Error>> {
    // FIXME: cargo already bundles tar as a dep, so just use that
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
