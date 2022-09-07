use std::{
    ffi::OsStr,
    fs::File,
    path::{Path, PathBuf},
};

use tar::Archive;

use crate::{
    description::PackageDescription,
    stats::{ComputedStats, Stats},
};

pub struct Repo {
    tarball_dir: PathBuf,
}

impl Repo {
    pub fn from_env() -> Self {
        let tarball_dir = match std::env::var("CARGO_QUICK_TARBALL_DIR") {
            Ok(path) => PathBuf::from(path),
            _ => home::home_dir().unwrap().join("tmp/quick"),
        };

        std::fs::create_dir_all(&tarball_dir).unwrap();
        Self { tarball_dir }
    }

    pub fn has(&self, package: &PackageDescription) -> bool {
        self.tarball_path(package).exists()
    }

    pub fn read(&self, package: &PackageDescription) -> std::io::Result<File> {
        File::open(self.tarball_path(package))
    }

    pub fn write(&self, package: &PackageDescription) -> std::io::Result<File> {
        let temp_tarball_path = self.tarball_path(package).with_extension("temp.tar");
        File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(temp_tarball_path)
    }

    pub fn commit(&self, package: &PackageDescription, stats: Stats) -> std::io::Result<()> {
        let tarball_path = self.tarball_path(package);
        let temp_tarball_path = tarball_path.with_extension("temp.tar");
        let stats_path = tarball_path.with_extension("stats.json");
        let temp_stats_path = temp_tarball_path.with_extension("stats.json");

        serde_json::to_writer_pretty(
            std::fs::File::create(&temp_stats_path)?,
            &ComputedStats::from(stats),
        )?;
        std::fs::rename(&temp_stats_path, stats_path)?;
        std::fs::rename(&temp_tarball_path, &tarball_path)?;

        println!("wrote to {tarball_path:?}");

        Ok(())
    }

    fn tarball_path(&self, package: &PackageDescription) -> PathBuf {
        let digest = package.pretty_digest();
        self.tarball_dir.join(format!("{digest}.tar"))
    }

    // FIXME: save the PackagDescription on disk somewhere, so that we can make this function return
    // `impl Iterator<Item = PackageDescription>` or something?
    pub(crate) fn find_file(&self, filename: &Path) -> impl Iterator<Item = PathBuf> {
        let filename = filename.to_owned();

        walkdir::WalkDir::new(&self.tarball_dir)
            .into_iter()
            .filter_map(move |entry| {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.extension() != Some(OsStr::new("tar")) || !path.is_file() {
                    return None;
                }

                let mut archive = Archive::new(File::open(&path).unwrap());
                if archive
                    .entries()
                    .unwrap()
                    .any(|entry| entry.unwrap().path().unwrap() == filename)
                {
                    Some(path.to_owned())
                } else {
                    None
                }
            })
    }
}
