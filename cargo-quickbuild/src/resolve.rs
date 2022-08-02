use cargo::core::compiler::UnitInterner;
use cargo::core::compiler::{CompileMode, RustcTargetData};

use cargo::core::resolver::features::ForceAllTargets;
use cargo::core::resolver::HasDevUnits;

use cargo::core::Workspace;

use cargo::ops::WorkspaceResolve;
use cargo::ops::{self, CompileOptions, Packages};

use cargo::util::CargoResult;

/// copy pasta from the top half of create_bcx(), from cargo-0.61.1/src/cargo/ops/cargo_compile.rs
pub fn create_resolve<'a, 'cfg>(
    ws: &'a Workspace<'cfg>,
    options: &'a CompileOptions,
    _interner: &'a UnitInterner,
) -> CargoResult<WorkspaceResolve<'cfg>> {
    let CompileOptions {
        ref build_config,
        ref spec,
        ref cli_features,
        ref filter,
        target_rustdoc_args: _,
        target_rustc_args: _,
        target_rustc_crate_types: _,
        local_rustdoc_args: _,
        rustdoc_document_private_items: _,
        honor_rust_version: _,
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
