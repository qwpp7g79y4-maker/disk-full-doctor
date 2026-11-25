//! deleted_open against a fake proc tree. Nothing here reads the host `/proc`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process;

use disk_full_doctor::{scan, FsProbe, FsStats, Root, Status};

struct RejectProbe;

impl FsProbe for RejectProbe {
    fn statvfs(&self, path: &Path) -> std::io::Result<FsStats> {
        panic!("deleted_open must not statvfs {}", path.display());
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
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

    fn path(&self) -> &Path {
        &self.path
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
fn fixture_reports_deleted_fd_bytes_and_pid() {
    let root = Root::new(fixture("deleted_open"));
    let report = scan(&root, &RejectProbe);
    let warns: Vec<_> = report
        .findings
        .iter()
        .filter(|finding| finding.check == "deleted_open" && finding.status == Status::Warn)
        .collect();
    assert_eq!(warns.len(), 1, "findings: {:?}", report.findings);
    assert_eq!(warns[0].bytes, Some(4096));
    assert!(
        warns[0].evidence.contains("pid 424242 "),
        "evidence: {}",
        warns[0].evidence
    );
    assert!(warns[0].evidence.contains("(deleted)"));
}

#[test]
fn unreadable_fd_directory_needs_root() {
    let temp = TempRoot::new("deleted-fd");
    let fd_dir = temp.path().join("proc/7/fd");
    fs::create_dir_all(&fd_dir).expect("create fd dir");
    let mut perms = fs::metadata(&fd_dir).expect("metadata").permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&fd_dir, perms).expect("chmod fd dir");

    let report = scan(&Root::new(temp.path()), &RejectProbe);
    let denied: Vec<_> = report
        .findings
        .iter()
        .filter(|finding| finding.check == "deleted_open" && finding.status == Status::NeedsRoot)
        .collect();
    assert_eq!(denied.len(), 1, "findings: {:?}", report.findings);
    assert!(
        denied[0].evidence.contains("/proc/7/fd"),
        "evidence: {}",
        denied[0].evidence
    );
    assert!(
        denied[0].hint.contains("run with sudo to see"),
        "hint: {}",
        denied[0].hint
    );
    assert!(denied[0].hint.contains("pid 7"), "hint: {}", denied[0].hint);
}
