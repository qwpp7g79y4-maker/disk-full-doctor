//! Read-only report of why a disk looks full when `df` and `du` disagree.
//!
//! The tool never writes, deletes, or opens a network connection.
//! Every path goes through [`Root`].

#![forbid(unsafe_code)]

mod checks;
mod probe;
mod report;
mod root;

pub use probe::{FsProbe, FsStats, RealFsProbe};
pub use report::{escape, Finding, Report, Status};
pub use root::Root;

/// Run every check against `root`.
///
/// Later milestones append checks here. Nothing in this function opens a
/// path that did not come from `root`.
pub fn scan(root: &Root, probe: &dyn FsProbe) -> Report {
    let mut findings = Vec::new();
    findings.extend(checks::deleted_open(root));
    findings.extend(checks::inodes(root, probe));
    findings.extend(checks::docker_logs(root));
    findings.extend(checks::journald(root));
    findings.extend(checks::buildkit(root));
    findings.extend(checks::df_du_gap(root, probe));
    Report::from_findings(findings)
}
