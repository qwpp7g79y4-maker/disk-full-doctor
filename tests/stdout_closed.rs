//! A report that cannot be written must not exit 0.
//!
//! A full standard output is exit 2 and `/dev/null` is a normal sink (exit 0).
//!
//! A closed standard output cannot be told apart: the Rust runtime reopens a
//! closed descriptor 1 as `/dev/null` before `main` runs.

use std::path::PathBuf;
use std::process::{Command, Output};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/empty")
}

/// Runs the binary through `sh` so the redirection happens before `exec`.
fn run_with_stdout(redirect: &str) -> Output {
    Command::new("sh")
        .arg("-c")
        .arg(format!("exec \"$0\" --root \"$1\" {redirect}"))
        .arg(env!("CARGO_BIN_EXE_disk-full-doctor"))
        .arg(fixture())
        .output()
        .expect("run sh")
}

#[test]
fn dev_null_stays_exit_0() {
    let output = run_with_stdout(">/dev/null");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn full_stdout_is_exit_2_without_a_panic() {
    let output = run_with_stdout(">/dev/full");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
}
