//! Code for building the graph used by `cargo tree`.

use itertools::Itertools;

use super::TreeOptions;
use cargo::core::compiler::{BuildContext, CompileKind, Unit};
use cargo::core::dependency::DepKind;
use cargo::core::resolver::features::CliFeatures;
use cargo::core::resolver::Resolve;
use cargo::core::{FeatureMap, FeatureValue, Package, PackageId, PackageIdSpec};
use cargo::util::interning::InternedString;
use cargo::util::CargoResult;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Node {
    Package {
        package_id: PackageId,
        /// Features that are enabled on this package.
        features: Vec<InternedString>,
        kind: CompileKind,
    },
    Feature {
        /// Index of the package node this feature is for.
        node_index: usize,
        /// Name of the feature.
        name: InternedString,
    },
}

impl Node {
    /// Make a Node representing the Node::Package of a Unit.
    ///
    /// Note: There is a many:1 relationship between Unit and Package,
    /// because some units are build.rs builds or build.rs invocations.
    fn package_for_unit(unit: &Unit) -> Node {
        Node::Package {
            package_id: unit.pkg.package_id(),
            features: unit.features.clone(),
            kind: unit.kind,
        }
    }
}

/// The kind of edge, for separating dependencies into different sections.
#[derive(Debug, Copy, Hash, Eq, Clone, PartialEq)]
pub enum EdgeKind {
    Dep(DepKind),
    Feature,
}

/// Set of outgoing edges for a single node.
///
/// Edges are separated by the edge kind (`DepKind` or `Feature`). This is
/// primarily done so that the output can easily display separate sections
/// like `[build-dependencies]`.
///
/// The value is a `Vec` because each edge kind can have multiple outgoing
/// edges. For example, package "foo" can have multiple normal dependencies.
#[derive(Clone)]
struct Edges(HashMap<EdgeKind, Vec<usize>>);

impl Edges {
    fn new() -> Edges {
        Edges(HashMap::new())
    }

    /// Adds an edge pointing to the given node. This is idempotent.
    fn add_edge(&mut self, kind: EdgeKind, index: usize) {
        let indexes = self.0.entry(kind).or_default();
        if !indexes.contains(&index) {
            indexes.push(index)
        }
    }
}

/// A graph of dependencies.
pub struct Graph<'a> {
    nodes: Vec<Node>,
    /// The indexes of `edges` correspond to the `nodes`. That is, `edges[0]`
    /// is the set of outgoing edges for `nodes[0]`. They should always be in
    /// sync.
    edges: Vec<Edges>,
    /// Index maps a node to an index, for fast lookup.
    index: HashMap<Node, usize>,
    /// Map for looking up packages.
    package_map: HashMap<PackageId, &'a Package>,
    /// Set of indexes of feature nodes that were added via the command-line.
    ///
    /// For example `--features foo` will mark the "foo" node here.
    cli_features: HashSet<usize>,
    /// Map of dependency names, used for building internal feature map for
    /// dep_name/feat_name syntax.
    ///
    /// Key is the index of a package node, value is a map of dep_name to a
    /// set of `(pkg_node_index, is_optional)`.
    dep_name_map: HashMap<usize, HashMap<InternedString, HashSet<(usize, bool)>>>,
}

impl<'a> Graph<'a> {
    fn new(package_map: HashMap<PackageId, &'a Package>) -> Graph<'a> {
        Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
            index: HashMap::new(),
            package_map,
            cli_features: HashSet::new(),
            dep_name_map: HashMap::new(),
        }
    }

    /// Adds a new node to the graph if it doesn't exist, returning its index.
    fn add_node_idempotently(&mut self, node: Node) -> usize {
        if let Some(index) = self.index.get(&node) {
            *index
        } else {
            self.add_node(node)
        }
    }

    /// Adds a new node to the graph, returning its new index.
    fn add_node(&mut self, node: Node) -> usize {
        let from_index = self.nodes.len();
        self.nodes.push(node);
        self.edges.push(Edges::new());
        self.index
            .insert(self.nodes[from_index].clone(), from_index);
        from_index
    }

    /// Returns a list of nodes the given node index points to for the given kind.
    pub fn connected_nodes(&self, from: usize, kind: &EdgeKind) -> Vec<usize> {
        match self.edges[from].0.get(kind) {
            Some(indexes) => {
                // Created a sorted list for consistent output.
                let mut indexes = indexes.clone();
                indexes.sort_unstable_by(|a, b| self.nodes[*a].cmp(&self.nodes[*b]));
                indexes
            }
            None => Vec::new(),
        }
    }

    /// Given a slice of PackageIds, returns the indexes of all nodes that match.
    pub fn indexes_from_ids(&self, package_ids: &[PackageId]) -> Vec<usize> {
        let mut result: Vec<(&Node, usize)> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_i, node)| match node {
                Node::Package { package_id, .. } => package_ids.contains(package_id),
                _ => false,
            })
            .map(|(i, node)| (node, i))
            .collect();
        // Sort for consistent output (the same command should always return
        // the same output). "unstable" since nodes should always be unique.
        result.sort_unstable();
        result.into_iter().map(|(_node, i)| i).collect()
    }

    pub fn package_for_id(&self, id: PackageId) -> &Package {
        self.package_map
            .get(&id)
            .unwrap_or_else(|| panic!("could not find {id:#?} in {:#?}", self.package_map))
    }

    pub fn package_id_for_index(&self, index: usize) -> PackageId {
        match self.nodes[index] {
            Node::Package { package_id, .. } => package_id,
            Node::Feature { .. } => panic!("unexpected feature node"),
        }
    }
}

/// Builds the graph by iterating over the UnitDeps of a BuildContext.
///
/// This is useful for finding bugs in the implementation of `build()`, below.
pub fn from_bcx<'a, 'cfg>(
    bcx: BuildContext<'a, 'cfg>,
    resolve: &Resolve,
    // FIXME: it feels like it would be easy for specs and cli_features to get out-of-sync with
    // what bcx has been configured with. Either make that structurally impossible or add an assert.
    specs: &[PackageIdSpec],
    cli_features: &CliFeatures,
    package_map: HashMap<PackageId, &'a Package>,
    opts: &TreeOptions,
) -> CargoResult<Graph<'a>> {
    let mut graph = Graph::new(package_map);

    // First pass: add all of the nodes for Packages
    for unit in bcx.unit_graph.keys().sorted() {
        let node = Node::package_for_unit(unit);
        // There may be multiple units for the same package if build-scripts are involved.
        graph.add_node_idempotently(node);

        // FIXME: I quite like the idea of adding all of the nodes in the first pass, but adding
        // the full set of features doesn't seem to be possible here. Maybe it's better to think of
        // features as more like a kind of Edge, and do it in the second loop.
        // if opts.graph_features {
        //     for name in unit.features.iter().copied() {
        //         let node = Node::Feature { node_index, name };
        //         graph.add_node_idempotently(node);
        //     }
        // }
    }

    // second pass: add all of the edges (and `Node::Feature`s if that's what's asked for)
    for (unit, deps) in bcx.unit_graph.iter() {
        let node = Node::package_for_unit(unit);
        let from_index = *graph.index.get(&node).unwrap();

        for dep in deps {
            if dep.unit.pkg.package_id() == unit.pkg.package_id() {
                // Probably a build script that's part of the same package. Skip it.
                continue;
            }
            let dep_node = Node::package_for_unit(&dep.unit);
            let dep_index = *graph.index.get(&dep_node).unwrap();

            // FIXME: This is really ugly. It's also quadratic in `deps.len()`, but `deps` is only
            // the direct dependencies of `unit`, so the ugliness is more important.
            // I think I want to `zip(sorted(deps), sorted(resolve.deps(unit)))` and then assert
            // that the ids line up, with nothing left over.
            let mut found = false;
            let mut added = false;
            for (_, dep_set) in resolve
                .deps(unit.pkg.package_id())
                .filter(|(dep_id, _dep_set)| dep_id == &dep.unit.pkg.package_id())
            {
                found = true;
                assert!(
                    !dep_set.is_empty(),
                    "resolver should be able to tell us why {unit:?} depends on {dep:?}"
                );

                // FIXME: think of better names for dep and link
                // (most code needs to have `dep` renamed to `link` when copy-pasting)
                for link in dep_set {
                    if opts.graph_features {
                        if link.uses_default_features() {
                            add_feature(
                                &mut graph,
                                InternedString::new("default"),
                                Some(from_index),
                                dep_index,
                                EdgeKind::Dep(link.kind()),
                            );
                            added = true;
                        }
                        for feature in link.features() {
                            // FIXME: is add_feature() idempotent?
                            add_feature(
                                &mut graph,
                                *feature,
                                Some(from_index),
                                dep_index,
                                EdgeKind::Dep(link.kind()),
                            );
                            added = true;
                        }
                        // FIXME: do this in its own pass or something?
                        graph
                            .dep_name_map
                            .entry(from_index)
                            .or_default()
                            .entry(link.name_in_toml())
                            .or_default()
                            .insert((dep_index, link.is_optional()));
                    } else {
                        let kind = EdgeKind::Dep(link.kind());
                        if opts.edge_kinds.contains(&kind) {
                            // FIXME: if it's not possible to get from unit to dep with this kind
                            // of edge then maybe we shouldn't add it? Maybe this would help with
                            // the tree::host_dep_feature test? Not sure how to determine this though.
                            graph.edges[from_index].add_edge(kind, dep_index);
                        }
                    }
                }
            }

            assert!(
                found,
                "resolver should have a record of {unit:?} depending on {dep:?}"
            );
            if opts.graph_features && !added {
                // HACK: if dep was added with default-features = false and no other features then
                // it won't be linked up yet. Fudge a direct link in there so that we can represent
                // it on the graph.
                graph.edges[from_index].add_edge(EdgeKind::Dep(DepKind::Normal), dep_index);
            }
        }
    }

    if opts.graph_features {
        let mut members_with_features = bcx.ws.members_with_features(specs, cli_features)?;
        members_with_features.sort_unstable_by_key(|e| e.0.package_id());
        for (member, cli_features) in members_with_features {
            // This package might be built for both host and target.
            let member_indexes = graph.indexes_from_ids(&[member.package_id()]);
            assert!(!member_indexes.is_empty());

            // FIXME: if the package shows up in both host and `target`, it may be possible for the
            // features to be different (this may not even be possible for workspace members in the
            // current resolver - I've not checked).
            //
            // We might be better off querying the UnitGraph again or something?
            let fmap = resolve.summary(member.package_id()).features();
            for member_index in member_indexes.into_iter() {
                add_cli_features(&mut graph, member_index, &cli_features, fmap);
            }
        }

        add_internal_features(&mut graph, resolve)
    }
    Ok(graph)
}

/// Adds a feature node between two nodes.
///
/// That is, it adds the following:
///
/// ```text
/// from -Edge-> featname -Edge::Feature-> to
/// ```
///
/// Returns a tuple `(missing, index)`.
/// `missing` is true if this feature edge was already added.
/// `index` is the index of the index in the graph of the `Feature` node.
fn add_feature(
    graph: &mut Graph<'_>,
    name: InternedString,
    from: Option<usize>,
    to: usize,
    kind: EdgeKind,
) -> (bool, usize) {
    // `to` *must* point to a package node.
    assert!(matches! {graph.nodes[to], Node::Package{..}});
    let node = Node::Feature {
        node_index: to,
        name,
    };
    let (missing, node_index) = match graph.index.get(&node) {
        Some(idx) => (false, *idx),
        None => (true, graph.add_node(node)),
    };
    if let Some(from) = from {
        graph.edges[from].add_edge(kind, node_index);
    }
    graph.edges[node_index].add_edge(EdgeKind::Feature, to);
    (missing, node_index)
}

/// Adds nodes for features requested on the command-line for the given member.
///
/// Feature nodes are added as "roots" (i.e., they have no "from" index),
/// because they come from the outside world. They usually only appear with
/// `--invert`.
fn add_cli_features(
    graph: &mut Graph<'_>,
    package_index: usize,
    cli_features: &CliFeatures,
    feature_map: &FeatureMap,
) {
    // NOTE: Recursive enabling of features will be handled by
    // add_internal_features.

    // Create a set of feature names requested on the command-line.
    let mut to_add: HashSet<FeatureValue> = HashSet::new();
    if cli_features.all_features {
        to_add.extend(feature_map.keys().map(|feat| FeatureValue::Feature(*feat)));
    }

    if cli_features.uses_default_features {
        to_add.insert(FeatureValue::Feature(InternedString::new("default")));
    }
    to_add.extend(cli_features.features.iter().cloned());

    // Add each feature as a node, and mark as "from command-line" in graph.cli_features.
    for fv in to_add {
        match fv {
            FeatureValue::Feature(feature) => {
                let index = add_feature(graph, feature, None, package_index, EdgeKind::Feature).1;
                graph.cli_features.insert(index);
            }
            // This is enforced by CliFeatures.
            FeatureValue::Dep { .. } => panic!("unexpected cli dep feature {}", fv),
            FeatureValue::DepFeature {
                dep_name,
                dep_feature,
                weak,
            } => {
                let dep_connections = match graph
                    .dep_name_map
                    .get(&package_index)
                    .and_then(|h| h.get(&dep_name))
                {
                    // Clone to deal with immutable borrow of `graph`. :(
                    Some(dep_connections) => dep_connections.clone(),
                    None => {
                        // --features bar?/feat where `bar` is not activated should be ignored.
                        // If this wasn't weak, then this is a bug.
                        if weak {
                            continue;
                        }
                        panic!(
                            "missing dep graph connection for CLI feature `{}` for member {:?}\n\
                             Please file a bug report at https://github.com/rust-lang/cargo/issues",
                            fv,
                            graph.nodes.get(package_index)
                        );
                    }
                };
                for (dep_index, is_optional) in dep_connections {
                    if is_optional {
                        // Activate the optional dep on self.
                        let index =
                            add_feature(graph, dep_name, None, package_index, EdgeKind::Feature).1;
                        graph.cli_features.insert(index);
                    }
                    let index =
                        add_feature(graph, dep_feature, None, dep_index, EdgeKind::Feature).1;
                    graph.cli_features.insert(index);
                }
            }
        }
    }
}

/// Recursively adds connections between features in the `[features]` table
/// for every package.
fn add_internal_features(graph: &mut Graph<'_>, resolve: &Resolve) {
    // Collect features already activated by dependencies or command-line.
    let feature_nodes: Vec<(PackageId, usize, usize, InternedString)> = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(i, node)| match node {
            Node::Package { .. } => None,
            Node::Feature { node_index, name } => {
                let package_id = graph.package_id_for_index(*node_index);
                Some((package_id, *node_index, i, *name))
            }
        })
        .collect();

    for (package_id, package_index, feature_index, feature_name) in feature_nodes {
        add_feature_rec(
            graph,
            resolve,
            feature_name,
            package_id,
            feature_index,
            package_index,
        );
    }
}

/// Recursively add feature nodes for all features enabled by the given feature.
///
/// `from` is the index of the node that enables this feature.
/// `package_index` is the index of the package node for the feature.
fn add_feature_rec(
    graph: &mut Graph<'_>,
    resolve: &Resolve,
    feature_name: InternedString,
    package_id: PackageId,
    from: usize,
    package_index: usize,
) {
    let feature_map = resolve.summary(package_id).features();
    let fvs = match feature_map.get(&feature_name) {
        Some(fvs) => fvs,
        None => return,
    };
    for fv in fvs {
        match fv {
            FeatureValue::Feature(dep_name) => {
                let (missing, feat_index) = add_feature(
                    graph,
                    *dep_name,
                    Some(from),
                    package_index,
                    EdgeKind::Feature,
                );
                // Don't recursive if the edge already exists to deal with cycles.
                if missing {
                    add_feature_rec(
                        graph,
                        resolve,
                        *dep_name,
                        package_id,
                        feat_index,
                        package_index,
                    );
                }
            }
            // Dependencies are already shown in the graph as dep edges. I'm
            // uncertain whether or not this might be confusing in some cases
            // (like feature `"somefeat" = ["dep:somedep"]`), so maybe in the
            // future consider explicitly showing this?
            FeatureValue::Dep { .. } => {}
            FeatureValue::DepFeature {
                dep_name,
                dep_feature,
                // Note: `weak` is mostly handled when the graph is built in
                // `is_dep_activated` which is responsible for skipping
                // unactivated weak dependencies. Here it is only used to
                // determine if the feature of the dependency name is
                // activated on self.
                weak,
            } => {
                let dep_indexes = match graph.dep_name_map[&package_index].get(dep_name) {
                    Some(indexes) => indexes.clone(),
                    None => {
                        log::debug!(
                            "enabling feature {} on {}, found {}/{}, \
                             dep appears to not be enabled",
                            feature_name,
                            package_id,
                            dep_name,
                            dep_feature
                        );
                        continue;
                    }
                };
                for (dep_index, is_optional) in dep_indexes {
                    let dep_pkg_id = graph.package_id_for_index(dep_index);
                    if is_optional && !weak {
                        // Activate the optional dep on self.
                        add_feature(
                            graph,
                            *dep_name,
                            Some(from),
                            package_index,
                            EdgeKind::Feature,
                        );
                    }
                    let (missing, feat_index) = add_feature(
                        graph,
                        *dep_feature,
                        Some(from),
                        dep_index,
                        EdgeKind::Feature,
                    );
                    if missing {
                        add_feature_rec(
                            graph,
                            resolve,
                            *dep_feature,
                            dep_pkg_id,
                            feat_index,
                            dep_index,
                        );
                    }
                }
            }
        }
    }
}
