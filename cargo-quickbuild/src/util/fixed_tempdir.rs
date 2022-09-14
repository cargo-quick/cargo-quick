use std::{
    fs::remove_dir_all,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

// HACK: keep tempdir location fixed to see if that fixes compilation issues.
pub struct FixedTempDir {
    path: PathBuf,
}

impl FixedTempDir {
    pub fn new(name: &str) -> Result<Self> {
        let path = std::env::temp_dir().join(name);
        let _ = remove_dir_all(&path);
        std::fs::create_dir(&path).with_context(|| format!("making tempdir in {path:?}"))?;
        Ok(FixedTempDir { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for FixedTempDir {
    fn drop(&mut self) {
        let _ = remove_dir_all(&self.path);
    }
}
