use std::collections::BTreeSet;

use anyhow::Result;
use cargo::core::resolver::features::FeaturesFor;
use cargo::core::PackageId;

use crate::builder::build_tarball;
use crate::description::PackageDescription;
use crate::quick_resolve::{BuildFor, QuickResolve};
use crate::repo::Repo;

pub fn build_missing_packages(
    resolve: &QuickResolve,
    repo: &Repo,
    root_package: PackageId,
) -> Result<(), anyhow::Error> {
    let build_for = BuildFor(FeaturesFor::NormalOrDev);

    let mut packages_to_build = resolve.recursive_deps_including_self(root_package, build_for);

    // dbg!(&root_package);
    // dbg!(&packages_to_build);
    assert!(packages_to_build.contains(&(root_package, build_for)));

    let mut built_packages: BTreeSet<(PackageId, BuildFor)> = Default::default();

    // FIXME: I think it might be better to switch this out for a simple depth-first traversal.
    // Mostly because it would reduce my iteration time - fewer packages need to be built before
    // uncovering "level 1" problems.
    for level in 0..=100 {
        println!("START OF LEVEL {level}");
        let current_level;
        (current_level, packages_to_build) =
            packages_to_build
                .iter()
                .partition(|(package_id, build_for)| {
                    outstanding_deps(resolve, &built_packages, *package_id, *build_for).is_empty()
                });

        dbg!(&current_level);

        if current_level.is_empty() && !packages_to_build.is_empty() {
            println!(
                "We haven't compiled everything yet, but there is nothing left to do\n\npackages_to_build: {packages_to_build:#?}"
            );
            dbg!(&built_packages);
            for (package_id, build_for) in packages_to_build {
                dbg!((
                    package_id,
                    outstanding_deps(resolve, &built_packages, package_id, build_for)
                ));
            }
            anyhow::bail!("current_level.is_empty() && !packages_to_build.is_empty()");
        }
        for (package_id, build_for) in current_level.iter().copied() {
            if package_id == root_package {
                // I suspect that I will also need to gracefully skip workspace packages, or something, for mvp
                assert!(packages_to_build.is_empty());
                assert_eq!(current_level.len(), 1);
                println!("🎉 We're done here 🎉");
                return Ok(());
            }
            build_tarball_if_not_exists(resolve, repo, package_id, build_for)?;
            built_packages.insert((package_id, build_for));
        }
    }

    Ok(())
}

pub fn outstanding_deps<'cfg, 'a>(
    resolve: &QuickResolve<'cfg, 'a>,
    built_packages: &BTreeSet<(PackageId, BuildFor)>,
    package_id: PackageId,
    build_for: BuildFor,
) -> Vec<(PackageId, BuildFor)> {
    resolve
        .recursive_deps_including_self(package_id, build_for)
        .into_iter()
        // TODO: check that a package can't be a build-dep for itself
        .filter(|(dep, b_for)| dep != &package_id && !built_packages.contains(&(*dep, *b_for)))
        .collect()
}

pub fn build_tarball_if_not_exists<'cfg, 'a>(
    resolve: &QuickResolve<'cfg, 'a>,
    repo: &Repo,
    package_id: PackageId,
    build_for: BuildFor,
) -> Result<()> {
    let description = PackageDescription::new(resolve, package_id, build_for);
    let package_digest = description.pretty_digest();

    if repo.has(&description) {
        let cargo_toml_deps = description.cargo_toml_deps();
        println!("{package_digest:?} already exists (\n```\n{cargo_toml_deps}\n```\n)");
        return Ok(());
    }
    println!("STARTING BUILD\n{package_digest:?}");
    build_tarball(resolve, repo, package_id, build_for)
}
