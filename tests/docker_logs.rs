//! Docker json-file log check on fixtures. No docker socket and no docker CLI.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process;

use disk_full_doctor::{scan, FsProbe, FsStats, Root, Status};

struct PanicProbe;

impl FsProbe for PanicProbe {
    fn statvfs(&self, path: &Path) -> std::io::Result<FsStats> {
        panic!("docker_logs must not statvfs {}", path.display());
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/docker_logs")
        .join(name)
}

fn docker_findings(
    report: &disk_full_doctor::Report,
    status: Status,
) -> Vec<&disk_full_doctor::Finding> {
    report
        .findings
        .iter()
        .filter(|finding| finding.check == "docker_logs" && finding.status == status)
        .collect()
}

#[test]
fn large_log_without_max_size_warns_with_bytes() {
    let root_path = fixture("unlimited");
    let report = scan(&Root::new(&root_path), &PanicProbe);
    let warns = docker_findings(&report, Status::Warn);
    assert_eq!(warns.len(), 1, "findings: {:?}", report.findings);
    assert_eq!(warns[0].bytes, Some(65536));
    assert!(
        warns[0]
            .evidence
            .contains("/var/lib/docker/containers/abc123def/abc123def-json.log"),
        "evidence: {}",
        warns[0].evidence
    );
    assert!(
        warns[0].evidence.contains("no max-size"),
        "evidence: {}",
        warns[0].evidence
    );
    assert!(
        !warns[0].evidence.contains(root_path.to_str().unwrap_or("")),
        "evidence leaked the fixture path: {}",
        warns[0].evidence
    );
}

#[test]
fn container_max_size_is_ok() {
    // Fixture daemon.json has no max-size. The container config.v2.json does.
    let report = scan(&Root::new(fixture("limited")), &PanicProbe);
    let oks = docker_findings(&report, Status::Ok);
    assert_eq!(oks.len(), 1, "findings: {:?}", report.findings);
    assert_eq!(oks[0].bytes, Some(128));
    assert!(
        oks[0].evidence.contains("max-size is set"),
        "evidence: {}",
        oks[0].evidence
    );
    assert!(docker_findings(&report, Status::Warn).is_empty());
}

const DAEMON_WITH_MAX_SIZE: &str =
    "{\"log-driver\": \"json-file\", \"log-opts\": {\"max-size\": \"10m\"}}\n";
const CONFIG_WITHOUT_MAX_SIZE: &str =
    "{\"HostConfig\": {\"LogConfig\": {\"Type\": \"json-file\", \"Config\": {}}}}\n";

fn plant_container(root: &Path, id: &str, config: &[u8], log_bytes: u64, fill: bool) {
    let etc = root.join("etc/docker");
    fs::create_dir_all(&etc).expect("etc/docker");
    fs::write(etc.join("daemon.json"), DAEMON_WITH_MAX_SIZE).expect("daemon.json");
    let dir = root.join("var/lib/docker/containers").join(id);
    fs::create_dir_all(&dir).expect("container dir");
    fs::write(dir.join("config.v2.json"), config).expect("config.v2.json");
    let log_path = dir.join(format!("{id}-json.log"));
    if fill {
        fs::write(&log_path, vec![b'x'; log_bytes as usize]).expect("log bytes");
    } else {
        let file = fs::File::create(&log_path).expect("log");
        file.set_len(log_bytes).expect("log length");
    }
}

fn assert_unlimited_log(report: &disk_full_doctor::Report, id: &str, bytes: u64) {
    let warns = docker_findings(report, Status::Warn);
    assert_eq!(warns.len(), 1, "findings: {:?}", report.findings);
    assert_eq!(warns[0].bytes, Some(bytes));
    let logical = format!("/var/lib/docker/containers/{id}/{id}-json.log");
    assert!(
        warns[0].evidence.contains(&logical),
        "evidence: {}",
        warns[0].evidence
    );
    assert!(
        warns[0].evidence.contains("no max-size"),
        "evidence: {}",
        warns[0].evidence
    );
    assert!(
        !warns[0].evidence.contains("max-size is set"),
        "evidence: {}",
        warns[0].evidence
    );
    assert!(
        warns[0].hint.contains("Unlimited json-file logs"),
        "hint: {}",
        warns[0].hint
    );
    assert!(docker_findings(report, Status::Ok).is_empty());
}

#[test]
fn daemon_max_size_does_not_limit_an_old_container() {
    // Review repro: daemon.json has max-size, this container's config.v2.json
    // was frozen without one, and the json log is 9000 bytes.
    let temp = TempRoot::new("docker-old-container");
    plant_container(
        temp.path(),
        "oldcontainer",
        CONFIG_WITHOUT_MAX_SIZE.as_bytes(),
        9000,
        true,
    );
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    assert_unlimited_log(&report, "oldcontainer", 9000);
}

#[test]
fn empty_container_config_is_not_a_limit() {
    let temp = TempRoot::new("docker-empty-config");
    plant_container(temp.path(), "emptycfg", b"", 64, true);
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    assert_unlimited_log(&report, "emptycfg", 64);
}

#[test]
fn huge_log_without_container_max_size_warns_with_byte_size() {
    const HUGE_LOG_BYTES: u64 = 20 * 1024 * 1024 * 1024;
    let temp = TempRoot::new("docker-huge-log");
    plant_container(
        temp.path(),
        "hugelog",
        CONFIG_WITHOUT_MAX_SIZE.as_bytes(),
        HUGE_LOG_BYTES,
        false,
    );
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    assert_unlimited_log(&report, "hugelog", HUGE_LOG_BYTES);
}

#[test]
fn broken_container_config_is_not_a_limit() {
    let temp = TempRoot::new("docker-broken-config");
    plant_container(temp.path(), "brokencfg", b"{this is not json", 32, true);
    let report = scan(&Root::new(temp.path()), &PanicProbe);
    assert_unlimited_log(&report, "brokencfg", 32);
}

#[test]
fn missing_docker_root_is_not_present() {
    let report = scan(&Root::new(fixture("absent")), &PanicProbe);
    let missing = docker_findings(&report, Status::NotPresent);
    assert_eq!(missing.len(), 1, "findings: {:?}", report.findings);
    assert!(
        missing[0].evidence.contains("/var/lib/docker"),
        "evidence: {}",
        missing[0].evidence
    );
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
fn unreadable_docker_root_needs_root() {
    let temp = TempRoot::new("docker-logs");
    let docker = temp.path.join("var/lib/docker");
    fs::create_dir_all(&docker).expect("docker dir");
    let mut perms = fs::metadata(&docker).expect("meta").permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&docker, perms).expect("chmod");

    let report = scan(&Root::new(temp.path()), &PanicProbe);
    let denied = docker_findings(&report, Status::NeedsRoot);
    assert_eq!(denied.len(), 1, "findings: {:?}", report.findings);
    assert!(
        denied[0].evidence.contains("/var/lib/docker"),
        "evidence: {}",
        denied[0].evidence
    );
    assert!(
        denied[0].hint.contains("run with sudo to see"),
        "hint: {}",
        denied[0].hint
    );
    assert!(
        denied[0].hint.contains("Docker"),
        "hint: {}",
        denied[0].hint
    );
}
