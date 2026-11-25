//! Branch coverage for the checks, through [`scan`] on temp trees.
//!
//! Nothing here passes `--root /` or reads the host `/proc`.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use disk_full_doctor::{scan, FsProbe, FsStats, Root, Status};

struct PanicProbe;

impl FsProbe for PanicProbe {
    fn statvfs(&self, path: &Path) -> io::Result<FsStats> {
        panic!("this check must not statvfs {}", path.display());
    }
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("dfd-branch-{label}-{}-{n}", process::id()));
        if path.exists() {
            chmod_tree(&path, 0o755);
            fs::remove_dir_all(&path).expect("clean temp root");
        }
        fs::create_dir_all(&path).expect("create temp root");
        assert_ne!(path, PathBuf::from("/"));
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

fn chmod(path: &Path, mode: u32) {
    let mut perms = fs::symlink_metadata(path).expect("meta").permissions();
    perms.set_mode(mode);
    fs::set_permissions(path, perms).expect("chmod");
}

fn plant(path: &Path, bytes: impl AsRef<[u8]>) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parents");
    }
    fs::write(path, bytes).expect("write");
}

fn mountinfo(root: &Path, body: &str) {
    plant(&root.join("proc/self/mountinfo"), body.as_bytes());
}

fn rows<'a>(
    report: &'a disk_full_doctor::Report,
    check: &str,
    status: Status,
) -> Vec<&'a disk_full_doctor::Finding> {
    report
        .findings
        .iter()
        .filter(|finding| finding.check == check && finding.status == status)
        .collect()
}

fn one<'a>(
    report: &'a disk_full_doctor::Report,
    check: &str,
    status: Status,
) -> &'a disk_full_doctor::Finding {
    let found = rows(report, check, status);
    assert_eq!(found.len(), 1, "{check} {status:?}: {:?}", report.findings);
    found[0]
}

#[test]
fn journal_persistent_only_parses_system_max_use() {
    let temp = TempRoot::new("journal-persistent");
    plant(
        &temp.path().join("var/log/journal/system.journal"),
        vec![b'j'; 100],
    );
    plant(
        &temp.path().join("etc/systemd/journald.conf"),
        b"# comment\n\n; also\nSystemMaxUse=\n  SystemMaxUse=10M\nStorage=persistent\n",
    );
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "journald", Status::Ok);
    assert_eq!(finding.bytes, Some(100));
    assert!(
        finding.evidence.contains("SystemMaxUse=10M"),
        "{}",
        finding.evidence
    );
}

#[test]
fn journal_runtime_only_sums_that_side() {
    let temp = TempRoot::new("journal-runtime");
    plant(
        &temp.path().join("run/log/journal/system.journal"),
        vec![b'r'; 30],
    );
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "journald", Status::Ok);
    assert_eq!(finding.bytes, Some(30));
    assert!(!finding.evidence.contains("SystemMaxUse="));
}

#[test]
fn journal_conf_without_a_value_omits_system_max_use() {
    let temp = TempRoot::new("journal-empty-max");
    plant(
        &temp.path().join("var/log/journal/system.journal"),
        b"123456789012",
    );
    plant(
        &temp.path().join("etc/systemd/journald.conf"),
        b"# nope\n; nope\n\nSystemMaxUse=\nSystemMaxUse =50M\n",
    );
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "journald", Status::Ok);
    assert_eq!(finding.bytes, Some(12));
    assert!(!finding.evidence.contains("SystemMaxUse="));
}

#[test]
fn unreadable_journal_conf_omits_system_max_use() {
    let temp = TempRoot::new("journal-conf-mode");
    plant(&temp.path().join("var/log/journal/system.journal"), b"abc");
    let conf = temp.path().join("etc/systemd/journald.conf");
    plant(&conf, b"SystemMaxUse=10M\n");
    chmod(&conf, 0o000);
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "journald", Status::Ok);
    assert_eq!(finding.bytes, Some(3));
    assert!(!finding.evidence.contains("SystemMaxUse="));
}

#[test]
fn journal_path_that_is_a_file_is_an_error() {
    let temp = TempRoot::new("journal-file");
    plant(&temp.path().join("var/log/journal"), b"not a directory");
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "journald", Status::Error);
    assert!(
        finding.evidence.contains("cannot read /var/log/journal"),
        "{}",
        finding.evidence
    );
}

#[test]
fn runtime_journal_path_that_is_a_file_is_an_error() {
    let temp = TempRoot::new("journal-runtime-file");
    plant(&temp.path().join("run/log/journal"), b"not a directory");
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "journald", Status::Error);
    assert!(
        finding.evidence.contains("cannot read /run/log/journal"),
        "{}",
        finding.evidence
    );
}

#[test]
fn unreadable_nested_journal_dir_needs_root() {
    let temp = TempRoot::new("journal-nested");
    plant(
        &temp.path().join("var/log/journal/system.journal"),
        b"12345678",
    );
    let secret = temp.path().join("var/log/journal/secret");
    fs::create_dir_all(&secret).expect("secret");
    chmod(&secret, 0o000);
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "journald", Status::NeedsRoot);
    assert!(finding.evidence.contains("/var/log/journal"));
    assert!(finding.hint.contains("run with sudo to see"));
}

#[test]
fn unreadable_runtime_journal_needs_root() {
    let temp = TempRoot::new("journal-runtime-mode");
    plant(
        &temp.path().join("var/log/journal/system.journal"),
        b"12345",
    );
    let runtime = temp.path().join("run/log/journal");
    fs::create_dir_all(&runtime).expect("runtime");
    chmod(&runtime, 0o000);
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "journald", Status::NeedsRoot);
    assert!(
        finding.evidence.contains("/run/log/journal"),
        "{}",
        finding.evidence
    );
}

#[test]
fn buildkit_skips_symlinks_and_sums_regular_files() {
    let temp = TempRoot::new("buildkit-files");
    let outside = std::env::temp_dir().join(format!(
        "dfd-buildkit-outside-{}-{}",
        process::id(),
        temp.path()
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("x")
    ));
    plant(&outside, vec![b'o'; 5000]);
    let _cleanup = RemoveFile(outside.clone());
    plant(
        &temp.path().join("var/lib/docker/buildkit/real"),
        vec![b'b'; 40],
    );
    plant(
        &temp.path().join("var/lib/docker/buildkit/sub/n"),
        vec![b'b'; 4],
    );
    std::os::unix::fs::symlink(&outside, temp.path().join("var/lib/docker/buildkit/link"))
        .expect("symlink");
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "buildkit", Status::Ok);
    assert_eq!(finding.bytes, Some(44));
}

#[test]
fn buildkit_external_only() {
    let temp = TempRoot::new("buildkit-external");
    plant(&temp.path().join("var/lib/buildkit/cache"), b"1234567");
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "buildkit", Status::Ok);
    assert_eq!(finding.bytes, Some(7));
}

#[test]
fn unreadable_external_buildkit_needs_root() {
    let temp = TempRoot::new("buildkit-external-mode");
    plant(&temp.path().join("var/lib/docker/buildkit/real"), b"abc");
    let external = temp.path().join("var/lib/buildkit");
    fs::create_dir_all(&external).expect("external");
    chmod(&external, 0o000);
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "buildkit", Status::NeedsRoot);
    assert!(
        finding.evidence.contains("/var/lib/buildkit"),
        "{}",
        finding.evidence
    );
    assert!(finding.hint.contains("run with sudo to see"));
}

#[test]
fn buildkit_dir_that_is_a_file_is_an_error() {
    let temp = TempRoot::new("buildkit-file");
    plant(&temp.path().join("var/lib/docker/buildkit"), b"nope");
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "buildkit", Status::Error);
    assert!(
        finding
            .evidence
            .contains("cannot read /var/lib/docker/buildkit"),
        "{}",
        finding.evidence
    );
}

#[test]
fn external_buildkit_file_is_an_error() {
    let temp = TempRoot::new("buildkit-external-file");
    plant(&temp.path().join("var/lib/buildkit"), b"nope");
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "buildkit", Status::Error);
    assert!(
        finding.evidence.contains("cannot read /var/lib/buildkit"),
        "{}",
        finding.evidence
    );
}

#[test]
fn docker_without_a_containers_dir_has_no_logs() {
    let temp = TempRoot::new("docker-no-containers");
    fs::create_dir_all(temp.path().join("var/lib/docker")).expect("docker");
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "docker_logs", Status::Ok);
    assert_eq!(finding.bytes, Some(0));
    assert!(finding.evidence.contains("no json-file container logs"));
}

#[test]
fn docker_skips_non_dirs_symlink_logs_and_non_utf8_names() {
    let temp = TempRoot::new("docker-skips");
    let containers = temp.path().join("var/lib/docker/containers");
    plant(&containers.join("notes.txt"), b"ignore");
    let bad = containers.join(OsString::from_vec(vec![0xff, 0xfe]));
    fs::create_dir_all(&bad).expect("bad id");
    plant(&bad.join("hidden-json.log"), vec![b'x'; 32]);
    let dir = containers.join("abc");
    fs::create_dir_all(&dir).expect("abc");
    plant(&dir.join("config.v2.json"), b"{}");
    plant(&dir.join("hostname"), b"box");
    std::os::unix::fs::symlink("hostname", dir.join("abc-json.log")).expect("symlink log");
    let mut odd_name = OsString::from_vec(vec![0xff]);
    odd_name.push("-json.log");
    plant(&dir.join(&odd_name), vec![b'z'; 9]);
    fs::create_dir(dir.join("nested-json.log")).expect("log dir");

    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "docker_logs", Status::Ok);
    assert!(finding.evidence.contains("no json-file container logs"));
    assert!(rows(&report, "docker_logs", Status::Warn).is_empty());
}

#[test]
fn docker_log_without_a_config_file_is_unlimited() {
    let temp = TempRoot::new("docker-no-config");
    let dir = temp.path().join("var/lib/docker/containers/noconfig");
    plant(&dir.join("noconfig-json.log"), vec![b'l'; 21]);
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "docker_logs", Status::Warn);
    assert_eq!(finding.bytes, Some(21));
    assert!(finding.evidence.contains("no max-size"));
}

#[test]
fn docker_config_directory_is_an_error() {
    let temp = TempRoot::new("docker-config-dir");
    let dir = temp.path().join("var/lib/docker/containers/cdir");
    fs::create_dir_all(dir.join("config.v2.json")).expect("config dir");
    plant(&dir.join("cdir-json.log"), b"log");
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "docker_logs", Status::Error);
    assert!(
        finding.evidence.contains("config.v2.json"),
        "{}",
        finding.evidence
    );
}

#[test]
fn unreadable_docker_config_needs_root() {
    let temp = TempRoot::new("docker-config-mode");
    let dir = temp.path().join("var/lib/docker/containers/lockedcfg");
    let config = dir.join("config.v2.json");
    plant(&config, b"{}");
    plant(&dir.join("lockedcfg-json.log"), b"log");
    chmod(&config, 0o000);
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "docker_logs", Status::NeedsRoot);
    assert!(
        finding.evidence.contains("config.v2.json"),
        "{}",
        finding.evidence
    );
    assert!(finding.hint.contains("run with sudo to see"));
}

#[test]
fn unreadable_container_dir_needs_root() {
    let temp = TempRoot::new("docker-container-mode");
    let dir = temp.path().join("var/lib/docker/containers/locked");
    fs::create_dir_all(&dir).expect("container");
    chmod(&dir, 0o000);
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "docker_logs", Status::NeedsRoot);
    assert!(
        finding
            .evidence
            .contains("/var/lib/docker/containers/locked"),
        "{}",
        finding.evidence
    );
}

#[test]
fn unreadable_containers_dir_needs_root() {
    let temp = TempRoot::new("docker-containers-mode");
    let containers = temp.path().join("var/lib/docker/containers");
    fs::create_dir_all(&containers).expect("containers");
    chmod(&containers, 0o000);
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "docker_logs", Status::NeedsRoot);
    assert!(
        finding.evidence.contains("/var/lib/docker/containers"),
        "{}",
        finding.evidence
    );
}

#[test]
fn docker_root_that_is_a_file_is_an_error() {
    let temp = TempRoot::new("docker-file");
    plant(&temp.path().join("var/lib/docker"), b"nope");
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "docker_logs", Status::Error);
    assert!(
        finding.evidence.contains("cannot read /var/lib/docker"),
        "{}",
        finding.evidence
    );
}

#[test]
fn containers_path_that_is_a_file_is_an_error() {
    let temp = TempRoot::new("docker-containers-file");
    plant(&temp.path().join("var/lib/docker/containers"), b"nope");
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "docker_logs", Status::Error);
    assert!(
        finding
            .evidence
            .contains("cannot read /var/lib/docker/containers"),
        "{}",
        finding.evidence
    );
}

struct ZeroProbe;

impl FsProbe for ZeroProbe {
    fn statvfs(&self, path: &Path) -> io::Result<FsStats> {
        assert_ne!(path, Path::new("/"), "statvfs on the host root");
        Ok(FsStats {
            blocks: 5,
            blocks_free: 0,
            block_size: 1,
            files: 0,
            files_free: 0,
        })
    }
}

#[test]
fn inodes_skip_filesystems_without_an_inode_limit() {
    let temp = TempRoot::new("inodes-zero");
    mountinfo(
        temp.path(),
        "22 1 8:1 / / rw,relatime - ext4 /dev/sda1 rw\n",
    );
    let report = scan(&Root::new(temp.path()), &ZeroProbe);
    let finding = one(&report, "inodes", Status::Ok);
    assert!(
        finding
            .evidence
            .contains("no mount reports a finite inode count"),
        "{}",
        finding.evidence
    );
    assert_eq!(finding.inodes, Some(0));
}

struct MixedProbe {
    zero: PathBuf,
    used: PathBuf,
}

impl FsProbe for MixedProbe {
    fn statvfs(&self, path: &Path) -> io::Result<FsStats> {
        let (files, files_free) = if path == self.zero {
            (0, 0)
        } else if path == self.used {
            (100, 150)
        } else {
            panic!("statvfs left the fixture: {}", path.display());
        };
        Ok(FsStats {
            blocks: 1,
            blocks_free: 1,
            block_size: 1,
            files,
            files_free,
        })
    }
}

#[test]
fn inodes_skip_a_zero_count_and_clamp_free_above_total() {
    let temp = TempRoot::new("inodes-mixed");
    let root = Root::new(temp.path());
    mountinfo(
        temp.path(),
        "22 1 8:1 / /zero rw - ext4 /dev/sda1 rw\n\
         26 22 8:2 / /used rw - ext4 /dev/sda2 rw\n",
    );
    let report = scan(
        &root,
        &MixedProbe {
            zero: root.join("/zero"),
            used: root.join("/used"),
        },
    );
    let finding = one(&report, "inodes", Status::Ok);
    assert!(
        finding.evidence.contains("mount /used "),
        "{}",
        finding.evidence
    );
    assert!(finding.evidence.contains("(0%)"), "{}", finding.evidence);
    assert_eq!(finding.inodes, Some(0));
}

struct SplitProbe {
    denied: PathBuf,
}

impl FsProbe for SplitProbe {
    fn statvfs(&self, path: &Path) -> io::Result<FsStats> {
        if path == self.denied {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"))
        } else {
            Err(io::Error::other("stat failed"))
        }
    }
}

#[test]
fn probe_permission_and_io_errors_stay_on_that_mount() {
    let temp = TempRoot::new("probe-errors");
    let root = Root::new(temp.path());
    mountinfo(
        temp.path(),
        "22 1 8:1 / /denied rw - ext4 /dev/sda1 rw\n\
         26 22 8:2 / /broken rw - ext4 /dev/sda2 rw\n",
    );
    let report = scan(
        &root,
        &SplitProbe {
            denied: root.join("/denied"),
        },
    );

    let inode_denied = one(&report, "inodes", Status::NeedsRoot);
    assert!(inode_denied.evidence.contains("/denied"));
    let inode_broken = one(&report, "inodes", Status::Error);
    assert!(
        inode_broken.evidence.contains("/broken"),
        "{}",
        inode_broken.evidence
    );
    assert!(inode_broken.evidence.contains("stat failed"));

    let df_denied = one(&report, "df_du_gap", Status::NeedsRoot);
    assert!(df_denied.evidence.contains("/denied"));
    assert!(df_denied.hint.contains("run with sudo to see"));
    let df_broken = one(&report, "df_du_gap", Status::Error);
    assert!(
        df_broken
            .evidence
            .contains("cannot read filesystem usage for /broken"),
        "{}",
        df_broken.evidence
    );
    assert!(df_broken.evidence.contains("stat failed"));
}

#[test]
fn empty_mountinfo_reports_nothing_to_compare() {
    let temp = TempRoot::new("mountinfo-empty");
    mountinfo(temp.path(), "");
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let inodes = one(&report, "inodes", Status::Ok);
    assert!(inodes
        .evidence
        .contains("no mount reports a finite inode count"));
    let gap = one(&report, "df_du_gap", Status::Ok);
    assert!(gap.evidence.contains("no mounts found"), "{}", gap.evidence);
}

#[test]
fn mountinfo_directory_is_an_error() {
    let temp = TempRoot::new("mountinfo-dir");
    fs::create_dir_all(temp.path().join("proc/self/mountinfo")).expect("dir");
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    for check in ["inodes", "df_du_gap"] {
        let finding = one(&report, check, Status::Error);
        assert!(
            finding
                .evidence
                .contains("cannot read /proc/self/mountinfo"),
            "{check}: {}",
            finding.evidence
        );
    }
}

struct GapProbe {
    data: PathBuf,
    gone: PathBuf,
    proc_mount: PathBuf,
}

impl FsProbe for GapProbe {
    fn statvfs(&self, path: &Path) -> io::Result<FsStats> {
        let blocks = if path == self.data {
            1010
        } else if path == self.gone {
            80
        } else if path == self.proc_mount {
            0
        } else {
            panic!("statvfs left the fixture: {}", path.display());
        };
        Ok(FsStats {
            blocks,
            blocks_free: 0,
            block_size: 1,
            files: 100,
            files_free: 90,
        })
    }
}

#[test]
fn df_ignores_symlinks_sockets_and_missing_mount_dirs() {
    let temp = TempRoot::new("df-walk");
    let outside = std::env::temp_dir().join(format!("dfd-du-outside-{}", process::id()));
    plant(&outside, vec![b'o'; 1000]);
    let _cleanup = RemoveFile(outside.clone());

    // `/proc` is its own mount so the mountinfo file is not part of `/data`'s du.
    mountinfo(
        temp.path(),
        "22 1 8:1 / /data rw,relatime - ext4 /dev/sda1 rw\n\
         23 22 0:3 / /proc rw,relatime - proc proc rw\n\
         30 22 8:2 / /gone rw - ext4 /dev/sda2 rw\n",
    );
    let data = temp.path().join("data");
    plant(&data.join("top"), vec![b't'; 10]);
    fs::create_dir(data.join("sub")).expect("sub");
    std::os::unix::fs::symlink(&outside, data.join("link")).expect("symlink");
    let _listener = std::os::unix::net::UnixListener::bind(data.join("sock")).expect("sock");

    let root = Root::new(temp.path());
    let report = scan(
        &root,
        &GapProbe {
            data: root.join("/data"),
            gone: root.join("/gone"),
            proc_mount: root.join("/proc"),
        },
    );
    let data_finding = rows(&report, "df_du_gap", Status::Warn)
        .into_iter()
        .find(|finding| finding.evidence.starts_with("/data:"))
        .expect("data mount");
    assert_eq!(data_finding.bytes, Some(1000), "{}", data_finding.evidence);
    assert!(
        data_finding.evidence.contains("du 10 bytes"),
        "{}",
        data_finding.evidence
    );
    assert!(!data_finding
        .evidence
        .contains(outside.to_str().unwrap_or("")));

    let gone = rows(&report, "df_du_gap", Status::Warn)
        .into_iter()
        .find(|finding| finding.evidence.starts_with("/gone:"))
        .expect("missing mount");
    assert_eq!(gone.bytes, Some(80), "{}", gone.evidence);
    assert!(gone.evidence.contains("du 0 bytes"), "{}", gone.evidence);
}

#[test]
fn deleted_open_skips_entries_that_are_not_deleted_files() {
    let temp = TempRoot::new("deleted-skips");
    let proc_dir = temp.path().join("proc");
    fs::create_dir_all(proc_dir.join("not-a-pid")).expect("not a pid");
    plant(&proc_dir.join("not-a-pid/note"), b"skip");
    fs::create_dir_all(proc_dir.join("0")).expect("pid 0");
    fs::create_dir_all(proc_dir.join("4294967296")).expect("overflow pid");
    fs::create_dir_all(proc_dir.join(OsString::from_vec(vec![0xff]))).expect("odd name");
    plant(&proc_dir.join("12/fd"), b"not a directory");
    let fd = proc_dir.join("14/fd");
    fs::create_dir_all(&fd).expect("fd");
    plant(&fd.join("1"), b"plain");
    std::os::unix::fs::symlink("missing (deleted)", fd.join("2")).expect("dangling");
    std::os::unix::fs::symlink("keep.log", fd.join("3")).expect("live");

    let report = scan(&Root::new(temp.path()), &PanicProbe);
    assert!(
        report
            .findings
            .iter()
            .filter(|finding| finding.check == "deleted_open")
            .all(|finding| finding.status != Status::Warn
                && finding.status != Status::Error
                && finding.status != Status::NeedsRoot),
        "{:?}",
        report.findings
    );
}

#[test]
fn proc_that_is_a_file_is_an_error() {
    let temp = TempRoot::new("proc-file");
    plant(&temp.path().join("proc"), b"nope");
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "deleted_open", Status::Error);
    assert!(
        finding.evidence.contains("cannot read /proc"),
        "{}",
        finding.evidence
    );
}

#[test]
fn unreadable_proc_needs_root() {
    let temp = TempRoot::new("proc-mode");
    let proc_dir = temp.path().join("proc");
    fs::create_dir_all(&proc_dir).expect("proc");
    chmod(&proc_dir, 0o000);
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let finding = one(&report, "deleted_open", Status::NeedsRoot);
    assert!(finding.evidence.contains("cannot read /proc"));
    assert!(finding.hint.contains("run with sudo to see"));
    assert!(finding.hint.contains("deleted-but-open"));
}

struct RemoveFile(PathBuf);

impl Drop for RemoveFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
