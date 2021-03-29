use cargo_lock::{
    dependency::graph::{Graph, NodeIndex},
    Lockfile, Package,
};
use petgraph::visit::{VisitMap, Visitable, Walker};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt::Debug,
    path::Path,
};

fn get_dependencies<'p>(graph: &'p Graph, node_index: &NodeIndex) -> BTreeSet<&'p Package> {
    let dfs = petgraph::visit::Dfs::new(&graph, *node_index);
    let deps: BTreeSet<&Package> = dfs.iter(&graph).map(|i| &graph[i]).collect();

    deps
}

fn hash_packages(packages: &BTreeSet<&Package>) -> String {
    let mut hasher = Sha256::new();
    let debugged = format!("{:?}", packages);
    hasher.update(debugged);
    format!("{:x}", hasher.finalize())
}

fn count_all(counts: &mut BTreeMap<String, u64>, path: &Path) -> Result<(), cargo_lock::Error> {
    let lockfile = Lockfile::load(path).unwrap();
    // FIXME: if lockfile.metadata or lockfile.patch contain anything
    // interesting then explode.
    let tree = lockfile.dependency_tree()?;
    let graph = tree.graph();

    for (dependency, node_index) in tree.nodes().iter() {
        let deps = get_dependencies(graph, node_index);
        let hash = hash_packages(&deps);

        let full_hash = format!("{}-{}-{}", deps.len(), dependency.name.as_str(), hash);
        *counts.entry(full_hash).or_insert(0) += 1;
    }

    Ok(())
}

fn get_first_arg() -> Result<std::ffi::OsString, Box<dyn std::error::Error>> {
    match std::env::args_os().nth(1) {
        None => Err(From::from("expected 1 argument, but got none")),
        Some(file_path) => Ok(file_path),
    }
}

fn track_progress(progress: &mut u64, thing: impl Debug) {
    *progress += 1;
    // Log at every power of 2
    if progress.count_ones() == 1 {
        eprintln!("progress: {} = {:?}", progress, thing);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let repo_root = get_first_arg()?;
    let glob = format!("{}/**/Cargo.lock", repo_root.to_str().unwrap());
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut progress = 0;
    for entry in globwalk::glob(glob)? {
        track_progress(&mut progress, &entry);
        count_all(&mut counts, entry?.path())?;
    }
    let mut items: Vec<(_, _)> = counts.iter().collect();
    items.sort_by(|(_k1, count1), (_k2, count2)| count1.cmp(count2));
    println!("{:#?}", items);
    Ok(())
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_get_dependencies() {
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

        let packages = get_dependencies(&graph, &node_index);

        let package_names = vec!["proc-macro2", "quote", "serde_derive", "syn", "unicode-xid"];

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

        let packages = get_dependencies(&graph, &node_index);

        assert_eq!(
            hash_packages(&packages),
            "c51c852fc6dac97c9cc2d2a68db004d49717dec757cf13662e72100347a2d8f7"
        );
    }
}
