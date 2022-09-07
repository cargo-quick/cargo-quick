use std::{fs::File, path::PathBuf};

use anyhow::bail;
use tar::Archive;

use crate::{archive::get_high_res_mtime, repo::Repo};

// At some point I will pick a command-line parsing crate, but for now this will do.
pub fn exec(args: &[String]) -> anyhow::Result<()> {
    assert_eq!(args[0], "repo");
    assert_eq!(args[1], "find");
    if args.len() != 3 {
        bail!("USAGE: cargo quickbuild repo find $filename");
    }

    let filename = args[2].as_str();
    assert_eq!(args, &["repo", "find", filename]);
    let filename = PathBuf::from(filename);

    let repo = Repo::from_env();
    for tarball_path in repo.find_file(&filename) {
        let mut archive = Archive::new(File::open(&tarball_path).unwrap());
        let mut entry = archive
            .entries()
            .unwrap()
            .map(|entry| entry.unwrap())
            .find(|entry| entry.path().unwrap().as_ref() == filename)
            .unwrap();

        let mtime = get_high_res_mtime(&mut entry)?;

        println!("{filename:?} found in: {tarball_path:?} with mtime {mtime}");
    }

    Ok(())
}
