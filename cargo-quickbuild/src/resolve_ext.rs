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
                .filter(|(dep_id, _)| {
                    // FIXME: this feels lossy.
                    // * HostDep is documented as being for proc macros only. By doing this, I think I am emulating the v1 resolver behaviour.
                    // * I am only passing in package name, but there may be multiple versions of the package in my tree.
                    self.resolved_features.is_dep_activated(
                        root_package,
                        FeaturesFor::NormalOrDev,
                        dep_id.name(),
                    ) || self.resolved_features.is_dep_activated(
                        root_package,
                        FeaturesFor::HostDep,
                        dep_id.name(),
                    )
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
