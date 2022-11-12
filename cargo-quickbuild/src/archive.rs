use std::collections::btree_map;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::{collections::BTreeMap, path::PathBuf};

use anyhow::{Context, Ok, Result};

use filetime::FileTime;
use tar::{Archive, Builder, Entry, EntryType};

use crate::pax::{BuilderExt, PaxBuilder};

pub fn tar_target_dir(
    scratch_dir_path: std::path::PathBuf,
    file: File,
    file_timestamps_to_exclude: &BTreeMap<PathBuf, FileTime>,
) -> Result<()> {
    // FIXME: each tarball contains duplicates of all of the dependencies that we just unpacked already
    // Either inline whatever append_dir_all() is doing and add filtering, or delete files before making the tarball
    let mut tar = Builder::new(file);
    let mut problem = false;
    for entry in walkdir::WalkDir::new(scratch_dir_path.join("target")) {
        let entry = entry?;
        let path = entry.path();
        let dest = path.strip_prefix(&scratch_dir_path).unwrap();
        // HACK: don't tar these files.
        if let ".rustc_info.json" | ".cargo-lock" | "CACHEDIR.TAG" =
            path.file_name().unwrap().to_str().unwrap()
        {
            continue;
        }
        if dest.starts_with("target/cargo-timings") {
            log::debug!("skipping timings file: {dest:?}");
            continue;
        }
        let mtime = FileTime::from_last_modification_time(&entry.metadata()?);
        match file_timestamps_to_exclude.get(dest) {
            Some(timestamp) if &mtime == timestamp => {
                log::debug!("skipping {dest:?} because it already exists");
            }
            Some(timestamp) if entry.file_type().is_file() => {
                let mut contents = String::new();
                match File::open(entry.path())?.read_to_string(&mut contents) {
                    core::result::Result::Ok(_) => {
                        println!("{dest:?}'s mtime has changed from {timestamp:?} to {mtime:?} and it is not a dir. contents:\n{contents:?}");
                    },
                    Err(_) => println!("{dest:?}'s mtime has changed from {timestamp:?} to {mtime:?} and it is not a dir. (binary file)"),
                }
                problem = true;
                append_path_with_mtime(&mut tar, path, dest, mtime)?;
            }
            Some(timestamp) => {
                log::debug!(
                    "adding {dest:?} because its mtime has changed from {timestamp:?} to {mtime:?}"
                );
                append_path_with_mtime(&mut tar, path, dest, mtime)?;
            }
            None => {
                append_path_with_mtime(&mut tar, path, dest, mtime)?;
            }
        }
    }
    tar.finish()?;
    if problem {
        panic!("Got a timestamp problem. See above logging for details.")
    }

    Ok(())
}

fn append_path_with_mtime(
    tar: &mut Builder<File>,
    path: &Path,
    dest: &Path,
    mtime: FileTime,
) -> Result<(), anyhow::Error> {
    let mut pax = PaxBuilder::new();
    pax.add(
        "mtime",
        &format!("{}.{:09}", mtime.unix_seconds(), mtime.nanoseconds()),
    );
    tar.append_pax_extensions(&pax)?;

    tar.append_path_with_name(path, dest)?;
    Ok(())
}

/// Originally  copy-pasta of tar-rs's private _unpack() method, but returns the list of paths that have been unpacked.
/// This was originally proposed as https://github.com/alexcrichton/tar-rs/pull/293 but it was determined
/// that this isn't something that tar-rs should support directly - we should instead use the tools that
/// tar-rs provides, and implement it ourselves.
/// TODO: Also include the folder mtime setting code from https://github.com/alexcrichton/tar-rs/pull/217/
pub fn tracked_unpack<R: Read>(
    archive: &mut Archive<R>,
    dst: &Path,
) -> Result<BTreeMap<PathBuf, FileTime>> {
    let mut file_timestamps = BTreeMap::default();
    // Delay any directory entries until the end (they will be created if needed by
    // descendants), to ensure that directory permissions do not interfer with descendant
    // extraction.
    let mut directories = Vec::new();
    let mut problem = String::new();
    for entry in archive.entries()? {
        let mut file = entry.context("reading entry from archive")?;
        if file.header().entry_type() == EntryType::Directory {
            directories.push(file);
        } else {
            let mtime = get_high_res_mtime(&mut file)?;
            let relative_path = file.path()?.to_path_buf();
            let absolute_path = dst.join(&relative_path);

            insert_timestamp(&mut file_timestamps, &relative_path, mtime)?;
            if absolute_path.exists() {
                let on_disk_mtime =
                    FileTime::from_last_modification_time(&std::fs::metadata(&absolute_path)?);
                if mtime != on_disk_mtime {
                    // FIXME: make an extension trait for this conversion
                    let mtime =
                        chrono::NaiveDateTime::from_timestamp(mtime.seconds(), mtime.nanoseconds());
                    let on_disk_mtime = chrono::NaiveDateTime::from_timestamp(
                        on_disk_mtime.seconds(),
                        on_disk_mtime.nanoseconds(),
                    );
                    let now = chrono::Utc::now();

                    let on_disk_contents = std::fs::read(&absolute_path)?;
                    let mut from_tarball_contents = Vec::new();
                    file.read_to_end(&mut from_tarball_contents)?;

                    let contents_differ_message = if on_disk_contents == from_tarball_contents {
                        String::from("contents identical")
                    } else {
                        format!(
                            "contents differ:\n\
                        on disk:\n{on_disk}\n\
                        from tarball:\n{from_tarball}",
                            on_disk = std::str::from_utf8(&on_disk_contents)
                                .unwrap_or_else(|_| "<<binary file>>"),
                            from_tarball = std::str::from_utf8(&from_tarball_contents)
                                .unwrap_or_else(|_| "<<binary file>>"),
                        )
                    };

                    problem = format!(
                        "{mtime} != {on_disk_mtime} for {relative_path:?} ({absolute_path:?}):\n\
                        (mtime != on_disk_mtime. now = {now})\n\
                        {contents_differ_message}",
                    );
                    eprintln!("{problem}");
                }
            }
            file.unpack_in(dst)?;
            filetime::set_file_times(&absolute_path, mtime, mtime)?;
        }
    }
    if !problem.is_empty() {
        anyhow::bail!("{problem}")
    }
    for mut dir in directories {
        let path = dir.path()?.to_path_buf();
        let mtime = get_high_res_mtime(&mut dir)?;
        insert_timestamp(&mut file_timestamps, &path, mtime)?;
        dir.unpack_in(dst)?;
        filetime::set_file_times(dst.join(dir.path()?), mtime, mtime)?;
    }
    Ok(file_timestamps)
}

pub(crate) fn get_high_res_mtime<R: Read>(file: &mut Entry<R>) -> Result<FileTime, anyhow::Error> {
    let path = file.path().unwrap().into_owned();
    let low_res_mtime = file.header().mtime().unwrap();
    let mtime = file
        .pax_extensions()?
        .expect("refusing to unpack tarball with low-resolution mtimes")
        .into_iter()
        .find(|e| e.as_ref().unwrap().key().as_ref().unwrap() == &"mtime")
        .map(|x| x.unwrap().value().unwrap())
        .or({
            if low_res_mtime == 123456789 {
                Some("123456789.0")
            } else {
                None
            }
        })
        .with_context(|| format!("no high res mtime for {path:?} - low res = {low_res_mtime}",))?;
    let (seconds, nanos) = mtime.split_once('.').unwrap();
    let seconds = seconds.parse()?;
    // right pad with 0s - https://docs.rs/pad/0.1.6/pad/#padding-in-the-stdlib
    let nanos = format!("{nanos:0<9}")
        .parse()
        .with_context(|| format!("parsing {nanos:?}"))?;
    Ok(FileTime::from_unix_time(seconds, nanos))
}

fn insert_timestamp(
    file_timestamps: &mut BTreeMap<PathBuf, FileTime>,
    path: &Path,
    mtime: FileTime,
) -> Result<(), anyhow::Error> {
    log::debug!("have high-res timesamp for {path:?}: {mtime}");

    match file_timestamps.entry(path.to_owned()) {
        btree_map::Entry::Vacant(entry) => {
            entry.insert(mtime);
            Ok(())
        }
        btree_map::Entry::Occupied(entry) => anyhow::bail!(
            "duplicate entry for {:?} ({:?}) when trying to insert {:?}",
            entry.key(),
            entry.get(),
            mtime,
        ),
    }
}
