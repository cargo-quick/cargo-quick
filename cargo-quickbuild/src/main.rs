use cargo_lock::{
    dependency::graph::{Graph, NodeIndex},
    Error, Lockfile, Package,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

fn main() -> Result<(), Error> {
    let lockfile = Lockfile::load("Cargo.lock").unwrap();
    // FIXME: if lockfile.metadata or lockfile.patch contain anything
    // interesting then explode.
    let tree = lockfile.dependency_tree()?;
    let graph = tree.graph();

    for index in tree.roots().iter() {
        let lockfiles = make_lockfiles(&lockfile, &graph, *index);
        for lockfile in lockfiles {
            // FIXME: find root crate name here
            let root = "subtree";
            let contents = lockfile.to_string();
            let hash = hash(&contents);
            let filename = format!("{root}-{hash}.lock", root = root, hash = hash);
            std::fs::write(filename, contents).expect("Unable to write file");
        }
    }
    Ok(())
}

fn make_lockfiles(lockfile: &Lockfile, graph: &Graph, index: NodeIndex) -> Vec<Lockfile> {
    let mut result = vec![make_subtree_lockfile(lockfile, graph, index)];

    for ix in graph.neighbors(index) {
        result.extend(make_lockfiles(lockfile, graph, ix).into_iter());
    }
    result
}

fn make_subtree_lockfile(lockfile: &Lockfile, graph: &Graph, index: NodeIndex) -> Lockfile {
    let packages = walk_subtree(graph, index);
    let mut set = BTreeSet::new();
    set.extend(packages.into_iter());
    let packages = set.into_iter().collect();

    Lockfile {
        packages,
        ..lockfile.clone()
    }
}

fn walk_subtree(graph: &Graph, index: NodeIndex) -> Vec<Package> {
    let mut result = vec![graph[index].clone()];
    for ix in graph.neighbors(index) {
        result.extend(walk_subtree(graph, ix).into_iter());
    }
    result
}

fn hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    let result = hasher.finalize();
    format!("{:x}", result)
}
