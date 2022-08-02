use std::collections::BTreeSet;

use cargo::core::resolver::features::FeaturesFor;
use cargo::core::PackageId;
use cargo::ops::WorkspaceResolve;
use itertools::Itertools;

pub trait ResolveExt {
    fn recursive_deps_including_self(&self, root_package: PackageId) -> BTreeSet<PackageId>;
}
impl<'cfg> ResolveExt for WorkspaceResolve<'cfg> {
    fn recursive_deps_including_self(&self, root_package: PackageId) -> BTreeSet<PackageId> {
        // FIXME: where the hell is `autocfg` coming from?
        let mut deps: BTreeSet<PackageId> = Default::default();

        deps.insert(root_package);

        // recusive deps
        loop {
            let layer = deps
                .iter()
                .map(|id| self.targeted_resolve.deps(*id))
                .flatten()
                .filter(|(_, deps)| {
                    // FIXME: this feels lossy.
                    // * HostDep is documented as being for proc macros only. By doing this, I think I am emulating the v1 resolver behaviour.
                    // * I am only passing in package name, but there may be multiple versions of the package in my tree.
                    // * I don't think it's valid to use root package. I think I need the parent package in each case.

                    deps.iter().any(|dep| {
                        (!dep.is_optional())
                            || self.resolved_features.is_dep_activated(
                                root_package,
                                FeaturesFor::NormalOrDev,
                                dep.name_in_toml(),
                            )
                            || self.resolved_features.is_dep_activated(
                                root_package,
                                FeaturesFor::HostDep,
                                dep.name_in_toml(),
                            )
                    })
                })
                .map(|(id, _)| id)
                .filter(|id| !deps.contains(id))
                .collect_vec();
            // dbg!(&layer);
            if layer.is_empty() {
                break;
            }
            deps.extend(layer)
        }
        deps
    }
}

#[cfg(test)]
mod tests {
    use cargo::util::interning::InternedString;

    use super::super::*;
    use super::*;

    #[test]
    fn sense_check_recursive_deps() -> anyhow::Result<()> {
        let config = Config::default()?;

        // FIXME: compile cargo in release mode
        let ws = Workspace::new(&Path::new("Cargo.toml").canonicalize()?, &config)?;
        let options = CompileOptions::new(&config, CompileMode::Build)?;
        let interner = UnitInterner::new();
        let resolve = create_resolve(&ws, &options, &interner)?;

        assert_eq!(dep_names_for_package(&resolve, "libc"), &["libc"]);
        assert_eq!(
            dep_names_for_package(&resolve, "jobserver"),
            &["jobserver", "libc"]
        );
        assert_eq!(
            dep_names_for_package(&resolve, "cc"),
            &["cc", "jobserver", "libc"]
        );
        assert_eq!(
            dep_names_for_package(&resolve, "libz-sys"),
            // FIXME: where the hell is `jobserver`, and where are "pkg-config", "vcpkg" coming from?
            &["cc", "libc", "libz-sys", "pkg-config", "vcpkg"]
        );
        assert_eq!(
            dep_names_for_package(&resolve, "libnghttp2-sys"),
            // FIXME: where the hell is `jobserver`?
            &["cc", "libc", "libnghttp2-sys"]
        );

        Ok(())
    }

    fn package_by_name(resolve: &WorkspaceResolve, name: &str) -> PackageId {
        let [root_package]: [_; 1] = resolve
            .targeted_resolve
            .iter()
            .filter(|id| id.name() == name)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        root_package
    }

    fn package_names(packages_to_build: &BTreeSet<PackageId>) -> Vec<InternedString> {
        packages_to_build.iter().map(|dep| dep.name()).collect_vec()
    }

    fn dep_names_for_package(resolve: &WorkspaceResolve, name: &str) -> Vec<InternedString> {
        package_names(&resolve.recursive_deps_including_self(package_by_name(resolve, name)))
    }
}
