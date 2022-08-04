use std::collections::BTreeSet;
use std::fmt::Write as _;

use crypto_hash::hex_digest;
use crypto_hash::Algorithm;

use cargo::core::PackageId;

use crate::quick_resolve::QuickResolve;

/// A self-contained description of a package build configuration
pub struct PackageDescription {
    package_id: PackageId,
    cargo_toml_deps: String,
}

impl PackageDescription {
    pub fn new<'cfg>(resolve: &QuickResolve<'cfg, '_>, package_id: PackageId) -> Self {
        let cargo_toml_deps = packages_to_cargo_toml_deps(resolve, package_id);
        Self {
            package_id,
            cargo_toml_deps,
        }
    }
    pub fn pretty_digest(&self) -> String {
        let digest = hex_digest(Algorithm::SHA256, self.cargo_toml_deps.as_bytes());
        let package_name = self.package_id.name();
        let package_version = self.package_id.version();

        format!("{package_name}-{package_version}-{digest}")
    }
}

impl core::fmt::Debug for PackageDescription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackageDescription")
            .field("package_id", &self.package_id)
            .field("pretty_digest", &self.pretty_digest())
            .finish()
    }
}

pub fn packages_to_cargo_toml_deps<'cfg>(
    resolve: &QuickResolve<'cfg, '_>,
    package_id: PackageId,
) -> String {
    let mut deps_string = String::new();
    writeln!(
        deps_string,
        "# {} {}",
        package_id.name(),
        package_id.version()
    )
    .unwrap();
    let deps = resolve.recursive_deps_including_self(package_id);
    let build_deps = resolve.recursive_build_deps(package_id);

    format!(
        "# {name} {version}\n\
        {deps}\n\
        [build-dependencies]\n\
        {build_deps}",
        name = package_id.name(),
        version = package_id.version(),
        deps = deps_to_string(resolve, deps),
        build_deps = deps_to_string(resolve, build_deps)
    )
}

pub fn deps_to_string(resolve: &QuickResolve, deps: BTreeSet<PackageId>) -> String {
    deps.into_iter()
    .map(|package_id| {
        let name = package_id.name();
        let version = package_id.version().to_string();
        let features = resolve.workspace_resolve.targeted_resolve.features(package_id);
        let safe_version = version.replace(|c: char| !c.is_alphanumeric(), "_");
        format!(
            r#"{name}_{safe_version} = {{ package = "{name}", version = "={version}", features = {features:?}, default-features = false }}"#
        ) + "\n"
    }).collect()
}
