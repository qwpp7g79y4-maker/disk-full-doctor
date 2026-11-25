//! Size of Docker json-file container logs, and whether `max-size` is set.
//!
//! Reads `<root>/var/lib/docker/containers/*/*-json.log` and each container's
//! `config.v2.json`. Docker writes the log options into that file when the
//! container is created and does not update it when `/etc/docker/daemon.json`
//! changes later, so a daemon-wide `max-size` is not a limit on an existing
//! log. No Docker socket and no `docker` CLI.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value;

use crate::{Finding, Root, Status};

use super::util::is_permission;

const CHECK: &str = "docker_logs";
const UNLIMITED_HINT: &str = "Set log-opts max-size in /etc/docker/daemon.json or the container \
LogConfig. Unlimited json-file logs can fill the disk. This tool only reports and does not \
change anything.";

enum JsonRead {
    Value(Value),
    Missing,
    Invalid,
    Denied,
    Failed(io::Error),
}

pub(crate) fn check(root: &Root) -> Vec<Finding> {
    let docker = root.join("/var/lib/docker");
    if let Err(finding) = ensure_readable_dir(&docker, "/var/lib/docker") {
        return vec![finding];
    }

    let containers = docker.join("containers");
    match fs::read_dir(&containers) {
        Ok(entries) => scan_containers(root, entries),
        Err(err) if err.kind() == io::ErrorKind::NotFound => vec![no_logs()],
        Err(err) if is_permission(&err) => vec![needs_root(
            "/var/lib/docker/containers",
            "Docker json-file container logs (cannot read /var/lib/docker/containers)",
        )],
        Err(err) => vec![error_finding(format!(
            "cannot read /var/lib/docker/containers: {err}"
        ))],
    }
}

fn scan_containers(root: &Root, entries: fs::ReadDir) -> Vec<Finding> {
    let mut findings = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) if is_permission(&err) => {
                findings.push(needs_root(
                    "/var/lib/docker/containers",
                    "Docker json-file container logs (cannot read /var/lib/docker/containers)",
                ));
                continue;
            }
            Err(err) => {
                findings.push(error_finding(format!(
                    "cannot read a container directory: {err}"
                )));
                continue;
            }
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        findings.extend(scan_container(root, &entry.path(), &id));
    }
    if findings.is_empty() {
        findings.push(no_logs());
    }
    findings
}

fn scan_container(root: &Root, dir: &Path, id: &str) -> Vec<Finding> {
    let _ = root;
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if is_permission(&err) => {
            return vec![needs_root(
                &format!("/var/lib/docker/containers/{id}"),
                &format!(
                    "Docker json-file container logs for {id} (cannot read /var/lib/docker/containers/{id})"
                ),
            )];
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(err) => {
            return vec![error_finding(format!(
                "cannot read /var/lib/docker/containers/{id}: {err}"
            ))]
        }
    };

    let config_limit = match read_json(&dir.join("config.v2.json")) {
        JsonRead::Value(value) => {
            has_string(&value, &["HostConfig", "LogConfig", "Config", "max-size"])
        }
        JsonRead::Missing | JsonRead::Invalid => false,
        JsonRead::Denied => {
            return vec![needs_root(
                &format!("/var/lib/docker/containers/{id}/config.v2.json"),
                &format!(
                    "Docker log options for {id} (cannot read /var/lib/docker/containers/{id}/config.v2.json)"
                ),
            )];
        }
        JsonRead::Failed(err) => {
            return vec![error_finding(format!(
                "cannot read /var/lib/docker/containers/{id}/config.v2.json: {err}"
            ))]
        }
    };
    // config.v2.json is the log config frozen at container creation. A
    // max-size that exists only in daemon.json does not apply to this log.
    let limited = config_limit;

    let mut findings = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) if is_permission(&err) => {
                findings.push(needs_root(
                    &format!("/var/lib/docker/containers/{id}"),
                    &format!(
                        "Docker json-file container logs for {id} (cannot read /var/lib/docker/containers/{id})"
                    ),
                ));
                continue;
            }
            Err(_) => continue,
        };
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !name.ends_with("-json.log") {
            continue;
        }
        let meta = match fs::symlink_metadata(entry.path()) {
            Ok(meta) => meta,
            Err(err) if is_permission(&err) => {
                findings.push(needs_root(
                    &format!("/var/lib/docker/containers/{id}/{name}"),
                    &format!(
                        "Docker json-file container log {name} (cannot read /var/lib/docker/containers/{id}/{name})"
                    ),
                ));
                continue;
            }
            Err(_) => continue,
        };
        // A symlink could point outside the root prefix. Count only regular files.
        if meta.file_type().is_symlink() || !meta.is_file() {
            continue;
        }
        let logical = format!("/var/lib/docker/containers/{id}/{name}");
        if limited {
            findings.push(Finding::new(
                CHECK,
                Status::Ok,
                Some(meta.len()),
                None,
                format!("{logical} max-size is set"),
                "json-file logs have a max-size limit.",
            ));
        } else {
            findings.push(Finding::new(
                CHECK,
                Status::Warn,
                Some(meta.len()),
                None,
                format!("{logical} has no max-size"),
                UNLIMITED_HINT,
            ));
        }
    }
    findings
}

/// `metadata` succeeds on a mode `000` directory the caller owns, so the
/// readable check is `read_dir`.
fn ensure_readable_dir(path: &Path, logical: &str) -> Result<(), Finding> {
    match fs::read_dir(path) {
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(not_present()),
        Err(err) if is_permission(&err) => Err(needs_root(
            logical,
            "Docker json-file container logs (cannot read /var/lib/docker)",
        )),
        Err(err) => Err(error_finding(format!("cannot read {logical}: {err}"))),
    }
}

fn read_json(path: &Path) -> JsonRead {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(value) => JsonRead::Value(value),
            Err(_) => JsonRead::Invalid,
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => JsonRead::Missing,
        Err(err) if is_permission(&err) => JsonRead::Denied,
        Err(err) => JsonRead::Failed(err),
    }
}

fn has_string(value: &Value, keys: &[&str]) -> bool {
    let mut current = value;
    for key in keys {
        match current.get(*key) {
            Some(next) => current = next,
            None => return false,
        }
    }
    current.as_str().is_some_and(|text| !text.trim().is_empty())
}

fn not_present() -> Finding {
    Finding::new(
        CHECK,
        Status::NotPresent,
        None,
        None,
        "/var/lib/docker is not present",
        "Docker's data root is not present, so container logs are not listed.",
    )
}

fn no_logs() -> Finding {
    Finding::new(
        CHECK,
        Status::Ok,
        Some(0),
        None,
        "no json-file container logs",
        "No *-json.log files were found under /var/lib/docker/containers.",
    )
}

fn needs_root(path: &str, what: &str) -> Finding {
    Finding::new(
        CHECK,
        Status::NeedsRoot,
        None,
        None,
        format!("cannot read {path}"),
        format!("run with sudo to see {what}"),
    )
}

fn error_finding(evidence: String) -> Finding {
    Finding::new(
        CHECK,
        Status::Error,
        None,
        None,
        evidence,
        "The check could not finish reading Docker container logs.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn max_size_comes_from_daemon_or_container_config() {
        let daemon = json!({"log-driver": "json-file", "log-opts": {"max-size": "10m"}});
        let bare_daemon = json!({"log-driver": "json-file"});
        let container = json!({
            "HostConfig": {"LogConfig": {"Type": "json-file", "Config": {"max-size": "10m"}}}
        });
        let bare_container = json!({
            "HostConfig": {"LogConfig": {"Type": "json-file", "Config": {}}}
        });
        assert!(has_string(&daemon, &["log-opts", "max-size"]));
        assert!(!has_string(&bare_daemon, &["log-opts", "max-size"]));
        assert!(has_string(
            &container,
            &["HostConfig", "LogConfig", "Config", "max-size"]
        ));
        assert!(!has_string(
            &bare_container,
            &["HostConfig", "LogConfig", "Config", "max-size"]
        ));
        assert!(!has_string(
            &json!({"log-opts": {"max-size": "  "}}),
            &["log-opts", "max-size"]
        ));
        assert!(!has_string(
            &json!({"log-opts": {"max-size": 10}}),
            &["log-opts", "max-size"]
        ));
        assert!(!has_string(&json!("nope"), &["log-opts", "max-size"]));
    }
}
