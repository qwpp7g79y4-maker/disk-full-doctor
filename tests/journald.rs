//! Journald log size check on fixtures. No `journalctl` call.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process;

use disk_full_doctor::{scan, FsProbe, FsStats, Root, Status};

struct PanicProbe;

impl FsProbe for PanicProbe {
    fn statvfs(&self, path: &Path) -> std::io::Result<FsStats> {
        panic!("journald must not statvfs {}", path.display());
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/journald")
        .join(name)
}

fn journald_findings(
    report: &disk_full_doctor::Report,
    status: Status,
) -> Vec<&disk_full_doctor::Finding> {
    report
        .findings
        .iter()
        .filter(|finding| finding.check == "journald" && finding.status == status)
        .collect()
}

#[test]
fn fixture_reports_total_bytes_and_system_max_use() {
    let report = scan(&Root::new(fixture("with-logs")), &PanicProbe);
    let oks = journald_findings(&report, Status::Ok);
    assert_eq!(oks.len(), 1, "findings: {:?}", report.findings);
    assert_eq!(oks[0].bytes, Some(3000 + 1000));
    assert!(
        oks[0].evidence.contains("SystemMaxUse=200M"),
        "evidence: {}",
        oks[0].evidence
    );
}

#[test]
fn missing_journal_dirs_is_not_present() {
    let report = scan(&Root::new(fixture("absent")), &PanicProbe);
    let missing = journald_findings(&report, Status::NotPresent);
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
fn unreadable_journal_dir_needs_root() {
    let temp = TempRoot::new("journald");
    let journal = temp.path().join("var/log/journal");
    fs::create_dir_all(&journal).expect("journal dir");
    let mut perms = fs::metadata(&journal).expect("meta").permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&journal, perms).expect("chmod");

    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let denied = journald_findings(&report, Status::NeedsRoot);
    assert_eq!(denied.len(), 1, "findings: {:?}", report.findings);
    assert!(
        denied[0].evidence.contains("/var/log/journal"),
        "evidence: {}",
        denied[0].evidence
    );
    assert!(
        denied[0].hint.contains("run with sudo to see"),
        "hint: {}",
        denied[0].hint
    );
}
