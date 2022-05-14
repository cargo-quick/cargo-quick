use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::{error::Error, ffi::OsStr, process::Command};

use cargo::core::compiler::unit_graph::UnitDep;
use cargo::core::compiler::{CompileMode, Unit, UnitInterner};
use cargo::core::Workspace;
use cargo::ops::{create_bcx, CompileOptions};
use cargo::Config;
use tempdir::TempDir;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<_> = std::env::args().collect();
    if args[0] == "quickbuild" {
        args.remove(0);
    }
    unpack_or_build_packages()?;
    // run_cargo_build(args)?;
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
    }

    Ok(())
}

// fn unpack_or_build_subtree(
//     initials: FeatureSet,
//     cargo_options: &CargoOptions,
//     package: FeatureList,
// ) -> Result<(), Box<dyn Error>> {
//     // Notice that we're flipping things about here: make a cargo set from our
//     // package downwards, but taking the features from the set of packages in
//     // the repo.
//     let cargo_set = CargoSet::new(
//         package
//             .package()
//             .to_package_set()
//             .to_feature_set(StandardFeatures::None),
//         initials.clone(),
//         &cargo_options,
//     )
//     .expect("cargo resolution should succeed");

//     let packages: Vec<_> = cargo_set
//         .host_features()
//         .packages_with_features(DependencyDirection::Reverse)
//         .collect();

//     if packages.is_empty() {
//         println!(
//             "skipping {package:?} {version:?}",
//             package = package.package().name(),
//             version = package.package().version().to_string(),
//         );
//         return Ok(());
//     }

//     // build_scratch_package(packages)?;

//     println!(
//         "building {package:?} {version:?}",
//         package = package.package().name(),
//         version = package.package().version().to_string(),
//     );
//     // println!("built {package:?}");
//     Ok(())
// }

// fn build_scratch_package(packages: Vec<FeatureList>) -> Result<(), Box<dyn Error>> {
//     let tempdir = TempDir::new("cargo-quickbuild-scratchpad")?;
//     let scratch_dir = tempdir.path().join("cargo-quickbuild-scratchpad");
//     let init_ok = command(["cargo", "init"])
//         .arg(&scratch_dir)
//         .status()?
//         .success();
//     if !init_ok {
//         Err("cargo init failed")?;
//     }

//     let cargo_toml_path = scratch_dir.join("Cargo.toml");
//     let mut cargo_toml = std::fs::OpenOptions::new()
//         .write(true)
//         .append(true)
//         .open(&cargo_toml_path)?;

//     packages.iter()
//     .map(|package| -> std::io::Result<()>{
//         let name = package.package().name();
//         let version = package.package().version().to_string();
//         let features = package.features();
//         // FIXME: this will probably break when we have multiple versions  of the same
//         // package in the tree. Could we include version.replace('.', '_') or something?
//         writeln!(cargo_toml,
//             r#"{name} = {{ version = "={version}", features = {features:?}, default-features = false }}"#
//         )
//     }).collect::<Result<_, std::io::Error>>()?;
//     cargo_toml.flush()?;
//     drop(cargo_toml);

//     command(["cat"]).arg(&cargo_toml_path).status()?;

//     let cargo_build_ok = command(["cargo", "build"])
//         .current_dir(&scratch_dir)
//         .status()?
//         .success();

//     if !cargo_build_ok {
//         Err("cargo build failed")?;
//     }
//     Ok(())
// }

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
