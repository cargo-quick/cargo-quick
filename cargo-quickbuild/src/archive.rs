use std::collections::BTreeMap;
use std::error::Error;
use std::fs::File;
use std::path::Path;

use cargo::core::compiler::unit_graph::UnitDep;
use cargo::core::compiler::Unit;
use tar::{Archive, Builder};

use super::get_tarball_path;

pub fn tar_target_dir(
    scratch_dir: std::path::PathBuf,
    temp_tarball_path: &std::path::PathBuf,
) -> Result<(), Box<dyn Error>> {
    // FIXME: each tarball contains duplicates of all of the dependencies that we just unpacked already
    // Either inline whatever append_dir_all() is doing and add filtering, or delete files before making the tarball
    let mut tar = Builder::new(
        File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(temp_tarball_path)?,
    );
    tar.append_dir_all("target", scratch_dir.join("target"))?;
    tar.finish()?;

    Ok(())
}

pub(crate) fn untar_target_dir(
    computed_deps: &BTreeMap<&Unit, &Vec<UnitDep>>,
    unit: &Unit,
    scratch_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    let tarball_path = get_tarball_path(computed_deps, unit);
    assert!(tarball_path.exists(), "{tarball_path:?} does not exist");
    println!("unpacking {tarball_path:?}");
    // FIXME: return BTreeMap<PathBuf, DateTime> or something, by unpacking what Archive::_unpack() does internally
    Archive::new(File::open(tarball_path)?).unpack(scratch_dir)?;

    Ok(())
}
