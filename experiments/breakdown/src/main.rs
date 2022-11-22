use cargo_lock::{
    dependency::graph::{Graph, NodeIndex},
    Lockfile, Package,
};
use petgraph::visit::Walker;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, error::Error, fmt::Debug, fs::File, path::Path};

// FIXME: this is copy-pasted from fetch/main.rs. Maybe make a shared `interfaces` crate?
#[derive(Debug, Deserialize)]
struct RepoRecord {
    name: String,
    has_cargo_lock: bool,
}

#[derive(Debug, serde::Serialize)]
struct SubtreeRecord<'a> {
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

fn write_all(
    writer: &mut csv::Writer<File>,
    rust_repos_dir: &str,
    repo_name: &str,
) -> Result<(), Box<dyn Error>> {
    let path = format!("{}/data/locks/{}/Cargo.lock", rust_repos_dir, repo_name);
    let path = Path::new(&path);
    if !path.exists() {
        return Ok(());
    };
    let lockfile = Lockfile::load(path)?;
    // FIXME: if lockfile.metadata or lockfile.patch contain anything
    // interesting then explode.
    let tree = lockfile.dependency_tree()?;
    let graph = tree.graph();

    // TODO:
    // * (optional) stop using tree.nodes() here, and use graph[node_index] to get the dependency
    // * find tokio in the dependency tree
    // * walk only from tokio downwards
    for (_, node_index) in tree
        .nodes()
        .iter()
        .filter(|(dep, _)| dep.name.as_str() == "tokio")
    {
        for neighbor_index in graph.neighbors(*node_index) {
            let deps = get_dependencies_including_self(graph, &neighbor_index);
            let hash = hash_packages(&deps);

            writer.serialize(SubtreeRecord {
                repo_path: repo_name,
                hash: &hash,
                package_name: graph[neighbor_index].name.as_str(),
                package_version: &graph[neighbor_index].version.to_string(),
                deps_count: deps.len(),
            })?;
        }
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
    let mut progress = 0;
    let rust_repos_dir = get_first_arg()?;
    let rust_repos_dir = rust_repos_dir.to_str().unwrap();

    let repo_list_csv_path = std::path::Path::new(&rust_repos_dir).join("data/github.csv");
    let output_csv_filename = format!("{}/data/subtrees.csv", rust_repos_dir);

    let csv_file = File::open(repo_list_csv_path)?;
    let mut csv_reader = csv::Reader::from_reader(csv_file);
    let repo_records = csv_reader.deserialize::<RepoRecord>();

    File::create(dbg!(&output_csv_filename))?;
    let mut writer = csv::Writer::from_path(output_csv_filename).unwrap();

    for repo_record in repo_records {
        let repo_record = repo_record?;
        if !repo_record.has_cargo_lock {
            continue;
        }

        track_progress(&mut progress, &repo_record.name);

        write_all(&mut writer, rust_repos_dir, &repo_record.name)
            .unwrap_or_else(|error| eprintln!("Error in {:?}: {:#?}", repo_record.name, error));
    }

    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;

    fn get_graph() -> Graph {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("../../Cargo.lock");
        let lockfile = Lockfile::load(path).unwrap();
        let tree = lockfile.dependency_tree().unwrap();
        tree.graph().clone()
    }

    fn get_package_index(graph: &Graph, dependency_name: &str) -> NodeIndex {
        graph
            .node_indices()
            .find(|node_index| graph[*node_index].name.as_str() == dependency_name)
            .unwrap()
    }

    #[test]
    fn test_get_dependencies_including_self() {
        let graph = get_graph();
        let node_index = get_package_index(&graph, "serde");
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
        let graph = get_graph();
        let node_index = get_package_index(&graph, "serde");

        let packages = get_dependencies_including_self(&graph, &node_index);

        // FIXME: don't rely on debug representation of Package for the hash.
        // I'm not convinced that it's stable.
        assert_eq!(
            hash_packages(&packages),
            "f090d4356ee809ef476e7f612352ba06bf6b9a41f88df20f67ad14cdc67b7ada"
        );
    }

    #[test]
    fn hash_packages_gives_different_values_for_leaf_nodes() {
        let graph = get_graph();
        let version_check_node_index = get_package_index(&graph, "version_check");
        let hashbrown_node_index = get_package_index(&graph, "hashbrown");

        let version_check_packages =
            get_dependencies_including_self(&graph, &version_check_node_index);
        let hashbrown_packages = get_dependencies_including_self(&graph, &hashbrown_node_index);

        assert_ne!(
            hash_packages(&version_check_packages),
            hash_packages(&hashbrown_packages),
        );
    }
}
