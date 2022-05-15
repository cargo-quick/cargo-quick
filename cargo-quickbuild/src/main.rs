use std::fmt::Write as _;
use std::io::Write;
use std::ops::Deref;
use std::path::Path;
use std::time::{Duration, Instant};
use std::{error::Error, ffi::OsStr, process::Command};

use cargo::core::compiler::unit_graph::UnitDep;
use cargo::core::compiler::{CompileMode, Unit, UnitInterner};
use cargo::core::Workspace;
use cargo::ops::{create_bcx, CompileOptions};
use cargo::Config;
use crypto_hash::{hex_digest, Algorithm};
use tempdir::TempDir;

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

    let ws = Workspace::new(&Path::new("Cargo.toml").canonicalize()?, &config)?;
    let options = CompileOptions::new(&config, CompileMode::Build)?;
    let interner = UnitInterner::new();
    let bcx = create_bcx(&ws, &options, &interner)?;

    let mut units: Vec<(&Unit, &Vec<UnitDep>)> = bcx.unit_graph.iter().collect();
    units.sort_unstable();

    // libs with no lib deps and no build.rs
    let no_deps = units
        .iter()
        .filter(|(unit, deps)| unit.target.is_lib() && deps.is_empty())
        .map(|(u, _d)| *u);

    for unit in no_deps {
        // if unit.pkg.package_id().name() == "arrayvec" {
        //     dbg!(unit);
        //     break;
        // }
        // if unit.pkg.package_id().name() != "anyhow" {
        //     continue;
        // }
        // Note to self: anyhow appears 3 times:
        // * lib_target("anyhow", ["lib"], "anyhow-1.0.57/src/lib.rs", Edition2018),
        // * custom_build_target("build-script-build", "anyhow-1.0.57/build.rs", Edition2018),
        //   mode: RunCustomBuild,
        // * custom_build_target("build-script-build", "anyhow-1.0.57/build.rs", Edition2018),
        //   mode: Build,
        println!(
            "{} {}",
            unit.pkg.package_id().name(),
            unit.pkg.package_id().version()
        );
        // dbg!(deps);

        build_scratch_package(unit, &Vec::new())?;
    }

    Ok(())
}

fn build_scratch_package(unit: &Unit, deps: &Vec<UnitDep>) -> Result<(), Box<dyn Error>> {
    let deps_string = units_to_cargo_toml_deps(unit, deps);
    let digest = hex_digest(Algorithm::SHA256, deps_string.as_bytes());

    let package_name = unit.deref().pkg.name();
    let package_version = unit.deref().pkg.version();
    let tarball_prefix = format!("{package_name}-{package_version}-{digest}");
    build_tarball(deps_string, tarball_prefix)?;
    Ok(())
}

fn units_to_cargo_toml_deps(unit: &Unit, deps: &Vec<UnitDep>) -> String {
    let mut deps_string = String::new();
    std::iter::once(unit).chain(
        deps.iter().map(|dep| &dep.unit)
    )
    .for_each(|unit| {
        let package = &unit.deref().pkg;
        let name = package.name();
        let version = package.version().to_string();
        let features = &unit.deref().features;
        // FIXME: this will probably break when we have multiple versions  of the same
        // package in the tree. Could we include version.replace('.', '_') or something?
        writeln!(deps_string,
            r#"{name} = {{ version = "={version}", features = {features:?}, default-features = false }}"#
        ).unwrap();
    });
    deps_string
}

struct Stats {
    start: Instant,
    init_done: Option<Instant>,
    build_done: Option<Instant>,
    tar_done: Option<Instant>,
}
impl Stats {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            init_done: None,
            build_done: None,
            tar_done: None,
        }
    }
    fn init_done(&mut self) {
        self.init_done.replace(Instant::now());
    }
    fn build_done(&mut self) {
        self.build_done.replace(Instant::now());
    }
    fn tar_done(&mut self) {
        self.tar_done.replace(Instant::now());
    }
}

#[derive(serde::Serialize)]
struct ComputedStats {
    init_duration: Duration,
    build_duration: Duration,
    tar_duration: Duration,
}

impl From<Stats> for ComputedStats {
    fn from(stats: Stats) -> Self {
        Self {
            init_duration: stats.init_done.unwrap() - stats.start,
            build_duration: stats.build_done.unwrap() - stats.init_done.unwrap(),
            tar_duration: stats.tar_done.unwrap() - stats.build_done.unwrap(),
        }
    }
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
    let init_ok = command(["cargo", "init"])
        .arg(scratch_dir)
        .status()?
        .success();
    Ok(if !init_ok {
        Err("cargo init failed")?;
    })
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
    command(["cat"]).arg(&cargo_toml_path).status()?;
    let cargo_build_ok = command(["cargo", "build", "--offline"])
        .current_dir(scratch_dir)
        .status()?
        .success();
    Ok(if !cargo_build_ok {
        Err("cargo build failed")?;
    })
}

fn tar_target_dir(
    scratch_dir: std::path::PathBuf,
    temp_tarball_path: &std::path::PathBuf,
) -> Result<(), Box<dyn Error>> {
    Ok(
        if !command([
            "tar",
            "-f",
            &temp_tarball_path.to_string_lossy(),
            "--format=pax",
            "-c",
            "target",
        ])
        .current_dir(&scratch_dir)
        .status()?
        .success()
        {
            // FIXME: there is an unstable method for this: add it as an extension method?
            Err("tar failed")?;
        },
    )
}
// fn run_cargo_build(args: Vec<String>) -> Result<(), Box<dyn Error>> {
//     let mut command = Command::new("cargo");
//     command.arg("build").args(args);
//     println!("would run {command:?}");

//     Ok(())
// }

fn command(args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Command {
    let mut args = args.into_iter();
    let mut command = Command::new(
        args.next()
            .expect("command() takes command and args (at least one item)"),
    );
    command.args(args);
    command
}
