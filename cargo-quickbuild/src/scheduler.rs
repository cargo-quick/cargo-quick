use std::collections::HashSet;
use std::path::Path;

use anyhow::Result;
use cargo::core::PackageId;

use crate::description::get_tarball_path;
use crate::description::packages_to_cargo_toml_deps;
use crate::quick_resolve::QuickResolve;

pub(crate) fn build_missing_packages(
    resolve: QuickResolve,
    tarball_dir: &Path,
    root_package: PackageId,
) -> Result<(), anyhow::Error> {
    let mut packages_to_build = resolve.recursive_deps_including_self(root_package);

    // dbg!(&root_package);
    // dbg!(&packages_to_build);
    assert!(packages_to_build.contains(&root_package));

    let mut built_packages: HashSet<PackageId> = Default::default();

    for level in 0..=100 {
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
        for package_id in current_level.iter().copied() {
            if package_id == root_package {
                // I suspect that I will also need to gracefully skip workspace packages, or something, for mvp
                assert!(packages_to_build.is_empty());
                assert_eq!(current_level.len(), 1);
                println!("🎉 We're done here 🎉");
                return Ok(());
            }
            build_tarball_if_not_exists(&resolve, tarball_dir, package_id)?;
            built_packages.insert(package_id);
        }
    }

    Ok(())
}

pub(crate) fn outstanding_deps<'cfg, 'a>(
    resolve: &QuickResolve<'cfg, 'a>,
    built_packages: &HashSet<PackageId>,
    package_id: PackageId,
) -> Vec<PackageId> {
    resolve
        .recursive_deps_including_self(package_id)
        .into_iter()
        .filter(|dep| dep != &package_id && !built_packages.contains(dep))
        .collect()
}

pub(crate) fn build_tarball_if_not_exists<'cfg, 'a>(
    resolve: &QuickResolve<'cfg, 'a>,
    tarball_dir: &Path,
    package_id: PackageId,
) -> Result<()> {
    let deps_string = packages_to_cargo_toml_deps(resolve, package_id);

    let tarball_path = get_tarball_path(resolve, tarball_dir, package_id);
    println!("STARTING BUILD\n{tarball_path:?} deps:\n{}", deps_string);
    if tarball_path.exists() {
        println!("{tarball_path:?} already exists");
        return Ok(());
    }
    crate::builder::build_tarball(resolve, tarball_dir, package_id)
}
