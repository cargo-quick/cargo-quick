use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::collections::HashMap;

use anyhow::Result;
use cargo::core::compiler::RustcTargetData;

use cargo::core::dependency::DepKind;
use cargo::core::resolver::features::FeaturesFor;
use cargo::core::Package;
use cargo::core::{PackageId, Workspace};
use cargo::ops::WorkspaceResolve;
use cargo::ops::{CompileOptions, Packages};

use itertools::Itertools;

use crate::vendor::tree::graph::Graph;
use crate::vendor::tree::{Charset, EdgeKind, Prefix, Target, TreeOptions};

// FIXME: can we use the DepKind enum instead, and write an extension method to convert it to FeaturesFor just-in-time?
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BuildFor(pub FeaturesFor);

impl Eq for BuildFor {}

// Arbitrarily impl Ord so that I can put it in a BTreeMap
impl PartialOrd for BuildFor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(match (self.0, other.0) {
            (FeaturesFor::HostDep, FeaturesFor::HostDep) => Ordering::Equal,
            (FeaturesFor::NormalOrDev, FeaturesFor::NormalOrDev) => Ordering::Equal,
            (FeaturesFor::NormalOrDev, FeaturesFor::HostDep) => Ordering::Less,
            (FeaturesFor::HostDep, FeaturesFor::NormalOrDev) => Ordering::Greater,
        })
    }
}
impl Ord for BuildFor {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

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
    pub fn recursive_deps_including_self(
        &self,
        package_id: PackageId,
        initial_build_for: BuildFor,
    ) -> BTreeSet<(PackageId, BuildFor)> {
        let kinds = &[DepKind::Normal, DepKind::Build];

        let mut deps = self._recursive_deps(package_id, kinds, initial_build_for);
        deps.insert((package_id, initial_build_for));
        deps
    }

    fn _recursive_deps(
        &self,
        initial_package_id: PackageId,
        kinds: &[DepKind],
        initial_build_for: BuildFor,
    ) -> BTreeSet<(PackageId, BuildFor)> {
        let mut deps: BTreeSet<(PackageId, BuildFor)> = Default::default();

        let mut indexes = self
            .graph
            .indexes_from_ids(&[initial_package_id])
            .into_iter()
            .map(|idx| (idx, initial_build_for))
            .collect_vec();
        loop {
            let layer = {
                let mut layer: BTreeSet<(PackageId, BuildFor)> = Default::default();
                for (node_index, build_for) in indexes {
                    for kind in kinds {
                        // TODO: write a test asserting that proc macro crates have no NormalOrDev deps, even if included in a tree that has them.
                        let deps = self
                            .graph
                            .connected_nodes(node_index, &EdgeKind::Dep(*kind));
                        for idx in deps {
                            let package_id = self.graph.package_id_for_index(idx);
                            let new_build_for = match (build_for.0, kind) {
                                (FeaturesFor::NormalOrDev, DepKind::Normal) => {
                                    let package = self.graph.package_for_id(package_id);
                                    if package.proc_macro() {
                                        BuildFor(FeaturesFor::HostDep)
                                    } else {
                                        BuildFor(FeaturesFor::NormalOrDev)
                                    }
                                }
                                (FeaturesFor::NormalOrDev, DepKind::Development) => {
                                    todo!("I don't think we want to support Development dependencies yet");
                                }
                                // build dep links turns all children into build deps
                                (FeaturesFor::NormalOrDev, DepKind::Build) => {
                                    BuildFor(FeaturesFor::HostDep)
                                }
                                // once a HostDep, always a HostDep
                                (FeaturesFor::HostDep, _) => BuildFor(FeaturesFor::HostDep),
                            };
                            layer.insert((package_id, new_build_for));
                        }
                    }
                }
                layer = layer.difference(&deps).copied().collect();
                layer
            };
            if layer.is_empty() {
                break;
            }
            indexes = layer
                .iter()
                .map(|(package_id, build_for)| {
                    self.graph
                        .indexes_from_ids(&[*package_id])
                        .into_iter()
                        .map(|idx| (idx, *build_for))
                        .collect_vec()
                })
                .flatten()
                .collect_vec();
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

        assert_eq!(target_dep_names_for_package(&resolve, "libc"), &["libc"]);
        // $ cargo tree --no-dedupe --edges=all -p jobserver
        // jobserver v0.1.24
        // └── libc feature "default"
        //     ├── libc v0.2.125
        //     └── libc feature "std"
        //         └── libc v0.2.125
        assert_eq!(
            target_dep_names_for_package(&resolve, "jobserver"),
            &["jobserver", "libc"]
        );
        assert_eq!(
            target_dep_names_for_package(&resolve, "cc"),
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
            target_dep_names_for_package(&resolve, "libz-sys"),
            &["libc", "libz-sys"]
        );
        assert_eq!(
            build_dep_names_for_package(&resolve, "libz-sys"),
            &["cc", "jobserver", "libc", "pkg-config"]
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
            target_dep_names_for_package(&resolve, "libnghttp2-sys"),
            &["libc", "libnghttp2-sys"]
        );
        assert_eq!(
            build_dep_names_for_package(&resolve, "libnghttp2-sys"),
            &["cc", "jobserver", "libc"]
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
            target_dep_names_for_package(&resolve, "curl-sys"),
            &["curl-sys", "libc", "libnghttp2-sys", "libz-sys"]
        );
        assert_eq!(
            build_dep_names_for_package(&resolve, "curl-sys"),
            &["cc", "jobserver", "libc", "pkg-config"]
        );

        // vte depends on vte_generate_state_changes which is a proc-macro crate
        // $ cargo tree --no-dedupe --edges=all -p vte
        // vte v0.10.1
        // ├── arrayvec v0.5.2
        // ├── utf8parse feature "default"
        // │   └── utf8parse v0.2.0
        // └── vte_generate_state_changes feature "default"
        //     └── vte_generate_state_changes v0.1.1 (proc-macro)
        //         ├── proc-macro2 feature "default"
        //         │   ├── proc-macro2 v1.0.38
        //         │   │   └── unicode-xid feature "default"
        //         │   │       └── unicode-xid v0.2.3
        //         │   └── proc-macro2 feature "proc-macro"
        //         │       └── proc-macro2 v1.0.38
        //         │           └── unicode-xid feature "default"
        //         │               └── unicode-xid v0.2.3
        //         └── quote feature "default"
        //             ├── quote v1.0.18
        //             │   └── proc-macro2 v1.0.38
        //             │       └── unicode-xid feature "default"
        //             │           └── unicode-xid v0.2.3
        //             └── quote feature "proc-macro"
        //                 ├── quote v1.0.18
        //                 │   └── proc-macro2 v1.0.38
        //                 │       └── unicode-xid feature "default"
        //                 │           └── unicode-xid v0.2.3
        //                 └── proc-macro2 feature "proc-macro"
        //                     └── proc-macro2 v1.0.38
        //                         └── unicode-xid feature "default"
        //                             └── unicode-xid v0.2.3

        assert_eq!(
            target_dep_names_for_package(&resolve, "vte"),
            &["arrayvec", "utf8parse", "vte"]
        );
        // The dep tree of vte_generate_state_changes should count as build deps because it's a proc-macro.
        // TODO: confirm that it doesn't also need to be included as a target dep.
        assert_eq!(
            build_dep_names_for_package(&resolve, "vte"),
            &[
                "proc-macro2",
                "quote",
                "unicode-xid",
                "vte_generate_state_changes"
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

    fn package_names_matching(
        packages_to_build: &BTreeSet<(PackageId, BuildFor)>,
        filter: BuildFor,
    ) -> Vec<InternedString> {
        packages_to_build
            .iter()
            .filter(|(_, build_for)| build_for == &filter)
            .map(|(dep, _)| dep.name())
            // .dedup()
            .collect_vec()
    }

    fn target_dep_names_for_package(resolve: &QuickResolve, name: &str) -> Vec<InternedString> {
        package_names_matching(
            &resolve.recursive_deps_including_self(
                package_by_name(resolve, name),
                BuildFor(FeaturesFor::NormalOrDev),
            ),
            BuildFor(FeaturesFor::NormalOrDev),
        )
    }

    fn build_dep_names_for_package(resolve: &QuickResolve, name: &str) -> Vec<InternedString> {
        package_names_matching(
            &resolve.recursive_deps_including_self(
                package_by_name(resolve, name),
                // It feels wrong that we have to specify this, but I think we have to,
                // because it represents the synthetic "self" dep, and also affects the colour
                // of all child dependencies in the tree.
                BuildFor(FeaturesFor::NormalOrDev),
            ),
            BuildFor(FeaturesFor::HostDep),
        )
    }
}
