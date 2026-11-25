//! Deleted files that a process still has open.
//!
//! Reads `<root>/proc/<pid>/fd/*`. A `readlink` target ending in ` (deleted)`
//! is a deleted file. The byte size is `stat` of that fd entry (on procfs,
//! `stat` follows the magic link to the still-open inode). Totals are per
//! process and per mount. The check never calls `lsof` and never signals a
//! process.

use std::fs;
use std::io;
use std::path::Path;

use crate::{Finding, Root, Status};

use super::mountinfo;
use super::util::is_permission;

const CHECK: &str = "deleted_open";
const DELETED_SUFFIX: &str = " (deleted)";
const RELEASE_HINT: &str = "This process still has the file open, so the space stays allocated. \
Stop or restart it to release the space. This tool only reports and does not change anything.";

struct DeletedFile {
    pid: u32,
    bytes: u64,
    path: String,
}

struct Denied;

pub(crate) fn check(root: &Root) -> Vec<Finding> {
    let proc_dir = root.join("/proc");
    let entries = match fs::read_dir(&proc_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return vec![not_present()];
        }
        Err(err) if is_permission(&err) => {
            return vec![needs_root(
                "/proc",
                "deleted-but-open files (cannot read /proc)",
            )];
        }
        Err(err) => return vec![error_finding(format!("cannot read /proc: {err}"))],
    };

    let mounts = mountinfo::read_mount_points(root).unwrap_or_default();
    let mut files = Vec::new();
    let mut findings = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) if is_permission(&err) => {
                findings.push(needs_root(
                    "/proc",
                    "deleted-but-open files (cannot read /proc)",
                ));
                continue;
            }
            Err(err) => {
                findings.push(error_finding(format!("cannot read a /proc entry: {err}")));
                continue;
            }
        };
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(pid) = parse_pid(name) else {
            continue;
        };
        if let Err(Denied) = read_pid(root, pid, &mut files) {
            findings.push(needs_root(
                &format!("/proc/{pid}/fd"),
                &format!("deleted-but-open files for pid {pid} (cannot read /proc/{pid}/fd)"),
            ));
        }
    }

    findings.extend(aggregate(&files, &mounts));
    findings
}

fn read_pid(root: &Root, pid: u32, out: &mut Vec<DeletedFile>) -> Result<(), Denied> {
    let fd_dir = root.join(format!("/proc/{pid}/fd"));
    let entries = match fs::read_dir(&fd_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) if is_permission(&err) => return Err(Denied),
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) if is_permission(&err) => return Err(Denied),
            Err(_) => continue,
        };
        inspect_fd(&entry.path(), pid, out)?;
    }
    Ok(())
}

fn inspect_fd(fd_path: &Path, pid: u32, out: &mut Vec<DeletedFile>) -> Result<(), Denied> {
    let target = match fs::read_link(fd_path) {
        Ok(target) => target,
        Err(err) if is_permission(&err) => return Err(Denied),
        Err(_) => return Ok(()),
    };
    let target_text = target.to_string_lossy();
    if !target_text.ends_with(DELETED_SUFFIX) {
        return Ok(());
    }
    // `metadata` is stat(2). On procfs that follows the fd link to the
    // open inode, which still has a size after unlink.
    let bytes = match fs::metadata(fd_path) {
        Ok(meta) => meta.len(),
        Err(err) if is_permission(&err) => return Err(Denied),
        Err(_) => return Ok(()),
    };
    out.push(DeletedFile {
        pid,
        bytes,
        path: target_text.into_owned(),
    });
    Ok(())
}

struct Group {
    pid: u32,
    mount: String,
    bytes: u64,
    paths: Vec<String>,
}

fn aggregate(files: &[DeletedFile], mounts: &[String]) -> Vec<Finding> {
    let mut groups: Vec<Group> = Vec::new();
    for file in files {
        let logical = file.path.strip_suffix(DELETED_SUFFIX).unwrap_or(&file.path);
        let mount = mountinfo::mount_for(logical, mounts).to_string();
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.pid == file.pid && group.mount == mount)
        {
            group.bytes = group.bytes.saturating_add(file.bytes);
            group.paths.push(file.path.clone());
        } else {
            groups.push(Group {
                pid: file.pid,
                mount,
                bytes: file.bytes,
                paths: vec![file.path.clone()],
            });
        }
    }

    groups
        .into_iter()
        .map(|group| {
            Finding::new(
                CHECK,
                Status::Warn,
                Some(group.bytes),
                None,
                format!(
                    "pid {} mount {} {} bytes {}",
                    group.pid,
                    group.mount,
                    group.bytes,
                    group.paths.join(", ")
                ),
                RELEASE_HINT,
            )
        })
        .collect()
}

fn parse_pid(name: &str) -> Option<u32> {
    if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    name.parse().ok()
}

fn not_present() -> Finding {
    Finding::new(
        CHECK,
        Status::NotPresent,
        None,
        None,
        "procfs is not present",
        "There is no procfs under the root prefix, so deleted-but-open files cannot be listed.",
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
        "The check could not finish reading procfs.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_bytes_per_process_and_per_mount() {
        let files = vec![
            DeletedFile {
                pid: 1,
                bytes: 100,
                path: "/var/a (deleted)".to_string(),
            },
            DeletedFile {
                pid: 1,
                bytes: 50,
                path: "/var/b (deleted)".to_string(),
            },
            DeletedFile {
                pid: 1,
                bytes: 7,
                path: "/tmp/c (deleted)".to_string(),
            },
            DeletedFile {
                pid: 2,
                bytes: 9,
                path: "/var/d (deleted)".to_string(),
            },
        ];
        let mounts = vec!["/".to_string(), "/var".to_string(), "/tmp".to_string()];
        let findings = aggregate(&files, &mounts);

        let bytes = |pid: u32, mount: &str| {
            findings
                .iter()
                .find(|finding| {
                    finding.evidence.contains(&format!("pid {pid} "))
                        && finding.evidence.contains(&format!("mount {mount} "))
                })
                .map(|finding| finding.bytes)
        };
        assert_eq!(bytes(1, "/var"), Some(Some(150)));
        assert_eq!(bytes(1, "/tmp"), Some(Some(7)));
        assert_eq!(bytes(2, "/var"), Some(Some(9)));
        assert_eq!(findings.len(), 3);
        assert!(findings
            .iter()
            .all(|finding| finding.status == Status::Warn));
    }

    #[test]
    fn parse_pid_rejects_empty_non_digits_and_overflow() {
        assert_eq!(parse_pid(""), None);
        assert_eq!(parse_pid("12x"), None);
        assert_eq!(parse_pid("4294967296"), None);
        assert_eq!(parse_pid("0"), Some(0));
        assert_eq!(parse_pid("424242"), Some(424242));
    }
}
