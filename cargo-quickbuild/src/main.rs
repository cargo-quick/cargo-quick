use cargo_lock::{
    dependency::graph::{Graph, NodeIndex},
    Error, Lockfile, Package,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

fn get_dependencies_including_self(graph: &Graph, node_index: &NodeIndex) -> BTreeSet<Package> {
    let mut deps = BTreeSet::new();
    deps.insert(graph[*node_index].clone());
    let ns = graph.neighbors(*node_index);
    for n in ns {
        let sub_neighbours = get_dependencies_including_self(graph, &n);
        deps.extend(sub_neighbours);
    }
    deps
}

fn hash_packages(packages: &BTreeSet<Package>) -> String {
    let mut hasher = Sha256::new();
    let debugged = format!("{:?}", packages);
    hasher.update(debugged);
    format!("{:x}", hasher.finalize())
}

fn main() -> Result<(), Error> {
    let lockfile = Lockfile::load("Cargo.lock").unwrap();
    // FIXME: if lockfile.metadata or lockfile.patch contain anything
    // interesting then explode.
    let tree = lockfile.dependency_tree()?;
    let graph = tree.graph();

    for node in tree.nodes().iter() {
        let (dependency, node_index) = node;
        let deps = get_dependencies_including_self(&graph, &node_index);
        let hash = hash_packages(&deps);

        println!("{}-{}", dependency.name.as_str(), hash);
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_get_dependencies_including_self() {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("../Cargo.lock");
        let lockfile = Lockfile::load(d).unwrap();
        let tree = lockfile.dependency_tree().unwrap();
        let graph = tree.graph();
        let (_dep, node_index) = tree
            .nodes()
            .iter()
            .find(|(dep, _node_index)| dep.name.as_str() == "serde")
            .unwrap();

        let packages = get_dependencies_including_self(&graph, &node_index);

        let package_names = vec![
            "proc-macro2",
            "quote",
            "serde",
            "serde_derive",
            "syn",
            "unicode-xid",
        ];

        assert_eq!(
            packages.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
            package_names
        );
    }

    #[test]
    fn test_hash_packages() {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("../Cargo.lock");
        let lockfile = Lockfile::load(d).unwrap();
        let tree = lockfile.dependency_tree().unwrap();
        let graph = tree.graph();
        let (_dep, node_index) = tree
            .nodes()
            .iter()
            .find(|(dep, _node_index)| dep.name.as_str() == "serde")
            .unwrap();

        let packages = get_dependencies_including_self(&graph, &node_index);

        assert_eq!(
            hash_packages(&packages),
            "49a34557c50d642266068e73fce9fade25b1238a484ac2bdf60e30506da1f267"
        );
    }
}
