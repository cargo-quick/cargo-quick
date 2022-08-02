use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use cargo::core::compiler::unit_dependencies::build_unit_dependencies;
use cargo::core::compiler::unit_graph::{self, UnitDep, UnitGraph};
use cargo::core::compiler::{standard_lib, TargetInfo};
use cargo::core::compiler::{BuildConfig, BuildContext, Compilation, Context};
use cargo::core::compiler::{CompileKind, CompileMode, CompileTarget, RustcTargetData, Unit};
use cargo::core::compiler::{DefaultExecutor, Executor, UnitInterner};
use cargo::core::profiles::{Profiles, UnitFor};
use cargo::core::resolver::features::{self, CliFeatures, FeaturesFor, ForceAllTargets};
use cargo::core::resolver::{HasDevUnits, Resolve};
use cargo::core::{FeatureValue, Package, PackageSet, Shell, Summary, Target};
use cargo::core::{PackageId, PackageIdSpec, SourceId, TargetKind, Workspace};
use cargo::drop_println;
use cargo::ops::WorkspaceResolve;
use cargo::ops::{self, CompileOptions, Packages};
use cargo::util::config::Config;
use cargo::util::interning::InternedString;
use cargo::util::restricted_names::is_glob_pattern;
use cargo::util::{closest_msg, profile, CargoResult, StableHasher};

/// copy pasta from the top half of create_bcx(), from cargo-0.61.1/src/cargo/ops/cargo_compile.rs
pub fn create_resolve<'a, 'cfg>(
    ws: &'a Workspace<'cfg>,
    options: &'a CompileOptions,
    interner: &'a UnitInterner,
) -> CargoResult<WorkspaceResolve<'cfg>> {
    let CompileOptions {
        ref build_config,
        ref spec,
        ref cli_features,
        ref filter,
        ref target_rustdoc_args,
        ref target_rustc_args,
        ref target_rustc_crate_types,
        ref local_rustdoc_args,
        rustdoc_document_private_items,
        honor_rust_version,
    } = *options;
    let config = ws.config();

    // Perform some pre-flight validation.
    match build_config.mode {
        CompileMode::Test
        | CompileMode::Build
        | CompileMode::Check { .. }
        | CompileMode::Bench
        | CompileMode::RunCustomBuild => {
            if std::env::var("RUST_FLAGS").is_ok() {
                config.shell().warn(
                    "Cargo does not read `RUST_FLAGS` environment variable. Did you mean `RUSTFLAGS`?",
                )?;
            }
        }
        CompileMode::Doc { .. } | CompileMode::Doctest | CompileMode::Docscrape => {
            if std::env::var("RUSTDOC_FLAGS").is_ok() {
                config.shell().warn(
                    "Cargo does not read `RUSTDOC_FLAGS` environment variable. Did you mean `RUSTDOCFLAGS`?"
                )?;
            }
        }
    }
    config.validate_term_config()?;

    let target_data = RustcTargetData::new(ws, &build_config.requested_kinds)?;

    let all_packages = &Packages::All;
    let rustdoc_scrape_examples = &config.cli_unstable().rustdoc_scrape_examples;
    let need_reverse_dependencies = rustdoc_scrape_examples.is_some();
    let full_specs = if need_reverse_dependencies {
        all_packages
    } else {
        spec
    };

    let resolve_specs = full_specs.to_package_id_specs(ws)?;
    let has_dev_units = if filter.need_dev_deps(build_config.mode) || need_reverse_dependencies {
        HasDevUnits::Yes
    } else {
        HasDevUnits::No
    };
    let resolve = ops::resolve_ws_with_opts(
        ws,
        &target_data,
        &build_config.requested_kinds,
        cli_features,
        &resolve_specs,
        has_dev_units,
        ForceAllTargets::No,
    )?;

    Ok(resolve)
}
