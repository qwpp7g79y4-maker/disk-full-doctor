//! Size of the journald logs, and `SystemMaxUse` if it is set.
//!
//! Reads `<root>/var/log/journal`, `<root>/run/log/journal`, and
//! `<root>/etc/systemd/journald.conf`. No `journalctl` call.

use std::fs;

use crate::{Finding, Root, Status};

use super::util::{dir_bytes, is_permission};

const CHECK: &str = "journald";
const HINT: &str = "Journald rotates these logs automatically when SystemMaxUse is set. Without \
a limit they can grow until the disk is full. This tool only reports and does not change \
anything.";

pub(crate) fn check(root: &Root) -> Vec<Finding> {
    let persistent = dir_bytes(&root.join("/var/log/journal"));
    let runtime = dir_bytes(&root.join("/run/log/journal"));

    if matches!(&persistent, Err(err) if is_permission(err)) {
        return vec![needs_root("/var/log/journal")];
    }
    if matches!(&runtime, Err(err) if is_permission(err)) {
        return vec![needs_root("/run/log/journal")];
    }

    let persistent = match persistent {
        Ok(bytes) => bytes,
        Err(err) => {
            return vec![error_finding(format!(
                "cannot read /var/log/journal: {err}"
            ))]
        }
    };
    let runtime = match runtime {
        Ok(bytes) => bytes,
        Err(err) => {
            return vec![error_finding(format!(
                "cannot read /run/log/journal: {err}"
            ))]
        }
    };

    if persistent.is_none() && runtime.is_none() {
        return vec![not_present()];
    }

    let bytes = persistent.unwrap_or(0) + runtime.unwrap_or(0);
    let mut evidence = format!("/var/log/journal + /run/log/journal use {bytes} bytes");
    if let Some(max_use) = read_system_max_use(root) {
        evidence.push_str(&format!(" (SystemMaxUse={max_use})"));
    }
    vec![Finding::new(
        CHECK,
        Status::Ok,
        Some(bytes),
        None,
        evidence,
        HINT,
    )]
}

/// Best-effort read of `SystemMaxUse` from journald.conf. Missing, unreadable,
/// or unset is `None`, never an error: this value is a bonus hint, not
/// something the check depends on.
fn read_system_max_use(root: &Root) -> Option<String> {
    let path = root.join("/etc/systemd/journald.conf");
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(value) = line.strip_prefix("SystemMaxUse=") {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn not_present() -> Finding {
    Finding::new(
        CHECK,
        Status::NotPresent,
        None,
        None,
        "/var/log/journal and /run/log/journal are not present",
        "No journald logs were found under either path.",
    )
}

fn needs_root(path: &str) -> Finding {
    Finding::new(
        CHECK,
        Status::NeedsRoot,
        None,
        None,
        format!("cannot read {path}"),
        format!("run with sudo to see journald logs (cannot read {path})"),
    )
}

fn error_finding(evidence: String) -> Finding {
    Finding::new(
        CHECK,
        Status::Error,
        None,
        None,
        evidence,
        "The check could not finish reading journald logs.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_system_max_use_is_none() {
        assert_eq!(read_system_max_use(&Root::new("/does/not/exist")), None);
    }
}
