//! BuildKit cache size check on fixtures. No `docker builder prune` or any
//! other command that changes anything.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process;

use disk_full_doctor::{scan, FsProbe, FsStats, Root, Status};

struct PanicProbe;

impl FsProbe for PanicProbe {
    fn statvfs(&self, path: &Path) -> std::io::Result<FsStats> {
        panic!("buildkit must not statvfs {}", path.display());
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/buildkit")
        .join(name)
}

fn buildkit_findings(
    report: &disk_full_doctor::Report,
    status: Status,
) -> Vec<&disk_full_doctor::Finding> {
    report
        .findings
        .iter()
        .filter(|finding| finding.check == "buildkit" && finding.status == status)
        .collect()
}

#[test]
fn fixture_reports_total_bytes() {
    let report = scan(&Root::new(fixture("with-cache")), &PanicProbe);
    let oks = buildkit_findings(&report, Status::Ok);
    assert_eq!(oks.len(), 1, "findings: {:?}", report.findings);
    assert_eq!(oks[0].bytes, Some(4000 + 2000));
}

#[test]
fn missing_buildkit_dirs_is_not_present() {
    let report = scan(&Root::new(fixture("absent")), &PanicProbe);
    let missing = buildkit_findings(&report, Status::NotPresent);
    assert_eq!(missing.len(), 1, "findings: {:?}", report.findings);
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn path(&self) -> &Path {
        &self.path
    }

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
fn unreadable_buildkit_dir_needs_root() {
    let temp = TempRoot::new("buildkit");
    let buildkit = temp.path().join("var/lib/docker/buildkit");
    fs::create_dir_all(&buildkit).expect("buildkit dir");
    let mut perms = fs::metadata(&buildkit).expect("meta").permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&buildkit, perms).expect("chmod");

    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let denied = buildkit_findings(&report, Status::NeedsRoot);
    assert_eq!(denied.len(), 1, "findings: {:?}", report.findings);
    assert!(
        denied[0].evidence.contains("/var/lib/docker/buildkit"),
        "evidence: {}",
        denied[0].evidence
    );
    assert!(
        denied[0].hint.contains("run with sudo to see"),
        "hint: {}",
        denied[0].hint
    );
}
