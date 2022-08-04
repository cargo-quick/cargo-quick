use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;
use std::path::PathBuf;

use crypto_hash::hex_digest;
use crypto_hash::Algorithm;

use cargo::core::PackageId;

use crate::quick_resolve::QuickResolve;

pub(crate) fn get_tarball_path<'cfg, 'a>(
    resolve: &QuickResolve<'cfg, 'a>,
    tarball_dir: &Path,
    package_id: PackageId,
) -> PathBuf {
    let deps_string = packages_to_cargo_toml_deps(resolve, package_id);

    let digest = hex_digest(Algorithm::SHA256, deps_string.as_bytes());
    let package_name = package_id.name();
    let package_version = package_id.version();

    std::fs::create_dir_all(&tarball_dir).unwrap();

    tarball_dir.join(format!("{package_name}-{package_version}-{digest}.tar"))
}

pub(crate) fn packages_to_cargo_toml_deps<'cfg>(
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

pub(crate) fn deps_to_string(resolve: &QuickResolve, deps: BTreeSet<PackageId>) -> String {
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
