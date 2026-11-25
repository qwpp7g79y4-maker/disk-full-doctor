//! Inode use per mount, from `<root>/proc/self/mountinfo` and [`FsProbe`].
//!
//! A mount at or above 90% inode use is a warning. The numbers come from
//! the probe, so tests never call `statvfs` on the host. Mountinfo is the
//! only mount source.

use std::io;

use crate::{Finding, FsProbe, FsStats, Root, Status};

use super::mountinfo;
use super::util::is_permission;

const CHECK: &str = "inodes";
/// Warn when used inodes are at or above this percentage of `f_files`.
const WARN_PERCENT: u64 = 90;

pub(crate) fn check(root: &Root, probe: &dyn FsProbe) -> Vec<Finding> {
    // Same lossy reader as the other checks. `read_to_string` rejects the
    // whole file on one non-UTF-8 byte and would skip every mount.
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
        match probe.statvfs(&stat_path) {
            Ok(stats) => {
                if stats.files == 0 {
                    continue;
                }
                findings.push(finding_for(mount, &stats));
            }
            Err(err) if is_permission(&err) => findings.push(needs_root_mount(mount)),
            Err(err) => findings.push(error_finding(format!(
                "cannot read inode use for {mount}: {err}"
            ))),
        }
    }
    if findings.is_empty() {
        findings.push(Finding::new(
            CHECK,
            Status::Ok,
            None,
            Some(0),
            "no mount reports a finite inode count",
            "Filesystems without an inode limit are below the 90% threshold.",
        ));
    }
    findings
}

fn finding_for(mount: &str, stats: &FsStats) -> Finding {
    let used = stats
        .files
        .saturating_sub(stats.files_free.min(stats.files));
    let percent = used_percent(stats.files, stats.files_free);
    let status = if percent >= WARN_PERCENT {
        Status::Warn
    } else {
        Status::Ok
    };
    let hint = if status == Status::Warn {
        "Inode use is at or above 90%. Remove files or raise the inode limit. \
This tool only reports and does not change anything."
    } else {
        "Inode use is below 90%."
    };
    Finding::new(
        CHECK,
        status,
        None,
        Some(used),
        format!("mount {mount} inodes {used}/{} ({percent}%)", stats.files),
        hint,
    )
}

fn used_percent(total: u64, free: u64) -> u64 {
    if total == 0 {
        return 0;
    }
    let used = total.saturating_sub(free.min(total));
    used.saturating_mul(100) / total
}

fn not_present() -> Finding {
    Finding::new(
        CHECK,
        Status::NotPresent,
        None,
        None,
        "mountinfo is not present",
        "There is no mount table under the root prefix, so inode use cannot be listed.",
    )
}

fn needs_root_mountinfo() -> Finding {
    Finding::new(
        CHECK,
        Status::NeedsRoot,
        None,
        None,
        "cannot read /proc/self/mountinfo",
        "run with sudo to see inode use (cannot read /proc/self/mountinfo)",
    )
}

fn needs_root_mount(mount: &str) -> Finding {
    Finding::new(
        CHECK,
        Status::NeedsRoot,
        None,
        None,
        format!("cannot read inode use for {mount}"),
        format!("run with sudo to see inode use for {mount}"),
    )
}

fn error_finding(evidence: String) -> Finding {
    Finding::new(
        CHECK,
        Status::Error,
        None,
        None,
        evidence,
        "The check could not finish reading inode use.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_warns_at_90_and_above() {
        assert_eq!(used_percent(100, 5), 95);
        assert_eq!(used_percent(100, 90), 10);
        assert_eq!(used_percent(100, 10), 90);
        assert_eq!(used_percent(100, 11), 89);
        assert_eq!(used_percent(0, 0), 0);
        assert_eq!(used_percent(100, 150), 0);
    }
}
