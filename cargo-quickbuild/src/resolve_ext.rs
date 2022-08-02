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
                .inspect(move |(dep_id, dep_set)| {
                    if root_package.name() == "jobserver" && dep_id.name() == "libc" {
                        dbg!(dep_id);
                        dbg!(dep_set);
                    }
                })
                .filter(|(_, dep_set)| {
                    dep_set.iter().all(|dep| {
                        dep.platform()
                            // FIXME: find a way to get platform and cfg settings properly
                            // `libc` is being excluded because we are not passing in `cfg(unix)` here.
                            .map(|platform| platform.matches("aarch64-apple-darwin", &[]))
                            .unwrap_or(true)
                    })
                })
                .inspect(move |(dep_id, dep_set)| {
                    if root_package.name() == "jobserver" {
                        dbg!(dep_id);
                    }
                    assert!(dep_id.name() != "winapi", "{:#?}", (dep_id, dep_set))
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
