use std::io::Write;
use std::{error::Error, ffi::OsStr, process::Command};

use guppy::{
    graph::{
        cargo::{CargoOptions, CargoResolverVersion, CargoSet},
        feature::{
            FeatureFilter, FeatureGraph, FeatureId, FeatureList, FeatureSet, StandardFeatures,
        },
        DependencyDirection, PackageMetadata,
    },
    CargoMetadata, MetadataCommand,
};
use tempdir::TempDir;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<_> = std::env::args().collect();
    if args[0] == "quickbuild" {
        args.remove(0);
    }
    unpack_or_build_packages()?;
    run_cargo_build(args)?;
    Ok(())
}

struct AllFeatures;
impl<'g> FeatureFilter<'g> for AllFeatures {
    fn accept(&mut self, _: &FeatureGraph<'g>, _: FeatureId<'g>) -> bool {
        true
    }
}
fn unpack_or_build_packages() -> Result<(), Box<dyn Error>> {
    let metadata = MetadataCommand::new().exec()?.build_graph()?;

    let initials = metadata
        .resolve_workspace()
        .to_feature_set(StandardFeatures::Default);
    let features_only = metadata.feature_graph().resolve_none();
    let mut cargo_options = CargoOptions::new();
    cargo_options.set_resolver(CargoResolverVersion::V2);
    let cargo_set = CargoSet::new(initials.clone(), features_only, &cargo_options)
        .expect("cargo resolution should succeed");

    for package in cargo_set
        .host_features()
        // depth first search
        .packages_with_features(DependencyDirection::Reverse)
        .take(8)
    {
        unpack_or_build_subtree(initials.clone(), &cargo_options, package)?;
    }

    Ok(())
}

fn unpack_or_build_subtree(
    initials: FeatureSet,
    cargo_options: &CargoOptions,
    package: FeatureList,
) -> Result<(), Box<dyn Error>> {
    // Notice that we're flipping things about here: make a cargo set from our
    // package downwards, but taking the features from the set of packages in
    // the repo.
    let cargo_set = CargoSet::new(
        package
            .package()
            .to_package_set()
            .to_feature_set(StandardFeatures::None),
        initials.clone(),
        &cargo_options,
    )
    .expect("cargo resolution should succeed");

    let packages: Vec<_> = cargo_set
        .host_features()
        .packages_with_features(DependencyDirection::Reverse)
        .collect();

    if packages.is_empty() {
        println!(
            "skipping {package:?}. Maybe it doesn't get built for the host arch?",
            package = package.package().name()
        );
        return Ok(());
    }

    build_scratch_package(packages)?;

    println!("built {package:?}");
    Ok(())
}

fn build_scratch_package(packages: Vec<FeatureList>) -> Result<(), Box<dyn Error>> {
    let tempdir = TempDir::new("cargo-quickbuild-scratchpad")?;
    let scratch_dir = tempdir.path().join("cargo-quickbuild-scratchpad");
    let init_ok = command(["cargo", "init"])
        .arg(&scratch_dir)
        .status()?
        .success();
    if !init_ok {
        Err("cargo init failed")?;
    }

    let cargo_toml_path = scratch_dir.join("Cargo.toml");
    let mut cargo_toml = std::fs::OpenOptions::new()
        .write(true)
        .append(true)
        .open(&cargo_toml_path)?;

    packages.iter()
    .map(|package| -> std::io::Result<()>{
        let name = package.package().name();
        let version = package.package().version().to_string();
        let features = package.features();
        // FIXME: this will probably break when we have multiple versions  of the same
        // package in the tree. Could we include version.replace('.', '_') or something?
        writeln!(cargo_toml,
            r#"{name} = {{ version = "={version}", features = {features:?}, default-features = false }}"#
        )
    }).collect::<Result<_, std::io::Error>>()?;
    cargo_toml.flush()?;
    drop(cargo_toml);

    command(["cat"]).arg(&cargo_toml_path).status()?;

    let cargo_build_ok = command(["cargo", "build"])
        .current_dir(&scratch_dir)
        .status()?
        .success();

    if !cargo_build_ok {
        Err("cargo build failed")?;
    }
    Ok(())
}

fn run_cargo_build(args: Vec<String>) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new("cargo");
    command.arg("build").args(args);
    println!("would run {command:?}");

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
