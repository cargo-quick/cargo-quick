use cargo_lock::{
    dependency::graph::{Graph, NodeIndex},
    Lockfile, Package,
};
use petgraph::visit::Walker;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, error::Error, fmt::Debug, fs::File, path::Path};

#[derive(Debug, serde::Serialize)]
struct Record<'a> {
    repo_path: &'a str,
    hash: &'a str,
    package_name: &'a str,
    package_version: &'a str,
    deps_count: usize,
}

fn get_dependencies_including_self<'p>(
    graph: &'p Graph,
    node_index: &NodeIndex,
) -> BTreeSet<&'p Package> {
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

fn write_all(writer: &mut csv::Writer<File>, path: &Path) -> Result<(), Box<dyn Error>> {
    let lockfile = Lockfile::load(path)?;
    // FIXME: if lockfile.metadata or lockfile.patch contain anything
    // interesting then explode.
    let tree = lockfile.dependency_tree()?;
    let graph = tree.graph();

    for (dependency, node_index) in tree.nodes().iter() {
        let deps = get_dependencies_including_self(graph, node_index);
        let hash = hash_packages(&deps);

        writer.serialize(Record {
            // FIXME: trim off start and end of path so that it looks like burntushi/ripgresp
            repo_path: path.to_str().unwrap(),
            hash: &hash,
            package_name: dependency.name.as_str(),
            package_version: &dependency.version.to_string(),
            deps_count: deps.len(),
        })?;
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
    let mut progress = 0;
    let csv_filename = format!("{}/data/subtrees.csv", repo_root.to_str().unwrap());

    // TODO: do we need to write the column headings?
    File::create(dbg!(&csv_filename))?;
    let mut writer = csv::Writer::from_path(csv_filename).unwrap();

    for entry in globwalk::glob(glob)? {
        track_progress(&mut progress, &entry);
        let entry = entry?;
        let path = entry.path();
        write_all(&mut writer, path)
            .unwrap_or_else(|error| eprintln!("Error in {:?}: {:#?}", path, error));
    }

    writer.flush()?;
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
            "c51c852fc6dac97c9cc2d2a68db004d49717dec757cf13662e72100347a2d8f7"
        );
    }
}
