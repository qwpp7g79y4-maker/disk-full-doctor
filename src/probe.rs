//! Filesystem measurements behind a trait so tests inject values
//! instead of calling `statvfs` on the host.

use std::io;
use std::path::Path;

/// Snapshot of the `statvfs` fields the checks need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsStats {
    pub blocks: u64,
    pub blocks_free: u64,
    pub block_size: u64,
    pub files: u64,
    pub files_free: u64,
}

/// Read filesystem accounting for one path.
pub trait FsProbe {
    fn statvfs(&self, path: &Path) -> io::Result<FsStats>;
}

/// `statvfs` on the path the check passes in.
///
/// Checks pass paths already joined onto [`crate::Root`]. Tests never use
/// this type; they inject their own [`FsProbe`].
#[derive(Debug, Default)]
pub struct RealFsProbe;

impl FsProbe for RealFsProbe {
    fn statvfs(&self, path: &Path) -> io::Result<FsStats> {
        let stats = rustix::fs::statvfs(path).map_err(io::Error::from)?;
        let block_size = if stats.f_frsize == 0 {
            stats.f_bsize
        } else {
            stats.f_frsize
        };
        Ok(FsStats {
            blocks: stats.f_blocks,
            blocks_free: stats.f_bfree,
            block_size,
            files: stats.f_files,
            files_free: stats.f_ffree,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn reads_a_temp_directory_and_errors_when_the_path_is_missing() {
        let dir = std::env::temp_dir().join(format!("dfd-probe-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        assert_ne!(dir, std::path::PathBuf::from("/"));

        let stats = RealFsProbe.statvfs(&dir).expect("statvfs on the temp dir");
        assert!(stats.block_size > 0);
        assert!(stats.blocks > 0);

        let missing = dir.join("missing");
        let err = RealFsProbe.statvfs(&missing).expect_err("missing path");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);

        let _ = fs::remove_dir_all(&dir);
    }
}
