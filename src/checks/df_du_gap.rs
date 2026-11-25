//! Per mount, `statvfs` used bytes next to a `du`-style sum of file sizes
//! under that mount, and the gap between the two. The walk never crosses
//! into a nested mount and never follows a symlink.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{Finding, FsProbe, Root, Status};

use super::mountinfo;
use super::util::is_permission;

const CHECK: &str = "df_du_gap";
const HINT: &str = "statvfs reports more used space than the sum of files under this mount. The \
gap often means a deleted-but-open file, or files hidden under another mount's mountpoint. This \
tool only reports and does not change anything.";

pub(crate) fn check(root: &Root, probe: &dyn FsProbe) -> Vec<Finding> {
    let mounts = match mountinfo::read_mount_points(root) {
        Ok(mounts) => mounts,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return vec![not_present()],
        Err(err) if is_permission(&err) => return vec![needs_root_mountinfo()],
        Err(err) => {
            return vec![error_finding(format!(
                "cannot read /proc/self/mountinfo: {err}"
            ))]
        }
    };

    let mut findings = Vec::new();
    for mount in &mounts {
        let stat_path = root.join(mount);
        let used = match probe.statvfs(&stat_path) {
            Ok(stats) => stats
                .blocks
                .saturating_sub(stats.blocks_free)
                .saturating_mul(stats.block_size),
            Err(err) if is_permission(&err) => {
                findings.push(needs_root(mount));
                continue;
            }
            Err(err) => {
                findings.push(error_finding(format!(
                    "cannot read filesystem usage for {mount}: {err}"
                )));
                continue;
            }
        };

        match du_walk(root, mount, &mounts) {
            Ok(du) => {
                let gap = used.saturating_sub(du);
                let status = if gap > 0 { Status::Warn } else { Status::Ok };
                findings.push(Finding::new(
                    CHECK,
                    status,
                    Some(gap),
                    None,
                    format!("{mount}: statvfs used {used} bytes, du {du} bytes, gap {gap} bytes"),
                    HINT,
                ));
            }
            Err(WalkError::Denied(path)) => findings.push(needs_root(&path)),
            Err(WalkError::Other(err)) => {
                findings.push(error_finding(format!("cannot walk {mount}: {err}")))
            }
        }
    }

    if findings.is_empty() {
        findings.push(Finding::new(
            CHECK,
            Status::Ok,
            None,
            None,
            "no mounts found",
            "No mount points were read from mountinfo.",
        ));
    }
    findings
}

enum WalkError {
    Denied(String),
    Other(io::Error),
}

/// Sum of regular file sizes under the mount's root directory. Stops at a
/// subdirectory that belongs to a different, nested mount, and never
/// follows a symlink.
fn du_walk(root: &Root, mount: &str, mounts: &[String]) -> Result<u64, WalkError> {
    let start = root.join(mount);
    let mut total = 0u64;
    let mut stack = vec![start];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) if is_permission(&err) => {
                return Err(WalkError::Denied(logical_path(root, &dir)))
            }
            Err(err) => return Err(WalkError::Other(err)),
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) if is_permission(&err) => {
                    return Err(WalkError::Denied(logical_path(root, &dir)))
                }
                Err(err) => return Err(WalkError::Other(err)),
            };
            let entry_path = entry.path();
            let meta = match fs::symlink_metadata(&entry_path) {
                Ok(meta) => meta,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) if is_permission(&err) => {
                    return Err(WalkError::Denied(logical_path(root, &entry_path)))
                }
                Err(err) => return Err(WalkError::Other(err)),
            };
            if meta.is_symlink() {
                continue;
            }
            if meta.is_dir() {
                let child_logical = logical_path(root, &entry_path);
                if mountinfo::mount_for(&child_logical, mounts) == mount {
                    stack.push(entry_path);
                }
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

/// Reverse of [`Root::join`]: the logical absolute path for something under
/// the root prefix.
fn logical_path(root: &Root, path: &Path) -> String {
    let relative = path.strip_prefix(root.prefix()).unwrap_or(path);
    let mut logical = PathBuf::from("/");
    logical.push(relative);
    logical.to_string_lossy().into_owned()
}

fn not_present() -> Finding {
    Finding::new(
        CHECK,
        Status::NotPresent,
        None,
        None,
        "mountinfo is not present",
        "There is no procfs under the root prefix, so mount usage cannot be compared to du.",
    )
}

fn needs_root_mountinfo() -> Finding {
    needs_root_evidence("/proc/self/mountinfo", "mount list (cannot read mountinfo)")
}

fn needs_root(mount: &str) -> Finding {
    needs_root_evidence(mount, &format!("du/df comparison for {mount}"))
}

fn needs_root_evidence(path: &str, what: &str) -> Finding {
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
        "The check could not finish comparing statvfs usage to file sizes.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_path_reverses_root_join() {
        let root = Root::new("/fixtures/root");
        let physical = root.join("/var/lib/docker");
        assert_eq!(logical_path(&root, &physical), "/var/lib/docker");
    }

    #[test]
    fn logical_path_keeps_a_path_outside_the_prefix() {
        let root = Root::new("/fixtures/root");
        assert_eq!(logical_path(&root, Path::new("/other")), "/other");
        assert_eq!(
            logical_path(&root, Path::new("relative/nope")),
            "/relative/nope"
        );
    }
}
