//! Implementation of `cargo tree`.

use anyhow::Context;
use cargo::core::compiler::{CompileKind, RustcTargetData};
use cargo::core::dependency::DepKind;
use cargo::core::resolver::{features::CliFeatures, ForceAllTargets, HasDevUnits};
use cargo::core::{Package, PackageId, PackageIdSpec, Workspace};
use cargo::ops::{self, Packages};
use cargo::util::{CargoResult, Config};
use cargo::{drop_print, drop_println};
use graph::Graph;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

pub mod graph;

pub use {graph::EdgeKind, graph::Node};

pub struct TreeOptions {
    pub cli_features: CliFeatures,
    /// The packages to display the tree for.
    pub packages: Packages,
    /// The platform to filter for.
    pub target: Target,
    /// The dependency kinds to display.
    pub edge_kinds: HashSet<EdgeKind>,
    pub invert: Vec<String>,
    /// The packages to prune from the display of the dependency tree.
    pub pkgs_to_prune: Vec<String>,
    /// The style of prefix for each line.
    pub prefix: Prefix,
    /// If `true`, duplicates will be repeated.
    /// If `false`, duplicates will be marked with `*`, and their dependencies
    /// won't be shown.
    pub no_dedupe: bool,
    /// If `true`, run in a special mode where it will scan for packages that
    /// appear with different versions, and report if any where found. Implies
    /// `invert`.
    pub duplicates: bool,
    /// The style of characters to use.
    pub charset: Charset,
    /// A format string indicating how each package should be displayed.
    pub format: String,
    /// Includes features in the tree as separate nodes.
    pub graph_features: bool,
    /// Maximum display depth of the dependency tree.
    pub max_display_depth: u32,
    /// Exculdes proc-macro dependencies.
    pub no_proc_macro: bool,
}

#[derive(PartialEq)]
pub enum Target {
    Host,
    Specific(Vec<String>),
    All,
}

impl Target {
    pub fn from_cli(targets: Vec<String>) -> Target {
        match targets.len() {
            0 => Target::Host,
            1 if targets[0] == "all" => Target::All,
            _ => Target::Specific(targets),
        }
    }
}

pub enum Charset {
    Utf8,
    Ascii,
}

impl FromStr for Charset {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Charset, &'static str> {
        match s {
            "utf8" => Ok(Charset::Utf8),
            "ascii" => Ok(Charset::Ascii),
            _ => Err("invalid charset"),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Prefix {
    None,
    Indent,
    Depth,
}

impl FromStr for Prefix {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Prefix, &'static str> {
        match s {
            "none" => Ok(Prefix::None),
            "indent" => Ok(Prefix::Indent),
            "depth" => Ok(Prefix::Depth),
            _ => Err("invalid prefix"),
        }
    }
}
