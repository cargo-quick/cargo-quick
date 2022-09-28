use cargo::core::resolver::features::FeaturesFor;
use crypto_hash::hex_digest;
use crypto_hash::Algorithm;

use cargo::core::PackageId;

use crate::quick_resolve::BuildFor;
use crate::quick_resolve::QuickResolve;

/// A self-contained description of a package build configuration
pub struct PackageDescription {
    package_id: PackageId,
    cargo_toml_deps: String,
}

impl PackageDescription {
    pub fn new<'cfg>(
        resolve: &QuickResolve<'cfg, '_>,
        package_id: PackageId,
        build_for: BuildFor,
    ) -> Self {
        let cargo_toml_deps = packages_to_cargo_toml_contents(resolve, package_id, build_for);
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
    pub fn cargo_toml_deps(&self) -> &str {
        &self.cargo_toml_deps
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

/// Generate the contents of a Cargo.toml file that can be used for building this package.
fn packages_to_cargo_toml_contents<'cfg>(
    resolve: &QuickResolve<'cfg, '_>,
    package_id: PackageId,
    build_for: BuildFor,
) -> String {
    let name = package_id.name();
    let version = package_id.version();
    let deps = resolve.recursive_deps_including_self(package_id, build_for);
    let target_deps = deps_to_string(
        resolve,
        deps.iter()
            .filter(|(_, build_for)| build_for.0 == FeaturesFor::NormalOrDev)
            .copied(),
    );
    let build_deps = deps_to_string(
        resolve,
        deps.iter()
            .filter(|(_, build_for)| build_for.0 == FeaturesFor::HostDep)
            .copied(),
    );

    format!(
        "# {name} {version}\n\
        \n\
        [package]\n\
        name = \"cargo-quickbuild-scratchpad\"\n\
        version = \"0.1.0\"\n\
        edition = \"2021\"\n\
        \n\
        [dependencies]\n\
        {target_deps}\n\
        \n\
        [build-dependencies]\n\
        {build_deps}\n\
        ",
    )
}

fn deps_to_string(
    resolve: &QuickResolve,
    deps: impl Iterator<Item = (PackageId, BuildFor)>,
) -> String {
    deps
    .map(|(package_id, _build_for)| {
        let name = package_id.name();
        let version = package_id.version().to_string();
        let features = resolve.workspace_resolve.targeted_resolve.features(package_id);
        let safe_version = version.replace(|c: char| !c.is_alphanumeric(), "_");
        format!(
            r#"{name}_{safe_version} = {{ package = "{name}", version = "={version}", features = {features:?}, default-features = false }}"#
        ) + "\n"
    }).collect()
}
