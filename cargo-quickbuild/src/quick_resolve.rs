use std::collections::BTreeSet;

use cargo::core::resolver::features::FeaturesFor;
use cargo::core::PackageId;
use cargo::ops::WorkspaceResolve;
use itertools::Itertools;

/// A wrapper around the cargo core resolve machinery, to make cargo-quickbuild work.
/// Probably won't be all that quick ;-)
pub struct QuickResolve<'cfg> {
    pub workspace_resolve: WorkspaceResolve<'cfg>,
}

impl<'cfg> QuickResolve<'cfg> {
    pub fn new(workspace_resolve: WorkspaceResolve<'cfg>) -> Self {
        Self { workspace_resolve }
    }
    pub fn recursive_deps_including_self(&self, root_package: PackageId) -> BTreeSet<PackageId> {
        // FIXME: where the hell is `autocfg` coming from?
        let mut deps: BTreeSet<PackageId> = Default::default();

        deps.insert(root_package);

        // recusive deps
        loop {
            let layer = deps
                .iter()
                .map(|id| {
                    self.workspace_resolve.targeted_resolve.deps(*id).filter(
                        move |(_dep_id, deps)| {
                            // FIXME: this feels lossy.
                            // * HostDep is documented as being for proc macros only. By doing this, I think I am emulating the v1 resolver behaviour.
                            // * I am only passing in package name, but there may be multiple versions of the package in my tree.
                            // * I don't think it's valid to use root package. I think I need the parent package in each case.

                            deps.iter().any(|dep| {
                                if dep.name_in_toml() == "libc" {
                                    dbg!((id, _dep_id, dep));
                                };
                                // FIXME: vcpkg is
                                // ```toml
                                // [target.'cfg(target_env = "msvc")'.build-dependencies]
                                // vcpkg = "0.2"
                                // ```
                                // The `vcpkg` dep has platform `cfg(target_env = "msvc")`, and `libc` has platform `cfg(unix)`
                                // self.resolved_features doesn't seem to be picking up that we're building for unix or something?
                                // How does cargo tree do it?
                                self.workspace_resolve.resolved_features.is_dep_activated(
                                    *id,
                                    FeaturesFor::NormalOrDev,
                                    dep.name_in_toml(),
                                ) || self.workspace_resolve.resolved_features.is_dep_activated(
                                    *id,
                                    FeaturesFor::HostDep,
                                    dep.name_in_toml(),
                                )
                            })
                        },
                    )
                })
                .flatten()
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
        let resolve = QuickResolve::new(create_resolve(&ws, &options, &interner)?);

        assert_eq!(dep_names_for_package(&resolve, "libc"), &["libc"]);
        // $ cargo tree --no-dedupe --edges=all -p jobserver
        // jobserver v0.1.24
        // └── libc feature "default"
        //     ├── libc v0.2.125
        //     └── libc feature "std"
        //         └── libc v0.2.125
        assert_eq!(
            dep_names_for_package(&resolve, "jobserver"),
            &["jobserver", "libc"]
        );
        assert_eq!(
            dep_names_for_package(&resolve, "cc"),
            &["cc", "jobserver", "libc"]
        );
        // $ cargo tree -p libz-sys --no-dedupe --edges=all
        // libz-sys v1.1.6
        // └── libc feature "default"
        //     ├── libc v0.2.125
        //     └── libc feature "std"
        //         └── libc v0.2.125
        // [build-dependencies]
        // ├── cc feature "default"
        // │   └── cc v1.0.73
        // │       └── jobserver feature "default"
        // │           └── jobserver v0.1.24
        // │               └── libc feature "default"
        // │                   ├── libc v0.2.125
        // │                   └── libc feature "std"
        // │                       └── libc v0.2.125
        // └── pkg-config feature "default"
        //     └── pkg-config v0.3.25
        assert_eq!(
            dep_names_for_package(&resolve, "libz-sys"),
            // FIXME: where the hell is `jobserver`, and where are "pkg-config", "vcpkg" coming from?
            &["cc", "jobserver", "libc", "libz-sys", "pkg-config"]
        );
        assert_eq!(
            dep_names_for_package(&resolve, "libnghttp2-sys"),
            // FIXME: where the hell is `jobserver`?
            &["cc", "libc", "libnghttp2-sys"]
        );

        Ok(())
    }

    fn package_by_name(resolve: &QuickResolve, name: &str) -> PackageId {
        let [root_package]: [_; 1] = resolve
            .workspace_resolve
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

    fn dep_names_for_package(resolve: &QuickResolve, name: &str) -> Vec<InternedString> {
        package_names(&resolve.recursive_deps_including_self(package_by_name(resolve, name)))
    }
}
