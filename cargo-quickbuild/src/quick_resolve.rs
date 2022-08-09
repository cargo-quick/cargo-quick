use std::collections::BTreeSet;
use std::collections::HashMap;

use anyhow::Result;
use cargo::core::compiler::RustcTargetData;

use cargo::core::dependency::DepKind;
use cargo::core::Package;
use cargo::core::{PackageId, Workspace};
use cargo::ops::WorkspaceResolve;
use cargo::ops::{CompileOptions, Packages};

use itertools::Itertools;

use crate::vendor::tree::graph::Graph;
use crate::vendor::tree::{Charset, EdgeKind, Prefix, Target, TreeOptions};

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
    pub fn recursive_deps_including_self(&self, package_id: PackageId) -> BTreeSet<PackageId> {
        let kinds = &[DepKind::Normal, DepKind::Build];

        let mut deps = self._recursive_deps(package_id, kinds);
        deps.insert(package_id);
        deps
    }

    pub fn recursive_build_deps(&self, package_id: PackageId) -> BTreeSet<PackageId> {
        let kinds = &[DepKind::Build];

        self._recursive_deps(package_id, kinds)
    }

    fn _recursive_deps(&self, package_id: PackageId, kinds: &[DepKind]) -> BTreeSet<PackageId> {
        let mut deps: BTreeSet<PackageId> = Default::default();

        let mut indexes = self.graph.indexes_from_ids(&[package_id]);
        loop {
            let layer = {
                let mut layer: BTreeSet<PackageId> = Default::default();
                for node_index in indexes {
                    for kind in kinds {
                        let deps = self
                            .graph
                            .connected_nodes(node_index, &EdgeKind::Dep(*kind));
                        for idx in deps {
                            layer.insert(self.graph.package_id_for_index(idx));
                        }
                    }
                }
                layer = layer.difference(&deps).copied().collect();
                layer
            };
            if layer.is_empty() {
                break;
            }
            indexes = self
                .graph
                .indexes_from_ids(&layer.iter().copied().collect_vec());
            deps.extend(layer);
        }
        deps
    }
}

pub fn create_quick_resolve<'cfg, 'a>(
    ws: &'a Workspace<'cfg>,
    options: &CompileOptions,
    workspace_resolve: &'a cargo::ops::WorkspaceResolve<'cfg>,
) -> Result<QuickResolve<'cfg, 'a>, anyhow::Error> {
    let requested_kinds = &options.build_config.requested_kinds;
    let target_data = RustcTargetData::new(ws, requested_kinds)?;
    let package_map: HashMap<PackageId, &Package> = workspace_resolve
        .pkg_set
        .packages()
        .map(|pkg| (pkg.package_id(), pkg))
        .collect();
    let packages = clone_packages(&options.spec);
    let opts = TreeOptions {
        cli_features: options.cli_features.clone(),
        packages,
        target: Target::Host,
        edge_kinds: [
            EdgeKind::Dep(DepKind::Normal),
            EdgeKind::Dep(DepKind::Build),
        ]
        .into_iter()
        .collect(),
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
    let graph = crate::vendor::tree::graph::build(
        ws,
        &workspace_resolve.targeted_resolve,
        &workspace_resolve.resolved_features,
        &options.spec.to_package_id_specs(ws)?,
        &options.cli_features,
        &target_data,
        requested_kinds,
        package_map,
        &opts,
    )
    .unwrap();
    let resolve = QuickResolve {
        ws,
        workspace_resolve,
        graph,
    };
    Ok(resolve)
}

fn clone_packages(packages: &Packages) -> Packages {
    match packages {
        Packages::Default => Packages::Default,
        Packages::All => Packages::All,
        Packages::OptOut(vec) => Packages::OptOut(vec.clone()),
        Packages::Packages(vec) => Packages::Packages(vec.clone()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;

    use cargo::core::compiler::{RustcTargetData, UnitInterner};
    use cargo::core::Package;
    use cargo::util::command_prelude::CompileMode;
    use cargo::util::interning::InternedString;
    use cargo::Config;
    use itertools::Itertools;

    use crate::resolve::create_resolve;
    use crate::vendor::tree::{Charset, Prefix, Target, TreeOptions};

    use super::*;

    #[test]
    fn sense_check_recursive_deps() -> anyhow::Result<()> {
        let config = Config::default()?;

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
            packages: clone_packages(&options.spec),
            target: Target::Host,
            edge_kinds: [
                EdgeKind::Dep(DepKind::Normal),
                EdgeKind::Dep(DepKind::Build),
            ]
            .into_iter()
            .collect(),
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
        let graph = crate::vendor::tree::graph::build(
            &ws,
            &workspace_resolve.targeted_resolve,
            &workspace_resolve.resolved_features,
            &options.spec.to_package_id_specs(&ws)?,
            &options.cli_features,
            &target_data,
            requested_kinds,
            package_map,
            &opts,
        )
        .unwrap();
        let resolve = QuickResolve {
            ws: &ws,
            workspace_resolve: &workspace_resolve,
            graph,
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
        // $ cargo tree --no-dedupe --edges=all -p libz-sys
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
            &["cc", "jobserver", "libc", "libz-sys", "pkg-config"]
        );
        // $ cargo tree --no-dedupe --edges=all -p libnghttp2-sys
        // libnghttp2-sys v0.1.7+1.45.0
        // └── libc feature "default"
        //     ├── libc v0.2.125
        //     └── libc feature "std"
        //         └── libc v0.2.125
        // [build-dependencies]
        // └── cc feature "default"
        //     └── cc v1.0.73
        //         └── jobserver feature "default"
        //             └── jobserver v0.1.24
        //                 └── libc feature "default"
        //                     ├── libc v0.2.125
        //                     └── libc feature "std"
        //                         └── libc v0.2.125
        assert_eq!(
            dep_names_for_package(&resolve, "libnghttp2-sys"),
            &["cc", "jobserver", "libc", "libnghttp2-sys"]
        );
        // cargo tree --no-dedupe --edges=all -p curl-sys
        // curl-sys v0.4.55+curl-7.83.1
        // ├── libc feature "default"
        // │   ├── libc v0.2.125
        // │   └── libc feature "std"
        // │       └── libc v0.2.125
        // ├── libnghttp2-sys feature "default"
        // │   └── libnghttp2-sys v0.1.7+1.45.0
        // │       └── libc feature "default"
        // │           ├── libc v0.2.125
        // │           └── libc feature "std"
        // │               └── libc v0.2.125
        // │       [build-dependencies]
        // │       └── cc feature "default"
        // │           └── cc v1.0.73
        // │               └── jobserver feature "default"
        // │                   └── jobserver v0.1.24
        // │                       └── libc feature "default"
        // │                           ├── libc v0.2.125
        // │                           └── libc feature "std"
        // │                               └── libc v0.2.125
        // └── libz-sys feature "libc"
        //     └── libz-sys v1.1.6
        //         └── libc feature "default"
        //             ├── libc v0.2.125
        //             └── libc feature "std"
        //                 └── libc v0.2.125
        //         [build-dependencies]
        //         ├── cc feature "default"
        //         │   └── cc v1.0.73
        //         │       └── jobserver feature "default"
        //         │           └── jobserver v0.1.24
        //         │               └── libc feature "default"
        //         │                   ├── libc v0.2.125
        //         │                   └── libc feature "std"
        //         │                       └── libc v0.2.125
        //         └── pkg-config feature "default"
        //             └── pkg-config v0.3.25
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
            dep_names_for_package(&resolve, "curl-sys"),
            &[
                "cc",
                "curl-sys",
                "jobserver",
                "libc",
                "libnghttp2-sys",
                "libz-sys",
                "pkg-config"
            ]
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
