//! Inode check against a fixture mount table and an injected probe.
//! The probe refuses any path outside the fixture, so the host is not queried.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process;

use disk_full_doctor::{scan, FsProbe, FsStats, Root, Status};

struct MapProbe {
    full: PathBuf,
    quiet: PathBuf,
}

impl FsProbe for MapProbe {
    fn statvfs(&self, path: &Path) -> io::Result<FsStats> {
        let stats = |files_free: u64| FsStats {
            blocks: 1000,
            blocks_free: 1000,
            block_size: 4096,
            files: 100,
            files_free,
        };
        if path == self.full {
            Ok(stats(5))
        } else if path == self.quiet {
            Ok(stats(90))
        } else {
            panic!("statvfs left the fixture mounts: {}", path.display());
        }
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn ninety_five_percent_warns_and_ten_percent_is_ok() {
    let fixture = fixture("inodes");
    let root = Root::new(&fixture);
    let probe = MapProbe {
        full: root.join("/"),
        quiet: root.join("/var"),
    };
    assert!(probe.full.starts_with(&fixture));
    assert!(probe.quiet.starts_with(&fixture));
    assert_ne!(probe.full, PathBuf::from("/"));

    let report = scan(&root, &probe);
    let inodes: Vec<_> = report
        .findings
        .iter()
        .filter(|finding| finding.check == "inodes")
        .collect();

    let full = inodes
        .iter()
        .find(|finding| finding.evidence.contains("mount / "))
        .expect("root mount");
    let quiet = inodes
        .iter()
        .find(|finding| finding.evidence.contains("mount /var "))
        .expect("var mount");

    assert_eq!(full.status, Status::Warn, "evidence: {}", full.evidence);
    assert!(
        full.evidence.contains("(95%)"),
        "evidence: {}",
        full.evidence
    );
    assert_eq!(full.inodes, Some(95));

    assert_eq!(quiet.status, Status::Ok, "evidence: {}", quiet.evidence);
    assert!(
        quiet.evidence.contains("(10%)"),
        "evidence: {}",
        quiet.evidence
    );
    assert_eq!(quiet.inodes, Some(10));
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("dfd-{label}-{}", process::id()));
        if path.exists() {
            chmod_tree(&path, 0o755);
            fs::remove_dir_all(&path).expect("clean temp root");
        }
        fs::create_dir_all(&path).expect("create temp root");
        Self { path }
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        chmod_tree(&self.path, 0o755);
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn chmod_tree(path: &Path, mode: u32) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    let mut perms = meta.permissions();
    perms.set_mode(mode);
    let _ = fs::set_permissions(path, perms);
    if meta.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                chmod_tree(&entry.path(), mode);
            }
        }
    }
}

#[test]
fn unreadable_mountinfo_needs_root() {
    let temp = TempRoot::new("inodes");
    let mountinfo = temp.path.join("proc/self/mountinfo");
    fs::create_dir_all(mountinfo.parent().expect("parent")).expect("dirs");
    fs::write(&mountinfo, "22 1 8:1 / / rw - ext4 /dev/sda1 rw\n").expect("write");
    let mut perms = fs::metadata(&mountinfo).expect("meta").permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&mountinfo, perms).expect("chmod");

    let report = scan(&Root::new(&temp.path), &PanicProbe);
    let denied: Vec<_> = report
        .findings
        .iter()
        .filter(|finding| finding.check == "inodes" && finding.status == Status::NeedsRoot)
        .collect();
    assert_eq!(denied.len(), 1, "findings: {:?}", report.findings);
    assert!(
        denied[0].hint.contains("run with sudo to see"),
        "hint: {}",
        denied[0].hint
    );
    assert!(
        denied[0].evidence.contains("mountinfo"),
        "evidence: {}",
        denied[0].evidence
    );
}

struct PanicProbe;

impl FsProbe for PanicProbe {
    fn statvfs(&self, path: &Path) -> io::Result<FsStats> {
        panic!("unreadable mountinfo must not statvfs {}", path.display());
    }
}

/// Accepts only paths inside the fixture and reports 10% inode use.
struct InsideProbe {
    prefix: PathBuf,
}

impl FsProbe for InsideProbe {
    fn statvfs(&self, path: &Path) -> io::Result<FsStats> {
        assert!(
            path.starts_with(&self.prefix),
            "statvfs left the fixture: {}",
            path.display()
        );
        Ok(FsStats {
            blocks: 1000,
            blocks_free: 1000,
            block_size: 4096,
            files: 100,
            files_free: 90,
        })
    }
}

#[test]
fn invalid_utf8_byte_does_not_fail_the_inode_check() {
    let temp = TempRoot::new("inodes-utf8");
    let mountinfo = temp.path.join("proc/self/mountinfo");
    fs::create_dir_all(mountinfo.parent().expect("parent")).expect("dirs");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"22 1 8:1 / / rw,relatime - ext4 /dev/sda1 rw\n");
    bytes.extend_from_slice(b"27 22 8:3 / /mnt/bad\xffname rw - ext4 /dev/sda3 rw\n");
    bytes.extend_from_slice(b"26 22 8:2 / /var rw - ext4 /dev/sda2 rw\n");
    fs::write(&mountinfo, &bytes).expect("write");

    let report = scan(
        &Root::new(&temp.path),
        &InsideProbe {
            prefix: temp.path.clone(),
        },
    );
    let inodes: Vec<_> = report
        .findings
        .iter()
        .filter(|finding| finding.check == "inodes")
        .collect();

    assert!(
        inodes.iter().all(|finding| finding.status != Status::Error),
        "one invalid byte must not fail the inode check: {inodes:?}"
    );
    assert!(
        inodes
            .iter()
            .all(|finding| !finding.evidence.contains("valid UTF-8")),
        "evidence still reports a UTF-8 read failure: {inodes:?}"
    );
    let root_mount = inodes
        .iter()
        .find(|finding| finding.evidence.contains("mount / "))
        .expect("root mount was measured");
    let var_mount = inodes
        .iter()
        .find(|finding| finding.evidence.contains("mount /var "))
        .expect("/var was measured");
    assert_eq!(root_mount.status, Status::Ok);
    assert_eq!(var_mount.status, Status::Ok);
    assert_eq!(
        inodes.len(),
        3,
        "the bad mount must still be measured: {inodes:?}"
    );
}
