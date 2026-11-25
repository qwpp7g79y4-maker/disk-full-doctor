//! The binary renders a fixture root in text and JSON.
//!
//! The fixture is not `/` and contains no host proc. The probe used by the
//! library test rejects any `statvfs` path outside that fixture.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

use disk_full_doctor::{scan, FsProbe, FsStats, Root};

struct RejectOutside {
    fixture: PathBuf,
}

impl FsProbe for RejectOutside {
    fn statvfs(&self, path: &Path) -> std::io::Result<FsStats> {
        assert!(
            path.starts_with(&self.fixture),
            "statvfs left the fixture: {}",
            path.display()
        );
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "fixture has no injected statvfs values",
        ))
    }
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/empty")
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_disk-full-doctor"))
        .args(args)
        .output()
        .expect("run disk-full-doctor")
}

#[test]
fn fixture_root_renders_text_and_json() {
    let fixture = fixture();
    assert!(fixture.is_dir());
    assert_ne!(fixture, PathBuf::from("/"));

    let root = Root::new(&fixture);
    let proc_path = root.join("/proc/self/mountinfo");
    assert_eq!(proc_path, fixture.join("proc/self/mountinfo"));
    assert_ne!(proc_path, PathBuf::from("/proc/self/mountinfo"));

    let probe = RejectOutside {
        fixture: fixture.clone(),
    };
    let report = scan(&root, &probe);
    let fixture_arg = fixture.to_str().expect("fixture path is utf-8");

    let text = run(&["--root", fixture_arg]);
    assert!(
        text.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&text.stderr)
    );
    let text_stdout = String::from_utf8(text.stdout).expect("text stdout is utf-8");
    assert_eq!(text_stdout, report.render_text());
    assert!(text_stdout.starts_with("disk-full-doctor\n"));

    let json = run(&["--json", "--root", fixture_arg]);
    assert!(
        json.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&json.stderr)
    );
    let json_stdout = String::from_utf8(json.stdout).expect("json stdout is utf-8");
    assert_eq!(json_stdout, report.render_json());
    let value: serde_json::Value = serde_json::from_str(&json_stdout).expect("json report");
    assert!(value
        .get("findings")
        .is_some_and(|findings| findings.is_array()));
}

#[test]
fn exit_code_is_0_when_nothing_actionable_was_found() {
    let fixture = fixture();
    let output = run(&["--root", fixture.to_str().expect("utf-8")]);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn exit_code_is_1_when_a_check_warns() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/docker_logs/unlimited");
    let output = run(&["--root", fixture.to_str().expect("utf-8")]);
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn exit_code_is_2_on_a_usage_error() {
    let output = run(&["--not-a-real-flag"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn help_exits_zero() {
    let output = run(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("help is utf-8");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("--json"), "{stdout}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(!stdout.contains("panicked"));
}

#[test]
fn built_temp_tree_renders_text_and_json() {
    let temp = rich_tree();
    let root = temp.path();
    assert!(root.is_dir());
    assert_ne!(root, Path::new("/"));
    let arg = root.to_str().expect("temp root is utf-8");
    assert_ne!(arg, "/");

    let text = run(&["--root", arg]);
    assert_eq!(
        text.status.code(),
        Some(1),
        "stderr: {}",
        lossy(&text.stderr)
    );
    let text_stdout = String::from_utf8(text.stdout).expect("text stdout is utf-8");
    assert!(text_stdout.starts_with("disk-full-doctor\n"));
    assert!(text_stdout.contains("pid 4242 "));
    assert!(text_stdout.contains("(deleted)"));
    assert!(text_stdout.contains("SystemMaxUse=10M"));
    assert!(text_stdout.contains("use 100 bytes"));
    assert!(text_stdout.contains("use 44 bytes"));
    assert!(text_stdout.contains("/var/lib/docker/containers/cid/cid-json.log has no max-size"));
    assert!(text_stdout.contains("statvfs used"));
    assert!(
        text_stdout.contains("mount / ") || text_stdout.contains("mount /var "),
        "{text_stdout}"
    );
    assert!(
        !text_stdout.contains(arg),
        "text report leaked the temp root: {text_stdout}"
    );
    assert!(!lossy(&text.stderr).contains("panicked"));

    let json = run(&["--json", "--root", arg]);
    assert_eq!(
        json.status.code(),
        Some(1),
        "stderr: {}",
        lossy(&json.stderr)
    );
    let json_stdout = String::from_utf8(json.stdout).expect("json stdout is utf-8");
    let value: serde_json::Value = serde_json::from_str(&json_stdout).expect("json report");
    let findings = value["findings"].as_array().expect("findings array");
    assert!(findings.iter().any(|finding| {
        finding["check"] == "docker_logs"
            && finding["status"] == "Warn"
            && finding["bytes"].as_u64() == Some(80)
    }));
    assert!(findings.iter().any(|finding| {
        finding["check"] == "journald" && finding["bytes"].as_u64() == Some(100)
    }));
    assert!(findings.iter().any(|finding| {
        finding["check"] == "buildkit" && finding["bytes"].as_u64() == Some(44)
    }));
    assert!(findings.iter().any(|finding| {
        finding["check"] == "deleted_open"
            && finding["status"] == "Warn"
            && finding["bytes"].as_u64() == Some(16)
    }));
    assert!(json_stdout.contains("\"df_du_gap\""));
    assert!(json_stdout.contains("\"inodes\""));
    assert!(!json_stdout.contains(arg), "json leaked the temp root");
    assert!(!lossy(&json.stderr).contains("panicked"));
}

#[test]
fn missing_root_exits_nonzero_with_a_message_and_no_panic() {
    let missing = std::env::temp_dir().join(format!(
        "dfd-missing-root-{}-{}",
        process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    assert!(!missing.exists());
    assert_ne!(missing, PathBuf::from("/"));
    let arg = missing.to_str().expect("utf-8");

    for json in [false, true] {
        let mut args = Vec::new();
        if json {
            args.push("--json");
        }
        args.push("--root");
        args.push(arg);
        let output = run(&args);
        assert_ne!(output.status.code(), Some(0), "json={json}");
        let stdout = lossy(&output.stdout);
        let stderr = lossy(&output.stderr);
        assert!(!stderr.contains("panicked"), "{stderr}");
        assert!(!stdout.contains("panicked"), "{stdout}");
        assert!(
            stderr.contains("does not exist"),
            "stdout={stdout} stderr={stderr}"
        );
        assert!(
            stdout.is_empty(),
            "a missing root must not be scanned: {stdout}"
        );
    }
}

#[test]
fn unreadable_root_exits_nonzero_with_a_message_and_no_panic() {
    let temp = TempRoot::new("unreadable-root");
    assert_ne!(temp.path(), Path::new("/"));
    let mut perms = fs::metadata(temp.path()).expect("meta").permissions();
    perms.set_mode(0o000);
    fs::set_permissions(temp.path(), perms).expect("chmod");
    let arg = temp.path().to_str().expect("utf-8");

    for json in [false, true] {
        let mut args = Vec::new();
        if json {
            args.push("--json");
        }
        args.push("--root");
        args.push(arg);
        let output = run(&args);
        assert_ne!(output.status.code(), Some(0), "json={json}");
        let stdout = lossy(&output.stdout);
        let stderr = lossy(&output.stderr);
        assert!(!stderr.contains("panicked"), "{stderr}");
        assert!(!stdout.contains("panicked"), "{stdout}");
        let message = format!("{stdout}{stderr}");
        assert!(
            message.contains("NeedsRoot") || message.contains("cannot read"),
            "{message}"
        );
    }
}

#[test]
fn root_behind_an_unsearchable_parent_exits_nonzero_without_panic() {
    let temp = TempRoot::new("locked-parent");
    let parent = temp.path().join("parent");
    let child = parent.join("child");
    fs::create_dir_all(&child).expect("child");
    let mut perms = fs::metadata(&parent).expect("meta").permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&parent, perms).expect("chmod");
    assert_ne!(child, PathBuf::from("/"));

    let output = run(&["--root", child.to_str().expect("utf-8")]);
    assert_ne!(output.status.code(), Some(0));
    let stdout = lossy(&output.stdout);
    let stderr = lossy(&output.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(!stdout.contains("panicked"), "{stdout}");
    let message = format!("{stdout}{stderr}");
    assert!(
        message.contains("NeedsRoot") || message.contains("cannot read"),
        "{message}"
    );
}

#[test]
fn symlink_loop_root_exits_nonzero_without_panic() {
    let temp = TempRoot::new("loop-root");
    let loop_path = temp.path().join("loop");
    std::os::unix::fs::symlink("loop", &loop_path).expect("symlink");
    assert_ne!(loop_path, PathBuf::from("/"));

    let output = run(&["--json", "--root", loop_path.to_str().expect("utf-8")]);
    assert_ne!(output.status.code(), Some(0));
    let stdout = lossy(&output.stdout);
    let stderr = lossy(&output.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(!stdout.contains("panicked"), "{stdout}");
    assert!(
        stderr.contains("cannot read"),
        "stdout={stdout} stderr={stderr}"
    );
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("dfd-cli-{label}-{}", process::id()));
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

fn rich_tree() -> TempRoot {
    let temp = TempRoot::new("rich");
    let root = temp.path();

    fs::create_dir_all(root.join("proc/self")).expect("proc");
    fs::write(
        root.join("proc/self/mountinfo"),
        "22 1 8:1 / / rw,relatime - ext4 /dev/sda1 rw\n\
         26 22 8:2 / /var rw,relatime - ext4 /dev/sda2 rw\n",
    )
    .expect("mountinfo");
    fs::create_dir_all(root.join("proc/not-a-pid")).expect("not a pid");
    fs::write(root.join("proc/not-a-pid/note"), b"skip").expect("note");

    let fd = root.join("proc/4242/fd");
    fs::create_dir_all(&fd).expect("fd");
    fs::create_dir_all(root.join("payload")).expect("payload");
    fs::write(root.join("payload/app.log (deleted)"), vec![b'a'; 16]).expect("deleted file");
    fs::write(root.join("payload/keep.log"), b"keep").expect("keep");
    std::os::unix::fs::symlink("../../../payload/app.log (deleted)", fd.join("3")).expect("fd 3");
    std::os::unix::fs::symlink("../../../payload/keep.log", fd.join("4")).expect("fd 4");

    fs::create_dir_all(root.join("var/log/journal")).expect("journal");
    fs::write(root.join("var/log/journal/system.journal"), vec![b'j'; 100]).expect("journal bytes");
    fs::create_dir_all(root.join("etc/systemd")).expect("systemd");
    fs::write(
        root.join("etc/systemd/journald.conf"),
        "# comment\n\n; also\nSystemMaxUse=\nSystemMaxUse=10M\n",
    )
    .expect("journald.conf");

    fs::create_dir_all(root.join("var/lib/docker/buildkit/sub")).expect("buildkit");
    fs::write(root.join("var/lib/docker/buildkit/real"), vec![b'b'; 40]).expect("blob");
    fs::write(root.join("var/lib/docker/buildkit/sub/n"), vec![b'b'; 4]).expect("nested blob");

    let cid = root.join("var/lib/docker/containers/cid");
    fs::create_dir_all(&cid).expect("container");
    fs::write(cid.join("cid-json.log"), vec![b'l'; 80]).expect("log");
    fs::write(
        cid.join("config.v2.json"),
        b"{\"HostConfig\":{\"LogConfig\":{\"Config\":{}}}}\n",
    )
    .expect("config");
    fs::write(cid.join("hostname"), b"box").expect("hostname");

    temp
}
