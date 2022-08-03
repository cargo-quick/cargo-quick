use std::collections::BTreeSet;

use cargo::core::{PackageId, Workspace};
use cargo::ops::tree::graph::Graph;
use cargo::ops::WorkspaceResolve;

/// A wrapper around the cargo core resolve machinery, to make cargo-quickbuild work.
/// Probably won't be all that quick ;-)
pub struct QuickResolve<'cfg, 'a>
where
    'cfg: 'a,
{
    pub ws: &'a Workspace<'cfg>,
    pub workspace_resolve: &'a WorkspaceResolve<'cfg>,
    pub graph: Graph<'a>,
}

impl<'cfg, 'a> QuickResolve<'cfg, 'a> {
    pub fn new_shim(
        _ws: &'cfg Workspace<'cfg>,
        _workspace_resolve: WorkspaceResolve<'cfg>,
    ) -> Self {
        unimplemented!("just a shim to make main.rs compile while I get the tests working")
    }
    pub fn recursive_deps_including_self(&self, _root_package: PackageId) -> BTreeSet<PackageId> {
        let deps: BTreeSet<PackageId> = Default::default();

        deps
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use cargo::core::compiler::RustcTargetData;
    use cargo::core::Package;
    use cargo::ops::tree::{Charset, Prefix, Target, TreeOptions};
    use cargo::util::interning::InternedString;
    use itertools::Itertools;

    use super::super::*;
    use super::*;

    #[test]
    fn sense_check_recursive_deps() -> anyhow::Result<()> {
        let config = Config::default()?;

        // FIXME: compile cargo in release mode
        let ws = Workspace::new(&Path::new("Cargo.toml").canonicalize()?, &config)?;
        let options = CompileOptions::new(&config, CompileMode::Build)?;

        let interner = UnitInterner::new();
        let workspace_resolve = create_resolve(&ws, &options, &interner)?;
        let requested_kinds = &options.build_config.requested_kinds;
        let target_data = RustcTargetData::new(&ws, requested_kinds)?;
        let package_map: HashMap<PackageId, &Package> = workspace_resolve
            .pkg_set
            .packages()
            .map(|pkg| (pkg.package_id(), pkg))
            .collect();

        let opts = TreeOptions {
            cli_features: options.cli_features.clone(),
            packages: options.spec.clone(),
            target: Target::Host,
            edge_kinds: Default::default(),
            invert: Default::default(),
            pkgs_to_prune: Default::default(),
            prefix: Prefix::None,
            no_dedupe: Default::default(),
            duplicates: Default::default(),
            charset: Charset::Ascii,
            format: Default::default(),
            graph_features: Default::default(),
            max_display_depth: Default::default(),
            no_proc_macro: Default::default(),
        };
        let graph = cargo::ops::tree::graph::build(
            &ws,
            &workspace_resolve.targeted_resolve,
            &workspace_resolve.resolved_features,
            &options.spec.to_package_id_specs(&ws)?,
            &options.cli_features,
            &target_data,
            &requested_kinds,
            package_map,
            &opts,
        )
        .unwrap();
        let resolve = QuickResolve {
            ws: &ws,
            workspace_resolve: &workspace_resolve,
            graph: graph,
        };

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
        drop(resolve);

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
