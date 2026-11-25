//! statvfs-vs-du check against a fixture mount table and an injected probe.
//! Read-only: no symlink is ever followed.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process;

use disk_full_doctor::{scan, FsProbe, FsStats, Root, Status};

const GIB: u64 = 1024 * 1024 * 1024;

struct MapProbe {
    root_mount: PathBuf,
    nested_mount: PathBuf,
    proc_mount: PathBuf,
}

impl FsProbe for MapProbe {
    fn statvfs(&self, path: &Path) -> io::Result<FsStats> {
        let stats = |blocks: u64| FsStats {
            blocks,
            blocks_free: 0,
            block_size: 1,
            files: 0,
            files_free: 0,
        };
        if path == self.root_mount {
            Ok(stats(10 * GIB))
        } else if path == self.nested_mount {
            Ok(stats(3 * GIB))
        } else if path == self.proc_mount {
            Ok(stats(0))
        } else {
            panic!("statvfs left the fixture mounts: {}", path.display());
        }
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/df_du_gap")
        .join(name)
}

/// Copies the committed mountinfo into a temp root and adds the big files as
/// sparse files (the check sums apparent sizes, so no disk space is used).
fn gap_root() -> TempRoot {
    let temp = TempRoot::new("df-du-gap-big");
    let mountinfo = fixture("gap").join("proc/self/mountinfo");
    fs::create_dir_all(temp.path().join("proc/self")).expect("proc dir");
    fs::copy(&mountinfo, temp.path().join("proc/self/mountinfo")).expect("copy mountinfo");
    fs::create_dir_all(temp.path().join("var/lib/docker")).expect("docker dir");
    sparse_file(&temp.path().join("bigfile"), 4 * GIB);
    sparse_file(&temp.path().join("var/lib/docker/bigfile"), 3 * GIB);
    temp
}

fn sparse_file(path: &Path, len: u64) {
    fs::File::create(path)
        .and_then(|file| file.set_len(len))
        .expect("create sparse file");
}

fn df_du_gap_findings(
    report: &disk_full_doctor::Report,
    status: Status,
) -> Vec<&disk_full_doctor::Finding> {
    report
        .findings
        .iter()
        .filter(|finding| finding.check == "df_du_gap" && finding.status == status)
        .collect()
}

#[test]
fn gap_is_statvfs_used_minus_du_and_the_walk_does_not_cross_mounts() {
    let temp = gap_root();
    let root = Root::new(temp.path());
    let probe = MapProbe {
        root_mount: root.join("/"),
        nested_mount: root.join("/var/lib/docker"),
        proc_mount: root.join("/proc"),
    };

    let report = scan(&root, &probe);
    let warns = df_du_gap_findings(&report, Status::Warn);
    let root_finding = warns
        .iter()
        .find(|finding| finding.evidence.starts_with("/:"))
        .expect("root mount finding");
    // 10 GiB statvfs used, 4 GiB of files under "/" (the 3 GiB under the
    // nested /var/lib/docker mount must not be summed into this du).
    assert_eq!(root_finding.bytes, Some(6 * GIB));

    let oks = df_du_gap_findings(&report, Status::Ok);
    let nested_finding = oks
        .iter()
        .find(|finding| finding.evidence.starts_with("/var/lib/docker:"))
        .expect("nested mount finding");
    assert_eq!(nested_finding.bytes, Some(0));
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

struct OkProbe;

impl FsProbe for OkProbe {
    fn statvfs(&self, _path: &Path) -> io::Result<FsStats> {
        Ok(FsStats {
            blocks: 1,
            blocks_free: 0,
            block_size: 1,
            files: 0,
            files_free: 0,
        })
    }
}

#[test]
fn unreadable_directory_under_a_mount_needs_root() {
    let temp = TempRoot::new("df-du-gap");
    fs::create_dir_all(temp.path().join("proc/self")).expect("proc dir");
    fs::write(
        temp.path().join("proc/self/mountinfo"),
        "22 1 8:1 / / rw,relatime - ext4 /dev/sda1 rw\n",
    )
    .expect("write mountinfo");
    let secret = temp.path().join("secret");
    fs::create_dir_all(&secret).expect("secret dir");
    let mut perms = fs::metadata(&secret).expect("meta").permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&secret, perms).expect("chmod");

    let report = scan(&Root::new(temp.path()), &OkProbe);
    let denied = df_du_gap_findings(&report, Status::NeedsRoot);
    assert_eq!(denied.len(), 1, "findings: {:?}", report.findings);
    assert!(
        denied[0].evidence.contains("secret"),
        "evidence: {}",
        denied[0].evidence
    );
    assert!(
        denied[0].hint.contains("run with sudo to see"),
        "hint: {}",
        denied[0].hint
    );
}
