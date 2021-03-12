use std::collections::BTreeSet;

use cargo_lock::{
    dependency::graph::{Graph, NodeIndex},
    Error, Lockfile, Package,
};

fn main() -> Result<(), Error> {
    let lockfile = Lockfile::load("Cargo.lock").unwrap();
    // FIXME: if lockfile.metadata or lockfile.patch contain anything
    // interesting then explode.
    let tree = lockfile.dependency_tree()?;
    let graph = tree.graph();

    for index in tree.roots().iter() {
        let subtree = make_subtree(&lockfile, &graph, *index).unwrap();
        println!("{}", subtree.to_string());
        break;
    }
    Ok(())
}

fn make_subtree(lockfile: &Lockfile, graph: &Graph, index: NodeIndex) -> Result<Lockfile, Error> {
    let packages = walk_subtree(graph, index);
    let mut set = BTreeSet::new();
    set.extend(packages.into_iter());
    let packages = set.into_iter().collect();

    Ok(Lockfile {
        packages,
        ..lockfile.clone()
    })
}

fn walk_subtree(graph: &Graph, index: NodeIndex) -> Vec<Package> {
    let mut result = vec![graph[index].clone()];
    for ix in graph.neighbors(index) {
        result.extend(walk_subtree(graph, ix).into_iter());
    }
    result
}
