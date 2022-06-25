use std::collections::btree_map;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::{collections::BTreeMap, path::PathBuf};

use anyhow::{Context, Result};
use cargo::core::compiler::unit_graph::UnitDep;
use cargo::core::compiler::Unit;
use filetime::FileTime;
use tar::{Archive, Builder, Entry, EntryType};

use super::get_tarball_path;

pub fn tar_target_dir(
    scratch_dir: std::path::PathBuf,
    temp_tarball_path: &std::path::PathBuf,
    _file_timestamps_to_exclude: &BTreeMap<PathBuf, FileTime>,
) -> Result<()> {
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
    tarball_dir: &Path,
    computed_deps: &BTreeMap<&Unit, &Vec<UnitDep>>,
    unit: &Unit,
    scratch_dir: &Path,
) -> Result<BTreeMap<PathBuf, FileTime>> {
    let tarball_path = get_tarball_path(tarball_dir, computed_deps, unit);
    assert!(tarball_path.exists(), "{tarball_path:?} does not exist");
    println!("unpacking {tarball_path:?}");
    // FIXME: return BTreeMap<PathBuf, DateTime> or something, by unpacking what Archive::_unpack() does internally
    let mut archive = Archive::new(File::open(tarball_path)?);
    tracked_unpack(&mut archive, scratch_dir)
}

/// Originally  copy-pasta of tar-rs's private _unpack() method, but returns the list of paths that have been unpacked.
/// This was originally proposed as https://github.com/alexcrichton/tar-rs/pull/293 but it was determined
/// that this isn't something that tar-rs should support directly - we should instead use the tools that
/// tar-rs provides, and implement it ourselves.
/// TODO: Also include the folder mtime setting code from https://github.com/alexcrichton/tar-rs/pull/217/
fn tracked_unpack<R: Read>(
    archive: &mut Archive<R>,
    dst: &Path,
) -> Result<BTreeMap<PathBuf, FileTime>> {
    let mut file_timestamps = BTreeMap::default();
    // Delay any directory entries until the end (they will be created if needed by
    // descendants), to ensure that directory permissions do not interfer with descendant
    // extraction.
    let mut directories = Vec::new();
    for entry in archive.entries()? {
        let mut file = entry.context("reading entry from archive")?;
        if file.header().entry_type() == EntryType::Directory {
            directories.push(file);
        } else {
            insert_timestamp(&mut file_timestamps, &file)?;
            file.unpack_in(dst)?;
        }
    }
    for mut dir in directories {
        insert_timestamp(&mut file_timestamps, &dir)?;
        dir.unpack_in(dst)?;
    }
    Ok(file_timestamps)
}

fn insert_timestamp<R: Read>(
    file_timestamps: &mut BTreeMap<PathBuf, FileTime>,
    file: &Entry<R>,
) -> Result<(), anyhow::Error> {
    let path = file.path()?.clone().into_owned();
    // FIXME: pack and unpack high resolution mtimes using what's in PAX extension headers.
    let time = FileTime::from_unix_time(file.header().mtime()? as i64, 0);
    match file_timestamps.entry(path) {
        btree_map::Entry::Vacant(entry) => {
            entry.insert(time);
            Ok(())
        }
        btree_map::Entry::Occupied(entry) => anyhow::bail!(
            "duplicate entry for {:?} ({:?}) when trying to insert {:?}",
            entry.key(),
            entry.get(),
            time,
        ),
    }
}
