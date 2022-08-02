use std::collections::BTreeSet;

use cargo::core::PackageId;
use cargo::core::Resolve;
use itertools::Itertools;

pub trait ResolveExt {
    fn recursive_deps_including_self(&self, root_package: PackageId) -> BTreeSet<PackageId>;
}
impl ResolveExt for Resolve {
    fn recursive_deps_including_self(&self, root_package: PackageId) -> BTreeSet<PackageId> {
        // FIXME: where the hell is `autocfg` coming from?
        let mut deps: BTreeSet<PackageId> = Default::default();

        deps.insert(root_package);

        // recusive deps
        loop {
            let layer = deps
                .iter()
                .map(|id| self.deps(*id))
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
