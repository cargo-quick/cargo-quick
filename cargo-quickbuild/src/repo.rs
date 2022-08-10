use std::{fs::File, path::PathBuf};

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
}
